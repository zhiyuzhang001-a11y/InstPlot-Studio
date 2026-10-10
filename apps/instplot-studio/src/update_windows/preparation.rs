//! Prepare BOTH trusted installers while the old GUI is still running.
//! No process exit, installer execution, persisted readiness or recovery decision.
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::UpdatePackage;

use super::{
    PinnedWindowsInstallerPair, VerifiedWindowsInstaller, VerifiedWindowsInstallerManifest,
    WindowsInstallation, WindowsInstallerPair, invalid,
};

/// The leases must be retained until ownership is safely handed to the helper.
/// This is not a durable request, helper acknowledgement, or permission to exit.
#[derive(Debug)]
pub struct PreparedWindowsInstallers {
    installation: WindowsInstallation,
    installers: PinnedWindowsInstallerPair,
}

impl PreparedWindowsInstallers {
    pub fn installation(&self) -> &WindowsInstallation {
        &self.installation
    }

    pub fn installers(&self) -> &PinnedWindowsInstallerPair {
        &self.installers
    }
}

/// The downloader must use the normal strict HTTPS/size/hash/cancellation path.
/// Its return is never accepted as proof: both caches are freshly verified here.
/// An existing recovery package is verified, never overwritten or repaired.
pub fn prepare_installers(
    candidate_cache: &Path,
    candidate_version: &str,
    cancel: &AtomicBool,
    download: impl FnOnce(&UpdatePackage, &Path) -> io::Result<()>,
) -> io::Result<PreparedWindowsInstallers> {
    check_cancel(cancel)?;
    let installation = super::discover_current_installation()?;
    installation.validate_candidate_version(candidate_version)?;
    let candidate = VerifiedWindowsInstaller::from_cache(candidate_cache, candidate_version)?;
    // Prevent candidate replacement during the potentially long old-package download.
    let candidate_lease = candidate.pin()?;
    probe_directory_writable(installation.directory())?;
    let (recovery_cache, metadata) = super::cached_recovery_manifest(&installation)?;
    obtain_recovery_package(&recovery_cache, &metadata, cancel, download)?;
    check_cancel(cancel)?;
    // Fresh signature/expiry verification of BOTH manifests, not only a remembered
    // descriptor. Package size/hash/private ACL and channel/role checks follow.
    let pair = WindowsInstallerPair::from_caches(
        &installation,
        &recovery_cache,
        candidate_cache,
        candidate_version,
    )?;
    let installers = pair.pin()?;
    super::native::revalidate_installation(&installation)?;
    probe_directory_writable(installation.directory())?;
    check_cancel(cancel)?;
    drop(candidate_lease);
    Ok(PreparedWindowsInstallers {
        installation,
        installers,
    })
}

fn check_cancel(cancel: &AtomicBool) -> io::Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "update preparation cancelled",
        ))
    } else {
        Ok(())
    }
}

fn obtain_recovery_package(
    directory: &Path,
    metadata: &VerifiedWindowsInstallerManifest,
    cancel: &AtomicBool,
    download: impl FnOnce(&UpdatePackage, &Path) -> io::Result<()>,
) -> io::Result<()> {
    check_cancel(cancel)?;
    super::validate_private_directory(directory)?;
    let destination = directory.join(&metadata.package().file_name);
    if !destination.try_exists()? {
        download(metadata.package(), &destination)?;
    }
    check_cancel(cancel)
}

fn probe_directory_writable(directory: &Path) -> io::Result<()> {
    use std::os::windows::fs::OpenOptionsExt;

    super::reject_redirected_path(directory)?;
    if !directory.is_dir() {
        return Err(invalid("installation directory is missing"));
    }
    let mut random = [0_u8; 16];
    getrandom::fill(&mut random).map_err(invalid)?;
    let path = directory.join(format!(
        ".studio-update-write-probe-{:032x}",
        u128::from_le_bytes(random)
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .share_mode(0)
        .open(&path)?;
    let result = file
        .write_all(b"Studio updater write probe")
        .and_then(|()| file.sync_all());
    drop(file);
    // Delete ONLY the unique file created by this call; never a wildcard or an
    // installation file. Cleanup failure blocks preparation and is reported.
    let cleanup = fs::remove_file(&path);
    result.and(cleanup)?;
    super::reject_redirected_path(directory)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn private_fixture() -> std::path::PathBuf {
        let mut random = [0_u8; 16];
        getrandom::fill(&mut random).unwrap();
        let path = std::env::temp_dir().join(format!(
            "studio-prepare 中文 spaces-{:032x}",
            u128::from_le_bytes(random)
        ));
        super::super::create_private_directory(&path).unwrap();
        fs::canonicalize(path).unwrap()
    }

    #[test]
    fn recovery_download_is_cancelable_and_never_overwrites_existing_package() {
        let root = private_fixture();
        let metadata = super::super::assets::metadata_fixture(2);
        let cancel = AtomicBool::new(true);
        assert!(
            obtain_recovery_package(&root, &metadata, &cancel, |_, _| {
                panic!("cancelled preparation must not download")
            })
            .is_err()
        );
        cancel.store(false, Ordering::Relaxed);
        let destination = root.join(&metadata.package().file_name);
        let mut file = super::super::create_private_file(&destination).unwrap();
        file.write_all(b"existing corrupt package: verify later, never replace")
            .unwrap();
        drop(file);
        obtain_recovery_package(&root, &metadata, &cancel, |_, _| {
            panic!("existing package must not be replaced")
        })
        .unwrap();
        assert_eq!(
            fs::read(&destination).unwrap(),
            b"existing corrupt package: verify later, never replace"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cancellation_after_download_is_not_prepared_readiness() {
        let root = private_fixture();
        let metadata = super::super::assets::metadata_fixture(2);
        let cancel = AtomicBool::new(false);
        let result = obtain_recovery_package(&root, &metadata, &cancel, |package, path| {
            assert_eq!(path, root.join(&package.file_name));
            let mut file = super::super::create_private_file(path)?;
            file.write_all(b"test package, not executable evidence")?;
            cancel.store(true, Ordering::Relaxed);
            Ok(())
        });
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::Interrupted);
        assert!(root.join(&metadata.package().file_name).is_file());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn write_probe_preserves_user_files_and_leaves_no_extra_file() {
        let root = private_fixture();
        let user = root.join("saved 用户 project.instplot");
        fs::write(&user, b"user project must remain unchanged").unwrap();
        probe_directory_writable(&root).unwrap();
        assert_eq!(
            fs::read(user).unwrap(),
            b"user project must remain unchanged"
        );
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        assert!(probe_directory_writable(&root.join("missing")).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
