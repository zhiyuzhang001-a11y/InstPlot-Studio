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
    _access: &'a WindowsInstallAccess,
    journal: super::journal::InstallerJournal<'a>,
    checkpoint_error: Option<String>,
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
        access.require_exclusive(installation.directory())?;
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
        let child = match Command::new(installer.installer().path())
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
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
            _access: access,
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
}
