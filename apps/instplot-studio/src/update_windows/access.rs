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
    lock: File,
    exclusive: bool,
    held: bool,
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
        if self.exclusive || !self.held {
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
                        | crate::update_transaction::UpdateStage::FailedBeforeApply
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
            lock,
            exclusive,
            held: true,
        })
    }

    pub(super) fn require_exclusive(&self, target: &Path) -> io::Result<()> {
        reject_redirected_path(target)?;
        if !self.held || !self.exclusive || self.target != std::fs::canonicalize(target)? {
            return Err(invalid(
                "exclusive access to the exact installation is required",
            ));
        }
        Ok(())
    }

    /// Parent remains open/frozen. A durable, exact WaitingForExit request must
    /// already block new ordinary startup before we briefly release our shared
    /// lock. Native exclusive acquisition then proves no other shared instance
    /// remains. Keep this guard until normal parent exit; any error denies exit.
    #[cfg(feature = "in-place-update-preview")]
    pub fn promote_for_parent_exit(
        &mut self,
        prepared: &super::PreparedWindowsHelper,
    ) -> io::Result<()> {
        let target = self.target.clone();
        self.promote_shared_with_gate(|| prepared.confirm_bound_parent_waiting(&target))
    }

    #[cfg(feature = "in-place-update-preview")]
    pub(super) fn require_parent_exit_access(
        &self,
        prepared: &super::PreparedWindowsHelper,
    ) -> io::Result<()> {
        self.require_exclusive(&self.target)?;
        prepared.confirm_bound_parent_waiting(&self.target)
    }

    #[cfg(feature = "in-place-update-preview")]
    pub(super) fn restore_parent_shared_access(
        &mut self,
        prepared: &super::PreparedWindowsHelper,
    ) -> io::Result<()> {
        let target = self.target.clone();
        self.restore_shared_with_gate(|| prepared.confirm_bound_parent_cancelled(&target))
    }

    #[cfg(any(test, feature = "in-place-update-preview"))]
    fn restore_shared_with_gate(
        &mut self,
        mut verify_cancelled: impl FnMut() -> io::Result<()>,
    ) -> io::Result<()> {
        verify_cancelled()?;
        if self.held && !self.exclusive {
            return Ok(());
        }
        if self.held {
            self.lock.unlock()?;
            self.held = false;
        }
        self.exclusive = false;
        self.lock.try_lock_shared().map_err(io::Error::from)?;
        self.held = true;
        verify_cancelled()
    }

    #[cfg(any(test, feature = "in-place-update-preview"))]
    fn promote_shared_with_gate(
        &mut self,
        mut verify_waiting: impl FnMut() -> io::Result<()>,
    ) -> io::Result<()> {
        if !self.held || self.exclusive {
            return Err(invalid(
                "parent promotion requires its held shared installation lock",
            ));
        }
        verify_waiting()?;
        self.lock.unlock()?;
        self.held = false;
        match self.lock.try_lock().map_err(io::Error::from) {
            Ok(()) => {
                self.exclusive = true;
                self.held = true;
                // Lost/corrupt waiting proof must not authorize GUI exit even
                // when the lock was acquired. Retain exclusive for inspection.
                verify_waiting()
            }
            Err(promotion) => match self.lock.try_lock_shared().map_err(io::Error::from) {
                Ok(()) => {
                    self.held = true;
                    Err(promotion)
                }
                Err(restoration) => Err(invalid(format!(
                    "parent lock promotion failed ({promotion}); shared lock restoration failed ({restoration}); keep parent frozen"
                ))),
            },
        }
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
        let prepared = state.clone();
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
        let mut aborted = prepared;
        aborted.transition(UpdateStage::FailedBeforeApply).unwrap();
        store.write(&aborted).unwrap();
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
        let mut first = WindowsInstallAccess::acquire(&target, &locks, false).unwrap();
        let second = WindowsInstallAccess::acquire(&target, &locks, false).unwrap();
        // No durable waiting proof means no unlock/promotion attempt.
        assert!(
            first
                .promote_shared_with_gate(|| Err(invalid("not waiting")))
                .is_err()
        );
        assert!(first.held && !first.exclusive);
        // The actual second instance prevents promotion; original shared
        // access must be restored, not silently dropped on failure.
        assert!(first.promote_shared_with_gate(|| Ok(())).is_err());
        assert!(first.held && !first.exclusive);
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
        let mut parent = WindowsInstallAccess::acquire(&target, &locks, false).unwrap();
        let calls = std::cell::Cell::new(0);
        parent
            .promote_shared_with_gate(|| {
                calls.set(calls.get() + 1);
                Ok(())
            })
            .unwrap();
        assert_eq!(calls.get(), 2);
        parent.require_exclusive(&target).unwrap();
        assert!(WindowsInstallAccess::acquire(&target, &locks, false).is_err());
        assert!(
            parent
                .promote_shared_with_gate(|| panic!("already promoted"))
                .is_err()
        );
        assert!(
            parent
                .restore_shared_with_gate(|| Err(invalid("not cancelled")))
                .is_err()
        );
        assert!(WindowsInstallAccess::acquire(&target, &locks, false).is_err());
        parent.restore_shared_with_gate(|| Ok(())).unwrap();
        assert!(parent.held && !parent.exclusive);
        assert!(parent.require_exclusive(&target).is_err());
        WindowsInstallAccess::acquire(&target, &locks, false).unwrap();
        parent.restore_shared_with_gate(|| Ok(())).unwrap();
        drop(parent);
        let mut parent = WindowsInstallAccess::acquire(&target, &locks, false).unwrap();
        calls.set(0);
        assert!(
            parent
                .promote_shared_with_gate(|| {
                    calls.set(calls.get() + 1);
                    if calls.get() == 2 {
                        Err(invalid("waiting proof changed"))
                    } else {
                        Ok(())
                    }
                })
                .is_err()
        );
        assert!(parent.held && parent.exclusive);
        assert!(WindowsInstallAccess::acquire(&target, &locks, false).is_err());
        drop(parent);
        std::fs::remove_dir_all(root).unwrap();
    }
}
