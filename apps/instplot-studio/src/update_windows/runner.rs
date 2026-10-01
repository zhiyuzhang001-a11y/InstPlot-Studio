//! Native installer execution. No shell, elevation, forced exit, or GUI launch.
//! The session retains its installer lease and installation lock until exit.
use std::io;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};

use super::{
    PinnedWindowsInstaller, TrackedWindowsProcess, WindowsInstallAccess, WindowsInstallation,
};
use super::{create_private_file, invalid, validate_private_directory};
use crate::update_transaction::{TransactionStore, UpdateTransaction};

pub struct RunningWindowsInstaller<'a> {
    child: Child,
    _installer: PinnedWindowsInstaller,
    access: Option<InstallerAccess<'a>>,
    journal: super::journal::InstallerJournal<'a>,
    checkpoint_error: Option<String>,
}

enum InstallerAccess<'a> {
    Borrowed(&'a WindowsInstallAccess),
    Owned(WindowsInstallAccess),
}

struct InstallerLaunch<'a, 'b> {
    installation: &'b WindowsInstallation,
    previous_process: &'b TrackedWindowsProcess,
    log: &'b Path,
    store: &'a TransactionStore,
    transaction: &'b UpdateTransaction,
}

impl InstallerAccess<'_> {
    fn guard(&self) -> &WindowsInstallAccess {
        match self {
            Self::Borrowed(access) => access,
            Self::Owned(access) => access,
        }
    }
}

impl<'a> RunningWindowsInstaller<'a> {
    /// Call only after the bound old GUI has exited normally. Acquiring this
    /// exclusive access already rejects any supported instance still running.
    /// On installer failure, keep the transaction in recovery-required; never
    /// interpret successful spawn or a still-running child as successful apply.
    pub fn start(
        installation: &WindowsInstallation,
        installer: PinnedWindowsInstaller,
        access: &'a WindowsInstallAccess,
        previous_process: &TrackedWindowsProcess,
        log: &Path,
        store: &'a TransactionStore,
        transaction: &UpdateTransaction,
    ) -> io::Result<Self> {
        Self::start_with_access(
            installation,
            installer,
            InstallerAccess::Borrowed(access),
            previous_process,
            log,
            store,
            transaction,
        )
    }

    /// Owned exclusion avoids self-referential helper state. The live installer
    /// retains it; transfer is possible only after a durable native exit.
    pub fn start_owned(
        installation: &WindowsInstallation,
        installer: PinnedWindowsInstaller,
        access: WindowsInstallAccess,
        previous_process: &TrackedWindowsProcess,
        log: &Path,
        store: &'a TransactionStore,
        transaction: &UpdateTransaction,
    ) -> io::Result<Self> {
        Self::start_with_access(
            installation,
            installer,
            InstallerAccess::Owned(access),
            previous_process,
            log,
            store,
            transaction,
        )
    }

    fn start_with_access(
        installation: &WindowsInstallation,
        installer: PinnedWindowsInstaller,
        access: InstallerAccess<'a>,
        previous_process: &TrackedWindowsProcess,
        log: &Path,
        store: &'a TransactionStore,
        transaction: &UpdateTransaction,
    ) -> io::Result<Self> {
        Self::start_with_spawn(
            installer,
            access,
            InstallerLaunch {
                installation,
                previous_process,
                log,
                store,
                transaction,
            },
            |path, arguments| {
                Command::new(path)
                    .args(arguments)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
            },
        )
    }

    #[cfg(test)]
    pub(super) fn start_owned_failure_fixture(
        installation: &WindowsInstallation,
        installer: PinnedWindowsInstaller,
        access: WindowsInstallAccess,
        previous_process: &TrackedWindowsProcess,
        log: &Path,
        store: &'a TransactionStore,
        transaction: &UpdateTransaction,
    ) -> io::Result<Self> {
        Self::start_with_spawn(
            installer,
            InstallerAccess::Owned(access),
            InstallerLaunch {
                installation,
                previous_process,
                log,
                store,
                transaction,
            },
            |path, _| {
                Command::new(path)
                    .args([
                        "--exact",
                        "update_windows::assets::tests::installer_failure_test_child",
                    ])
                    .env("STUDIO_INSTALLER_FAILURE_FIXTURE", "1")
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
            },
        )
    }

    fn start_with_spawn(
        installer: PinnedWindowsInstaller,
        access: InstallerAccess<'a>,
        launch: InstallerLaunch<'a, '_>,
        spawn: impl FnOnce(&Path, Vec<std::ffi::OsString>) -> io::Result<Child>,
    ) -> io::Result<Self> {
        let InstallerLaunch {
            installation,
            previous_process,
            log,
            store,
            transaction,
        } = launch;
        access.guard().require_exclusive(installation.directory())?;
        previous_process.require_executable(installation.executable())?;
        if !previous_process.wait_for_exit(std::time::Duration::ZERO)? {
            return Err(invalid("bound old application has not exited normally"));
        }
        let version = installer.installer().version();
        if version != installation.version() {
            installation.validate_candidate_version(&version.to_string())?;
            super::native::revalidate_installation(installation)?;
        }
        installer.installer().revalidate()?;
        let arguments = installation.installer_arguments(log)?;
        validate_private_directory(
            log.parent()
                .ok_or_else(|| invalid("installer log has no parent"))?,
        )?;
        store.require_directory(
            log.parent()
                .ok_or_else(|| invalid("installer log has no parent"))?,
        )?;
        let mut journal = super::journal::InstallerJournal::begin(
            store,
            transaction,
            installation,
            installer.installer(),
        )?;
        // Reserve a fresh private log. Inno may write it but not overwrite an
        // unrelated log or repair a foreign ACL. Keep logs on spawn failure.
        if let Err(error) = create_private_file(log).map(drop) {
            let _ = journal.not_started();
            return Err(error);
        }
        let child = match spawn(installer.installer().path(), arguments) {
            Ok(child) => child,
            Err(error) => {
                let _ = journal.not_started();
                return Err(error);
            }
        };
        // Never return Err/drop the lease after CreateProcess succeeds. A
        // checkpoint failure retains the live child, lease and access lock;
        // persisted Intent prevents a later helper from blindly replaying it.
        let checkpoint_error = super::process::child_process_created(&child)
            .and_then(|created| journal.running(child.id(), created))
            .err()
            .map(|error| error.to_string());
        Ok(Self {
            child,
            _installer: installer,
            access: Some(access),
            journal,
            checkpoint_error,
        })
    }

    /// Poll from the external helper's bounded state machine. None means the
    /// installer is still running: no application launch or recovery is safe.
    /// Retain this object on timeout OR checkpoint error; do not kill the
    /// installer or drop locks. Exit is returned only after its durable record,
    /// and does not by itself mean installation or GUI health succeeded.
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        if self.access.is_none() {
            return Err(invalid("installer exclusion was already transferred"));
        }
        let Some(status) = self.child.try_wait()? else {
            return match &self.checkpoint_error {
                Some(error) => Err(io::Error::other(error.clone())),
                None => Ok(None),
            };
        };
        let created = super::process::child_process_created(&self.child)?;
        self.journal.exited(
            self.child.id(),
            created,
            status
                .code()
                .ok_or_else(|| invalid("installer exit code unavailable"))?,
        )?;
        self.checkpoint_error = None;
        Ok(Some(status))
    }

    pub fn process_id(&self) -> u32 {
        self.child.id()
    }

    /// Read-only failed-exit proof for recovery before any candidate GUI exists.
    /// Requires this retained Child and the already-transferred owned exclusion;
    /// a journal on disk alone cannot authorize recovery or replay.
    pub(super) fn require_owned_failed_exit(
        &mut self,
        store: &TransactionStore,
        transaction: &UpdateTransaction,
        installation: &WindowsInstallation,
        installer: &super::VerifiedWindowsInstaller,
    ) -> io::Result<()> {
        if self.access.is_some() || self.checkpoint_error.is_some() {
            return Err(invalid(
                "installer exclusion has not been durably transferred",
            ));
        }
        if self._installer.installer().path() != installer.path()
            || self._installer.installer().sha256() != installer.sha256()
            || self._installer.installer().version() != installer.version()
            || self._installer.installer().size_bytes() != installer.size_bytes()
        {
            return Err(invalid(
                "failed installer asset differs from retained package",
            ));
        }
        let status = self
            .child
            .try_wait()?
            .ok_or_else(|| invalid("installer is still running"))?;
        self.journal.require_owned_failed_exit(
            store,
            transaction,
            installation,
            installer,
            (
                self.child.id(),
                super::process::child_process_created(&self.child)?,
            ),
            status
                .code()
                .ok_or_else(|| invalid("installer exit code unavailable"))?,
        )
    }

    /// No drop-on-error ownership transfer. A live child or checkpoint error
    /// leaves this object holding its guard and package lease for inspection.
    pub fn take_owned_access_after_exit(&mut self) -> io::Result<WindowsInstallAccess> {
        if !matches!(self.access, Some(InstallerAccess::Owned(_))) {
            return Err(invalid("installer has no owned exclusion to transfer"));
        }
        if self.try_wait()?.is_none() {
            return Err(invalid("installer is still running"));
        }
        match self.access.take() {
            Some(InstallerAccess::Owned(access)) => Ok(access),
            _ => unreachable!("owned exclusion checked without intervening mutation"),
        }
    }
}
