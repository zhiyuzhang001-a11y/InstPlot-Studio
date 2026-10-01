//! Cross-process access for one Windows installation, in owner-only cache.
//! Applications retain shared access; the external helper requires exclusive.
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::{create_private_directory, create_private_file, invalid, reject_redirected_path};
use super::{validate_private_directory, validate_private_file};

pub struct WindowsInstallAccess {
    target: PathBuf,
    _lock: File,
    exclusive: bool,
}

impl WindowsInstallAccess {
    pub fn shared(target: &Path) -> io::Result<Self> {
        Self::acquire(target, &Self::locks_root()?, false)
    }

    pub fn exclusive(target: &Path) -> io::Result<Self> {
        Self::acquire(target, &Self::locks_root()?, true)
    }

    fn locks_root() -> io::Result<PathBuf> {
        let root = super::native::updater_private_root()?.join("installation-locks");
        if let Err(error) = create_private_directory(&root)
            && validate_private_directory(&root).is_err()
        {
            return Err(error);
        }
        Ok(root)
    }

    // Private adapter for isolated tests; production callers cannot choose a
    // second lock root to bypass another supported instance's shared access.
    pub(super) fn acquire(target: &Path, locks_root: &Path, exclusive: bool) -> io::Result<Self> {
        reject_redirected_path(target)?;
        let target = std::fs::canonicalize(target)?;
        if !target.is_dir() {
            return Err(invalid("installation access requires a directory"));
        }
        validate_private_directory(locks_root)?;
        // All supported instances use the same private root. Encoding the
        // canonical Unicode path prevents collisions with another installation.
        use std::os::windows::ffi::OsStrExt;
        let mut hash = Sha256::new();
        for unit in target.as_os_str().encode_wide() {
            hash.update(unit.to_le_bytes());
        }
        let directory = locks_root.join(format!("{:x}", hash.finalize()));
        // Concurrent creators may race. Only accept the winner after checking
        // its owner/ACL; never repair permissions of an existing directory.
        match create_private_directory(&directory) {
            Ok(()) => {}
            Err(error) => {
                if validate_private_directory(&directory).is_err() {
                    return Err(error);
                }
            }
        }
        let path = directory.join("installation.lock");
        let lock = match create_private_file(&path) {
            Ok(file) => file,
            Err(error) => {
                if validate_private_file(&path).is_err() {
                    return Err(error);
                }
                std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&path)?
            }
        };
        validate_private_directory(&directory)?;
        validate_private_file(&path)?;
        if exclusive {
            lock.try_lock()
        } else {
            lock.try_lock_shared()
        }
        .map_err(io::Error::from)?;
        Ok(Self {
            target,
            _lock: lock,
            exclusive,
        })
    }

    pub(super) fn require_exclusive(&self, target: &Path) -> io::Result<()> {
        reject_redirected_path(target)?;
        if !self.exclusive || self.target != std::fs::canonicalize(target)? {
            return Err(invalid(
                "exclusive access to the exact installation is required",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_instances_must_release_shared_access_before_installation() {
        let mut random = [0; 16];
        getrandom::fill(&mut random).unwrap();
        let root = std::env::temp_dir().join(format!(
            "studio-install-lock-{:032x}",
            u128::from_le_bytes(random)
        ));
        std::fs::create_dir(&root).unwrap();
        let target = root.join("安装 路径");
        let other = root.join("other");
        std::fs::create_dir(&target).unwrap();
        std::fs::create_dir(&other).unwrap();
        let locks = root.join("private-locks");
        create_private_directory(&locks).unwrap();
        let first = WindowsInstallAccess::acquire(&target, &locks, false).unwrap();
        let second = WindowsInstallAccess::acquire(&target, &locks, false).unwrap();
        assert!(WindowsInstallAccess::acquire(&target, &locks, true).is_err());
        assert!(first.require_exclusive(&target).is_err());
        drop(first);
        assert!(WindowsInstallAccess::acquire(&target, &locks, true).is_err());
        drop(second);
        let exclusive = WindowsInstallAccess::acquire(&target, &locks, true).unwrap();
        exclusive.require_exclusive(&target).unwrap();
        assert!(exclusive.require_exclusive(&other).is_err());
        assert!(WindowsInstallAccess::acquire(&target, &locks, false).is_err());
        // Another installation must not share this installation's lock.
        let independent = WindowsInstallAccess::acquire(&other, &locks, true).unwrap();
        drop(independent);
        drop(exclusive);
        WindowsInstallAccess::acquire(&target, &locks, false).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
}
