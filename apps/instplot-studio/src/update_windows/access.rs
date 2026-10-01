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

    /// Ordinary preview GUIs must not enter while an update has released its
    /// exclusive lock only to let the authenticated candidate initialize.
    /// Retain the shared guard during this check: an installer cannot race it.
    pub fn require_idle_startup(&self) -> io::Result<()> {
        self.require_idle_in(&super::native::updater_private_root()?.join("transactions"))
    }

    fn require_idle_in(&self, root: &Path) -> io::Result<()> {
        if self.exclusive {
            return Err(invalid(
                "ordinary startup requires shared installation access",
            ));
        }
        match std::fs::symlink_metadata(root) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
            Ok(_) => validate_private_directory(root)?,
        }
        // Never follow an untrusted journal or silently skip a corrupt entry.
        // A bounded scan preserves old evidence without requiring its deletion.
        for (index, entry) in std::fs::read_dir(root)?.enumerate() {
            if index >= 4096 {
                return Err(invalid(
                    "too many update transactions; manual review required",
                ));
            }
            let entry = entry?;
            let name = entry.file_name();
            let name = name
                .to_str()
                .ok_or_else(|| invalid("invalid transaction name"))?;
            if name.len() != 32
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err(invalid("unexpected update transaction entry"));
            }
            let state = crate::update_transaction::read_validated_snapshot(&entry.path())?;
            if state.id() != name || state.identity().platform != "windows-x86_64" {
                return Err(invalid("update journal directory identity mismatch"));
            }
            if state.identity().installed_path == self.target
                && !matches!(
                    state.stage(),
                    crate::update_transaction::UpdateStage::Prepared
                        | crate::update_transaction::UpdateStage::Completed
                        | crate::update_transaction::UpdateStage::RolledBack
                )
            {
                return Err(invalid("in-place update or recovery is unfinished"));
            }
        }
        Ok(())
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
    fn ordinary_startup_rejects_unfinished_or_corrupt_journals() {
        use crate::update_transaction::{
            TransactionStore, UpdateIdentity, UpdateStage, UpdateTransaction,
        };
        let mut random = [0; 16];
        getrandom::fill(&mut random).unwrap();
        let fixture = std::env::temp_dir().join(format!(
            "studio-startup-gate-{:032x}",
            u128::from_le_bytes(random)
        ));
        std::fs::create_dir(&fixture).unwrap();
        let target = fixture.join("安装 路径");
        let other = fixture.join("other");
        std::fs::create_dir(&target).unwrap();
        std::fs::create_dir(&other).unwrap();
        let locks = fixture.join("locks");
        create_private_directory(&locks).unwrap();
        let guard = WindowsInstallAccess::acquire(&target, &locks, false).unwrap();
        let independent = WindowsInstallAccess::acquire(&other, &locks, false).unwrap();
        let root = fixture.join("transactions");
        guard.require_idle_in(&root).unwrap();
        create_private_directory(&root).unwrap();
        let mut state = UpdateTransaction::new(UpdateIdentity {
            product: "instplot-studio".into(),
            platform: "windows-x86_64".into(),
            installed_path: std::fs::canonicalize(&target).unwrap(),
            previous_version: "0.1.2-rc.2".into(),
            candidate_version: "0.1.2-rc.3".into(),
            candidate_sha256: "a".repeat(64),
            candidate_size: 20,
        })
        .unwrap();
        let directory = root.join(state.id());
        create_private_directory(&directory).unwrap();
        let store = TransactionStore::lock(&directory).unwrap();
        store.write(&state).unwrap();
        guard.require_idle_in(&root).unwrap();
        for stage in [
            UpdateStage::WaitingForExit,
            UpdateStage::Applying,
            UpdateStage::RecoveryRequired,
            UpdateStage::Restoring,
        ] {
            state.transition(stage).unwrap();
            store.write(&state).unwrap();
            assert!(guard.require_idle_in(&root).is_err());
            independent.require_idle_in(&root).unwrap();
            if stage == UpdateStage::Applying {
                let mut awaiting = state.clone();
                awaiting
                    .await_health(1, "native-process-creation".into())
                    .unwrap();
                store.write(&awaiting).unwrap();
                assert!(guard.require_idle_in(&root).is_err());
                independent.require_idle_in(&root).unwrap();
                store.write(&state).unwrap();
            }
        }
        state.transition(UpdateStage::RolledBack).unwrap();
        store.write(&state).unwrap();
        guard.require_idle_in(&root).unwrap();
        std::fs::write(directory.join("transaction.json"), b"corrupt").unwrap();
        assert!(guard.require_idle_in(&root).is_err());
        assert!(independent.require_idle_in(&root).is_err());
        drop(store);
        drop(guard);
        drop(independent);
        std::fs::remove_dir_all(fixture).unwrap();
    }

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
