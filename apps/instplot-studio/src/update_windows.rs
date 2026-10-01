//! Windows installation contract, before any installer is allowed to run.
//!
//! This module deliberately does not execute installers. The Windows adapter must
//! obtain the record from HKCU, verify the executable/product/version, acquire a
//! private transaction lock, and verify both installer assets before using it.
//! Registry identity alone is not authentication of an installer or process.

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use semver::Version;

pub const STUDIO_APP_ID: &str = "{F5A7E98E-2AFB-4E58-8DF8-C20DB09D42A2}_is1";
const EXECUTABLE: &str = "instplot-studio.exe";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallScope {
    CurrentUser,
    Machine,
    Unknown,
}

/// Snapshot from native installation discovery, not user-supplied configuration.
#[derive(Clone, Debug)]
pub struct WindowsInstallRecord {
    pub app_id: String,
    pub scope: InstallScope,
    pub directory: PathBuf,
    pub version: String,
    pub desktop_shortcut: bool,
}

#[derive(Clone, Debug)]
pub struct WindowsInstallation {
    directory: PathBuf,
    executable: PathBuf,
    version: Version,
    desktop_shortcut: bool,
}

impl WindowsInstallation {
    /// Fail closed when the running executable differs from the installation
    /// record. Reject links/reparse points instead of following an unknown target.
    pub fn bind(
        record: &WindowsInstallRecord,
        running_executable: &Path,
        running_version: &str,
    ) -> io::Result<Self> {
        if record.app_id != STUDIO_APP_ID || record.scope != InstallScope::CurrentUser {
            return Err(invalid(
                "only the registered current-user Studio installation is supported",
            ));
        }
        let version = Version::parse(&record.version).map_err(invalid)?;
        if version != Version::parse(running_version).map_err(invalid)? {
            return Err(invalid("running and registered versions differ"));
        }
        reject_redirected_path(&record.directory)?;
        reject_redirected_path(running_executable)?;
        let directory = fs::canonicalize(&record.directory)?;
        let executable = fs::canonicalize(running_executable)?;
        let registered_executable = directory.join(EXECUTABLE);
        reject_redirected_path(&registered_executable)?;
        if !directory.is_dir()
            || !executable.is_file()
            || executable != fs::canonicalize(&registered_executable)?
        {
            return Err(invalid(
                "running executable is not the registered installation",
            ));
        }
        Ok(Self {
            directory,
            executable,
            version,
            desktop_shortcut: record.desktop_shortcut,
        })
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn executable(&self) -> &Path {
        &self.executable
    }

    pub fn version(&self) -> &Version {
        &self.version
    }

    /// Same product channel, strictly newer version. This does not replace
    /// signed-manifest, sequence, expiry or package hash validation.
    pub fn validate_candidate_version(&self, candidate: &str) -> io::Result<()> {
        let candidate = Version::parse(candidate).map_err(invalid)?;
        if candidate <= self.version || candidate.pre.is_empty() != self.version.pre.is_empty() {
            return Err(invalid(
                "candidate is not a newer version in the same channel",
            ));
        }
        Ok(())
    }

    /// Pass separate arguments directly to Command, never through a shell.
    /// The controller has already requested normal exit; Inno may not force it.
    pub fn installer_arguments(&self, log: &Path) -> io::Result<Vec<OsString>> {
        if !log.is_absolute() || log.components().any(|p| matches!(p, Component::ParentDir)) {
            return Err(invalid(
                "installer log must be in an absolute private transaction path",
            ));
        }
        let mut directory = OsString::from("/DIR=");
        directory.push(&self.directory);
        let mut log_argument = OsString::from("/LOG=");
        log_argument.push(log);
        Ok(vec![
            "/VERYSILENT".into(),
            "/SUPPRESSMSGBOXES".into(),
            "/NORESTART".into(),
            "/SP-".into(),
            "/NOCLOSEAPPLICATIONS".into(),
            "/NORESTARTAPPLICATIONS".into(),
            if self.desktop_shortcut {
                "/TASKS=desktopicon"
            } else {
                "/TASKS=!desktopicon"
            }
            .into(),
            directory,
            log_argument,
        ])
    }
}

fn reject_redirected_path(path: &Path) -> io::Result<()> {
    if !path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
    {
        return Err(invalid(
            "installation path must be absolute without traversal",
        ));
    }
    let mut prefix = PathBuf::new();
    for part in path.components() {
        prefix.push(part);
        #[cfg(windows)]
        if let Component::Prefix(value) = part {
            use std::path::Prefix;
            if !matches!(value.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)) {
                return Err(invalid(
                    "network and device installation paths are unsupported",
                ));
            }
            // A drive prefix without its root refers to that drive's current
            // directory. Inspect only after RootDir has been appended.
            continue;
        }
        let metadata = fs::symlink_metadata(&prefix)?;
        if metadata.file_type().is_symlink() {
            return Err(invalid("installation path contains a symbolic link"));
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err(invalid("installation path contains a reparse point"));
            }
        }
    }
    Ok(())
}

fn invalid(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let mut random = [0; 8];
            getrandom::fill(&mut random).unwrap();
            let root = std::env::temp_dir().join(format!(
                "studio-windows-contract-{:x}",
                u64::from_le_bytes(random)
            ));
            fs::create_dir(&root).unwrap();
            // macOS /var is a system alias; use its resolved temporary root.
            let root = fs::canonicalize(root).unwrap();
            let directory = root.join("custom path 数据");
            fs::create_dir(&directory).unwrap();
            fs::write(directory.join(EXECUTABLE), b"test fixture only").unwrap();
            Self(root)
        }
        fn record(&self) -> WindowsInstallRecord {
            WindowsInstallRecord {
                app_id: STUDIO_APP_ID.into(),
                scope: InstallScope::CurrentUser,
                directory: self.0.join("custom path 数据"),
                version: "0.1.2-rc.2".into(),
                desktop_shortcut: true,
            }
        }
        fn bind(&self, record: &WindowsInstallRecord) -> io::Result<WindowsInstallation> {
            WindowsInstallation::bind(
                record,
                &self.0.join("custom path 数据").join(EXECUTABLE),
                "0.1.2-rc.2",
            )
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            // Only this unique, test-owned fixture is removed.
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn custom_unicode_path_and_task_selection_are_preserved() {
        let fixture = Fixture::new();
        for desktop in [true, false] {
            let mut record = fixture.record();
            record.desktop_shortcut = desktop;
            let install = fixture.bind(&record).unwrap();
            let args = install
                .installer_arguments(&fixture.0.join("private log.txt"))
                .unwrap();
            assert!(args.contains(&OsString::from("/NOCLOSEAPPLICATIONS")));
            assert!(!args.contains(&OsString::from("/CLOSEAPPLICATIONS")));
            assert_eq!(
                args[6],
                if desktop {
                    "/TASKS=desktopicon"
                } else {
                    "/TASKS=!desktopicon"
                }
            );
            let mut expected = OsString::from("/DIR=");
            expected.push(&record.directory);
            assert_eq!(args[7], expected);
            assert_eq!(install.executable(), record.directory.join(EXECUTABLE));
        }
    }

    #[test]
    fn wrong_scope_app_id_and_version_are_rejected() {
        let fixture = Fixture::new();
        for scope in [InstallScope::Machine, InstallScope::Unknown] {
            let mut record = fixture.record();
            record.scope = scope;
            assert!(fixture.bind(&record).is_err());
        }
        let mut record = fixture.record();
        record.app_id = "InstPlot Lite".into();
        assert!(fixture.bind(&record).is_err());
        record = fixture.record();
        record.version = "0.1.2-rc.1".into();
        assert!(fixture.bind(&record).is_err());
    }

    #[test]
    fn another_installation_and_missing_registration_path_are_rejected() {
        let fixture = Fixture::new();
        let mut record = fixture.record();
        record.directory = fixture.0.join("another");
        fs::create_dir(&record.directory).unwrap();
        fs::write(record.directory.join(EXECUTABLE), b"other").unwrap();
        assert!(fixture.bind(&record).is_err());
        record.directory = fixture.0.join("missing");
        assert!(fixture.bind(&record).is_err());
        record.directory = PathBuf::from("relative");
        assert!(fixture.bind(&record).is_err());
    }

    #[test]
    fn candidate_must_be_newer_without_changing_channel() {
        let fixture = Fixture::new();
        let install = fixture.bind(&fixture.record()).unwrap();
        assert!(install.validate_candidate_version("0.1.2-rc.3").is_ok());
        for version in ["0.1.2-rc.1", "0.1.2-rc.2", "0.1.2", "not-a-version"] {
            assert!(install.validate_candidate_version(version).is_err());
        }
        assert!(
            install
                .installer_arguments(Path::new("relative.log"))
                .is_err()
        );
        // PathBuf::join normalizes '..' on Windows verbatim canonical paths.
        // Preserve raw input so the rejection test actually supplies traversal.
        let mut traversal = fixture.0.as_os_str().to_os_string();
        let separator = std::path::MAIN_SEPARATOR_STR;
        traversal.push(format!("{separator}..{separator}escape.log"));
        assert!(install.installer_arguments(Path::new(&traversal)).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_installation_is_rejected() {
        let fixture = Fixture::new();
        let mut record = fixture.record();
        std::os::unix::fs::symlink(&record.directory, fixture.0.join("redirect")).unwrap();
        record.directory = fixture.0.join("redirect");
        assert!(fixture.bind(&record).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn network_and_device_paths_fail_before_discovery() {
        let fixture = Fixture::new();
        for path in [
            r"\\server\share\Studio",
            r"\\?\UNC\server\share\Studio",
            r"\\.\C:\Studio",
        ] {
            let mut record = fixture.record();
            record.directory = PathBuf::from(path);
            let error = fixture.bind(&record).unwrap_err();
            assert!(error.to_string().contains("network and device"), "{error}");
        }
    }
}
