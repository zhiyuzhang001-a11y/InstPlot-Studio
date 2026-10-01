//! Bundle exchange primitive for the macOS updater prototype.
//!
//! This is deliberately separate from process launch and DMG verification. The
//! platform adapter must verify candidates and acquire exclusive installation
//! access before mutating files; this module never kills a process or deletes a
//! bundle. Interrupted exchanges preserve the old bundle for recovery.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::update_transaction::{TransactionStore, UpdateStage, UpdateTransaction};

pub struct BundleAccess {
    target: PathBuf,
    _lock: File,
    exclusive: bool,
    instance_path: Option<PathBuf>,
    _instance_lock: Option<File>,
}

impl BundleAccess {
    /// A running app holds shared access for its entire lifetime. The updater
    /// cannot acquire exclusive access while any supported instance is alive.
    pub fn shared(target: &Path) -> io::Result<Self> {
        Self::lock(target, false)
    }

    pub fn exclusive(target: &Path) -> io::Result<Self> {
        Self::lock(target, true)
    }

    fn lock(target: &Path, exclusive: bool) -> io::Result<Self> {
        let target = normalized_target(target)?;
        let name = format!(
            ".instplot-update-{:x}.lock",
            Sha256::digest(target.as_os_str().as_encoded_bytes())
        );
        let path = target.parent().unwrap().join(name);
        reject_non_file(&path)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let lock = options.open(path)?;
        if exclusive {
            lock.try_lock()
        } else {
            lock.try_lock_shared()
        }
        .map_err(io::Error::from)?;
        let (instance_path, instance_lock) = if !exclusive {
            let mut random = [0_u8; 16];
            getrandom::fill(&mut random).map_err(|e| io::Error::other(e.to_string()))?;
            let prefix = instance_prefix(&target);
            let path = target
                .parent()
                .unwrap()
                .join(format!("{prefix}{:x}", Sha256::digest(random)));
            let mut instance_options = OpenOptions::new();
            instance_options.read(true).write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                instance_options.mode(0o600);
            }
            let instance_lock = instance_options.open(&path)?;
            instance_lock
                .try_lock()
                .map_err(|e| io::Error::other(e.to_string()))?;
            (Some(path), Some(instance_lock))
        } else {
            (None, None)
        };
        Ok(Self {
            target,
            _lock: lock,
            exclusive,
            instance_path,
            _instance_lock: instance_lock,
        })
    }

    pub fn other_instances(&self) -> io::Result<bool> {
        let prefix = instance_prefix(&self.target);
        for item in fs::read_dir(self.target.parent().unwrap())? {
            let item = item?;
            if !item.file_name().to_string_lossy().starts_with(&prefix)
                || self.instance_path.as_ref() == Some(&item.path())
            {
                continue;
            }
            reject_non_file(&item.path())?;
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(item.path())?;
            if file.try_lock().is_err() {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

impl Drop for BundleAccess {
    fn drop(&mut self) {
        if let Some(path) = self.instance_path.take() {
            self._instance_lock.take();
            // Only this randomly named marker is removed, never installation files.
            let _ = fs::remove_file(path);
        }
    }
}

fn instance_prefix(target: &Path) -> String {
    format!(
        ".instplot-instance-{:x}-",
        Sha256::digest(target.as_os_str().as_encoded_bytes())
    )
}

pub struct BundleExchange<'a> {
    store: &'a TransactionStore,
    state: UpdateTransaction,
    target: PathBuf,
    candidate: PathBuf,
    backup: PathBuf,
    failed_candidate: PathBuf,
}

impl<'a> BundleExchange<'a> {
    /// Staging must be a private sibling of the installation, so each rename
    /// stays on the same filesystem. Paths are fixed, not read as shell commands.
    pub fn open(
        store: &'a TransactionStore,
        state: UpdateTransaction,
        staging: &Path,
    ) -> io::Result<Self> {
        let target = normalized_target(&state.identity().installed_path)?;
        let staging = fs::canonicalize(staging)?;
        if staging.parent() != target.parent() || state.identity().platform != "macos-aarch64" {
            return Err(invalid(
                "bundle staging must be a sibling of the macOS installation",
            ));
        }
        // Require the caller's staging directory to be the actual locked store.
        store.require_directory(&staging)?;
        Ok(Self {
            store,
            state,
            target,
            candidate: staging.join("candidate.app"),
            backup: staging.join("previous.app"),
            failed_candidate: staging.join("failed-candidate.app"),
        })
    }

    pub fn state(&self) -> &UpdateTransaction {
        &self.state
    }

    /// Callback must check product identity, executable version, architecture,
    /// signature integrity and trusted candidate evidence, not just existence.
    pub fn apply(
        &mut self,
        access: &BundleAccess,
        verify: impl Fn(&Path, &str) -> io::Result<()>,
    ) -> io::Result<()> {
        self.require_exclusive(access)?;
        if self.state.stage() != UpdateStage::WaitingForExit {
            return Err(invalid("bundle is not ready to apply"));
        }
        require_bundle(&self.target)?;
        require_bundle(&self.candidate)?;
        if self.backup.try_exists()? || self.failed_candidate.try_exists()? {
            return Err(invalid(
                "backup path already exists; inspect previous transaction",
            ));
        }
        verify(&self.target, &self.state.identity().previous_version)?;
        verify(&self.candidate, &self.state.identity().candidate_version)?;
        // Persist intent before either rename. Both intermediate states can be
        // recognized without treating two renames as one atomic operation.
        self.state.transition(UpdateStage::Applying)?;
        self.store.write(&self.state)?;
        if let Err(error) = fs::rename(&self.target, &self.backup)
            .and_then(|()| fs::rename(&self.candidate, &self.target))
        {
            self.state.record_error(&error.to_string());
            self.state.transition(UpdateStage::RecoveryRequired)?;
            self.store.write(&self.state)?;
            return Err(error);
        }
        sync_parent(&self.target)?;
        Ok(())
    }

    pub fn await_health(&mut self, pid: u32, started: String) -> io::Result<()> {
        self.state.await_health(pid, started)?;
        self.store.write(&self.state)
    }

    pub fn accept_health(
        &mut self,
        receipt: &crate::update_transaction::HealthReceipt,
    ) -> io::Result<()> {
        self.state.accept_health(receipt)?;
        self.store.write(&self.state)
    }

    /// Restoration requires exclusive access too. A still-running candidate
    /// blocks this call rather than being killed or overwritten.
    pub fn restore(
        &mut self,
        access: &BundleAccess,
        verify: impl Fn(&Path, &str) -> io::Result<()>,
    ) -> io::Result<()> {
        self.require_exclusive(access)?;
        if !self.state.stage().needs_recovery_inspection() {
            return Err(invalid("transaction does not need recovery"));
        }
        if self.state.stage() != UpdateStage::RecoveryRequired {
            self.state.transition(UpdateStage::RecoveryRequired)?;
        }
        self.store.write(&self.state)?;
        // Inspect both sides before any mutation; ambiguous states fail closed.
        if self.backup.try_exists()? {
            require_bundle(&self.backup)?;
            verify(&self.backup, &self.state.identity().previous_version)?;
            if self.target.try_exists()? {
                require_bundle(&self.target)?;
                verify(&self.target, &self.state.identity().candidate_version)?;
                if self.failed_candidate.try_exists()? {
                    return Err(invalid("failed candidate path already exists"));
                }
            }
            self.state.transition(UpdateStage::Restoring)?;
            self.store.write(&self.state)?;
            if self.target.try_exists()? {
                fs::rename(&self.target, &self.failed_candidate)?;
            }
            fs::rename(&self.backup, &self.target)?;
        } else {
            // Crash before the first rename, or after restoration but before
            // its final journal write: the original bundle must verify in place.
            require_bundle(&self.target)?;
            verify(&self.target, &self.state.identity().previous_version)?;
            self.state.transition(UpdateStage::Restoring)?;
            self.store.write(&self.state)?;
        }
        sync_parent(&self.target)?;
        verify(&self.target, &self.state.identity().previous_version)?;
        self.state.transition(UpdateStage::RolledBack)?;
        self.store.write(&self.state)
    }

    fn require_exclusive(&self, access: &BundleAccess) -> io::Result<()> {
        if !access.exclusive || access.target != self.target {
            Err(invalid("exclusive access to this installation is required"))
        } else {
            Ok(())
        }
    }
}

fn normalized_target(target: &Path) -> io::Result<PathBuf> {
    if !target.is_absolute() || target.extension().is_none_or(|ext| ext != "app") {
        return Err(invalid(
            "installation target must be an absolute app bundle",
        ));
    }
    let parent = target
        .parent()
        .ok_or_else(|| invalid("missing bundle parent"))?;
    let parent = fs::canonicalize(parent)?;
    let name = target
        .file_name()
        .ok_or_else(|| invalid("missing bundle name"))?;
    let canonical = parent.join(name);
    if canonical != target {
        return Err(invalid("installation parent must already be canonical"));
    }
    if fs::symlink_metadata(target).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(invalid("installation cannot be a symlink"));
    }
    Ok(canonical)
}

fn require_bundle(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        Err(invalid("bundle must be a real directory"))
    } else {
        Ok(())
    }
}

fn reject_non_file(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.is_file() || metadata.file_type().is_symlink() => {
            Err(invalid("invalid installation lock"))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn sync_parent(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        File::open(path.parent().unwrap())?.sync_all()?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update_transaction::UpdateIdentity;

    struct Fixture {
        root: PathBuf,
        target: PathBuf,
        staging: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let mut random = [0_u8; 12];
            getrandom::fill(&mut random).unwrap();
            let root = std::env::temp_dir()
                .join(format!("instplot-bundle-test-{:x}", Sha256::digest(random)));
            fs::create_dir(&root).unwrap();
            let root = fs::canonicalize(root).unwrap();
            let target = root.join("InstPlot Studio.app");
            let staging = root.join("transaction");
            fs::create_dir(&staging).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&staging, fs::Permissions::from_mode(0o700)).unwrap();
            }
            for (path, version) in [
                (target.clone(), "0.1.3-rc.1"),
                (staging.join("candidate.app"), "0.1.3-rc.2"),
            ] {
                fs::create_dir(&path).unwrap();
                fs::write(path.join("identity"), version).unwrap();
            }
            Self {
                root,
                target,
                staging,
            }
        }
        fn state(&self) -> UpdateTransaction {
            let mut state = UpdateTransaction::new(UpdateIdentity {
                product: "instplot-studio".into(),
                platform: "macos-aarch64".into(),
                installed_path: self.target.clone(),
                previous_version: "0.1.3-rc.1".into(),
                candidate_version: "0.1.3-rc.2".into(),
                candidate_sha256: "a".repeat(64),
                candidate_size: 1,
            })
            .unwrap();
            state.transition(UpdateStage::WaitingForExit).unwrap();
            state
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    fn verify(path: &Path, version: &str) -> io::Result<()> {
        if fs::read_to_string(path.join("identity"))? == version {
            Ok(())
        } else {
            Err(invalid("wrong bundle"))
        }
    }

    #[test]
    fn exchange_keeps_backup_and_can_restore_after_failed_health() {
        let fixture = Fixture::new();
        let store = TransactionStore::lock(&fixture.staging).unwrap();
        let access = BundleAccess::exclusive(&fixture.target).unwrap();
        let mut exchange = BundleExchange::open(&store, fixture.state(), &fixture.staging).unwrap();
        exchange.apply(&access, verify).unwrap();
        verify(&fixture.target, "0.1.3-rc.2").unwrap();
        verify(&fixture.staging.join("previous.app"), "0.1.3-rc.1").unwrap();
        exchange
            .await_health(12, "candidate-created".into())
            .unwrap();
        exchange.restore(&access, verify).unwrap();
        verify(&fixture.target, "0.1.3-rc.1").unwrap();
        assert_eq!(exchange.state().stage(), UpdateStage::RolledBack);
        assert!(fixture.staging.join("failed-candidate.app").is_dir());
    }

    #[test]
    fn interrupted_between_renames_restores_old_bundle() {
        let fixture = Fixture::new();
        let store = TransactionStore::lock(&fixture.staging).unwrap();
        let access = BundleAccess::exclusive(&fixture.target).unwrap();
        let mut state = fixture.state();
        state.transition(UpdateStage::Applying).unwrap();
        store.write(&state).unwrap();
        fs::rename(&fixture.target, fixture.staging.join("previous.app")).unwrap();
        let state = store.read(state.identity()).unwrap();
        let mut exchange = BundleExchange::open(&store, state, &fixture.staging).unwrap();
        exchange.restore(&access, verify).unwrap();
        verify(&fixture.target, "0.1.3-rc.1").unwrap();
    }

    #[test]
    fn live_instance_blocks_exchange_and_wrong_candidate_changes_nothing() {
        let fixture = Fixture::new();
        let live = BundleAccess::shared(&fixture.target).unwrap();
        assert!(!live.other_instances().unwrap());
        let another = BundleAccess::shared(&fixture.target).unwrap();
        assert!(live.other_instances().unwrap());
        assert!(another.other_instances().unwrap());
        drop(another);
        assert!(!live.other_instances().unwrap());
        assert!(BundleAccess::exclusive(&fixture.target).is_err());
        drop(live);
        let store = TransactionStore::lock(&fixture.staging).unwrap();
        let access = BundleAccess::exclusive(&fixture.target).unwrap();
        fs::write(fixture.staging.join("candidate.app/identity"), "wrong").unwrap();
        let mut exchange = BundleExchange::open(&store, fixture.state(), &fixture.staging).unwrap();
        assert!(exchange.apply(&access, verify).is_err());
        verify(&fixture.target, "0.1.3-rc.1").unwrap();
        assert!(!fixture.staging.join("previous.app").exists());
    }
}
