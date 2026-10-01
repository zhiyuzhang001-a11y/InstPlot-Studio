//! Native installer execution. No shell, elevation, forced exit, or GUI launch.
//! The session retains its installer lease and installation lock until exit.
use std::io;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};

use super::{
    PinnedWindowsInstaller, TrackedWindowsProcess, WindowsInstallAccess, WindowsInstallation,
};
use super::{create_private_file, invalid, validate_private_directory};

pub struct RunningWindowsInstaller<'a> {
    child: Child,
    _installer: PinnedWindowsInstaller,
    _access: &'a WindowsInstallAccess,
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
        // Reserve a fresh private log. Inno may write it but not overwrite an
        // unrelated log or repair a foreign ACL. Keep logs on spawn failure.
        drop(create_private_file(log)?);
        let child = Command::new(installer.installer().path())
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        Ok(Self {
            child,
            _installer: installer,
            _access: access,
        })
    }

    /// Poll from the external helper's bounded state machine. None means the
    /// installer is still running: no application launch or recovery is safe.
    /// Retain this object on timeout; do not kill the installer or drop locks.
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.child.try_wait()
    }

    pub fn process_id(&self) -> u32 {
        self.child.id()
    }
}
