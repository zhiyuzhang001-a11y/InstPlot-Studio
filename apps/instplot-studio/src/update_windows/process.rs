//! Bind one real process and wait for normal exit. Never terminate a process.
use std::io;
use std::path::Path;
use std::time::Duration;

use windows::Win32::Foundation::{CloseHandle, FILETIME, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows::Win32::System::Threading::{
    GetProcessTimes, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    PROCESS_SYNCHRONIZE, QueryFullProcessImageNameW, WaitForSingleObject,
};
use windows::core::PWSTR;

pub struct TrackedWindowsProcess {
    handle: HANDLE,
    bound_executable: Option<std::path::PathBuf>,
}

impl Drop for TrackedWindowsProcess {
    fn drop(&mut self) {
        // SAFETY: this object exclusively owns a successful OpenProcess handle.
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

impl TrackedWindowsProcess {
    /// Bind a child launched by this helper. The Child retains its native
    /// handle while creation time/path are checked, preventing PID reuse.
    pub fn bind_child(child: &std::process::Child, executable: &Path) -> io::Result<Self> {
        let created = Self::capture(child.id())?.created()?;
        Self::bind(child.id(), created, executable)
    }

    fn capture(pid: u32) -> io::Result<Self> {
        if pid == 0 {
            return Err(io::Error::other("invalid update process identity"));
        }
        // SAFETY: read/query and synchronization only, no inheritance or termination rights.
        let handle = unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                false,
                pid,
            )
        }
        .map_err(|e| io::Error::other(e.to_string()))?;
        Ok(Self {
            handle,
            bound_executable: None,
        })
    }

    pub fn bind(pid: u32, created: u64, executable: &Path) -> io::Result<Self> {
        let mut process = Self::capture(pid)?;
        if created == 0 || process.created()? != created {
            return Err(io::Error::other(
                "update process creation time does not match",
            ));
        }
        let mut buffer = vec![0_u16; 32768];
        let mut count = buffer.len() as u32;
        // SAFETY: owned writable UTF-16 buffer and live process handle; length in characters.
        unsafe {
            QueryFullProcessImageNameW(
                process.handle,
                PROCESS_NAME_WIN32,
                PWSTR(buffer.as_mut_ptr()),
                &mut count,
            )
        }
        .map_err(|e| io::Error::other(e.to_string()))?;
        if count == 0 || count as usize >= buffer.len() {
            return Err(io::Error::other("invalid update process path length"));
        }
        use std::os::windows::ffi::OsStringExt;
        let actual =
            std::path::PathBuf::from(std::ffi::OsString::from_wide(&buffer[..count as usize]));
        if !executable.is_absolute()
            || std::fs::canonicalize(actual)? != std::fs::canonicalize(executable)?
        {
            return Err(io::Error::other("update process executable does not match"));
        }
        process.bound_executable = Some(std::fs::canonicalize(executable)?);
        Ok(process)
    }

    pub(super) fn require_executable(&self, executable: &Path) -> io::Result<()> {
        // A partially failed installer may have removed the old exe. Recovery
        // still uses the previously bound process identity, not a new process
        // guessed from the missing path. Its parent must remain the same.
        let expected = match std::fs::canonicalize(executable) {
            Ok(path) => path,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let parent = executable.parent().ok_or(error)?;
                let name = executable
                    .file_name()
                    .ok_or_else(|| io::Error::other("missing executable name"))?;
                std::fs::canonicalize(parent)?.join(name)
            }
            Err(error) => return Err(error),
        };
        if self.bound_executable.as_ref() != Some(&expected) {
            return Err(io::Error::other(
                "old process is not bound to this installation",
            ));
        }
        Ok(())
    }

    fn created(&self) -> io::Result<u64> {
        let mut creation = FILETIME::default();
        let mut exit = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        // SAFETY: valid owned handle and four initialized writable FILETIME values.
        unsafe {
            GetProcessTimes(
                self.handle,
                &mut creation,
                &mut exit,
                &mut kernel,
                &mut user,
            )
        }
        .map_err(|e| io::Error::other(e.to_string()))?;
        Ok((u64::from(creation.dwHighDateTime) << 32) | u64::from(creation.dwLowDateTime))
    }

    /// False means the same process is still alive; do not apply or restore yet.
    pub fn wait_for_exit(&self, timeout: Duration) -> io::Result<bool> {
        if timeout > Duration::from_secs(30) {
            return Err(io::Error::other(
                "normal-exit wait exceeds its bounded limit",
            ));
        }
        let millis = u32::try_from(timeout.as_millis()).map_err(io::Error::other)?;
        // SAFETY: live owned process handle, finite wait, never closes or kills the process.
        match unsafe { WaitForSingleObject(self.handle, millis) } {
            WAIT_OBJECT_0 => Ok(true),
            WAIT_TIMEOUT => Ok(false),
            _ => Err(io::Error::last_os_error()),
        }
    }
}

pub fn current_process_created() -> io::Result<u64> {
    TrackedWindowsProcess::capture(std::process::id())?.created()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_process_is_bound_and_never_forced_to_exit() {
        let created = current_process_created().unwrap();
        let executable = std::env::current_exe().unwrap();
        let process =
            TrackedWindowsProcess::bind(std::process::id(), created, &executable).unwrap();
        process.require_executable(&executable).unwrap();
        assert!(!process.wait_for_exit(Duration::ZERO).unwrap());
        assert!(process.wait_for_exit(Duration::from_secs(31)).is_err());
        assert!(TrackedWindowsProcess::bind(std::process::id(), created + 1, &executable).is_err());
        assert!(TrackedWindowsProcess::bind(0, created, &executable).is_err());
        assert!(
            TrackedWindowsProcess::bind(
                std::process::id(),
                created,
                &executable.with_file_name("foreign.exe")
            )
            .is_err()
        );
    }
}
