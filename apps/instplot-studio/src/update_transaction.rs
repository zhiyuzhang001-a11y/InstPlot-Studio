//! Durable, platform-independent update contracts. No installer is launched here.
//!
//! Applying an update remains a platform stage gate: a verified download alone
//! must never cause an application to exit or imply that installation succeeded.

use std::fs::{self, File, OpenOptions};
use std::io;
#[cfg(not(windows))]
use std::io::Write;
use std::path::{Path, PathBuf};

#[cfg(not(windows))]
use atomicwrites::{AllowOverwrite, AtomicFile};
use semver::Version;
use serde::{Deserialize, Serialize};

const SCHEMA: u32 = 1;
const MAX_STATE_BYTES: u64 = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateStage {
    Prepared,
    WaitingForExit,
    Applying,
    AwaitingHealth,
    Completed,
    RecoveryRequired,
    Restoring,
    RolledBack,
    FailedBeforeApply,
}

impl UpdateStage {
    fn permits(self, next: Self) -> bool {
        matches!(
            (self, next),
            (
                Self::Prepared,
                Self::WaitingForExit | Self::FailedBeforeApply
            ) | (
                Self::WaitingForExit,
                Self::Applying | Self::FailedBeforeApply
            ) | (
                Self::Applying,
                Self::AwaitingHealth | Self::RecoveryRequired
            ) | (
                Self::AwaitingHealth,
                Self::Completed | Self::RecoveryRequired
            ) | (Self::RecoveryRequired, Self::Restoring)
                | (Self::Restoring, Self::RolledBack | Self::RecoveryRequired)
        )
    }

    /// A helper interrupted here cannot assume that installed files are intact.
    pub fn needs_recovery_inspection(self) -> bool {
        matches!(
            self,
            Self::Applying | Self::AwaitingHealth | Self::RecoveryRequired | Self::Restoring
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateIdentity {
    pub product: String,
    pub platform: String,
    pub installed_path: PathBuf,
    pub previous_version: String,
    pub candidate_version: String,
    pub candidate_sha256: String,
    pub candidate_size: u64,
}

impl UpdateIdentity {
    fn validate(&self) -> io::Result<()> {
        let previous = Version::parse(&self.previous_version).map_err(invalid)?;
        let candidate = Version::parse(&self.candidate_version).map_err(invalid)?;
        if self.product != "instplot-studio"
            || !matches!(self.platform.as_str(), "macos-aarch64" | "windows-x86_64")
            || !self.installed_path.is_absolute()
            || self.installed_path.parent().is_none()
            || self.installed_path.file_name().is_none()
            || self.installed_path.components().any(|part| {
                matches!(
                    part,
                    std::path::Component::ParentDir | std::path::Component::CurDir
                )
            })
            || candidate <= previous
            || previous.pre.is_empty() != candidate.pre.is_empty()
            || !is_hex(&self.candidate_sha256, 64)
            || self.candidate_size == 0
            || self.candidate_size > 2 * 1024 * 1024 * 1024
        {
            return Err(invalid("invalid update identity"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HealthReceipt {
    pub transaction_id: String,
    pub nonce: String,
    pub product: String,
    pub platform: String,
    pub version: String,
    pub installed_path: PathBuf,
    pub process_id: u32,
    pub process_started: String,
    pub initialized: bool,
    pub window_ready: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateTransaction {
    schema: u32,
    id: String,
    nonce: String,
    identity: UpdateIdentity,
    stage: UpdateStage,
    candidate_process: Option<(u32, String)>,
    last_error: Option<String>,
}

impl UpdateTransaction {
    pub fn new(identity: UpdateIdentity) -> io::Result<Self> {
        let mut random = [0_u8; 48];
        getrandom::fill(&mut random).map_err(|error| io::Error::other(error.to_string()))?;
        let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
        Self::prepared(hex(&random[..16]), hex(&random[16..]), identity)
    }

    /// The caller must supply OS-generated random values, not timestamps or PIDs.
    pub fn prepared(id: String, nonce: String, identity: UpdateIdentity) -> io::Result<Self> {
        identity.validate()?;
        if !is_hex(&id, 32) || !is_hex(&nonce, 64) {
            return Err(invalid("invalid transaction token"));
        }
        Ok(Self {
            schema: SCHEMA,
            id,
            nonce,
            identity,
            stage: UpdateStage::Prepared,
            candidate_process: None,
            last_error: None,
        })
    }

    pub fn identity(&self) -> &UpdateIdentity {
        &self.identity
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn stage(&self) -> UpdateStage {
        self.stage
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// Used only to build a private, single-use candidate startup handshake.
    pub fn health_nonce(&self) -> &str {
        &self.nonce
    }

    pub fn transition(&mut self, next: UpdateStage) -> io::Result<()> {
        if !self.stage.permits(next) || next == UpdateStage::Completed {
            return Err(invalid(
                "invalid update transition; completion requires health receipt",
            ));
        }
        self.stage = next;
        Ok(())
    }

    pub fn await_health(&mut self, pid: u32, started: String) -> io::Result<()> {
        if pid == 0 || started.is_empty() {
            return Err(invalid("candidate process identity is required"));
        }
        self.transition(UpdateStage::AwaitingHealth)?;
        self.candidate_process = Some((pid, started));
        Ok(())
    }

    pub fn accept_health(&mut self, receipt: &HealthReceipt) -> io::Result<()> {
        let expected_process = Some((receipt.process_id, receipt.process_started.clone()));
        if self.stage != UpdateStage::AwaitingHealth
            || receipt.transaction_id != self.id
            || receipt.nonce != self.nonce
            || receipt.product != self.identity.product
            || receipt.platform != self.identity.platform
            || receipt.version != self.identity.candidate_version
            || receipt.installed_path != self.identity.installed_path
            || expected_process != self.candidate_process
            || !receipt.initialized
            || !receipt.window_ready
        {
            return Err(invalid("unmatched or incomplete update health receipt"));
        }
        self.stage = UpdateStage::Completed;
        Ok(())
    }

    pub fn record_error(&mut self, message: &str) {
        // Error logs must not persist unbounded output or the health nonce.
        self.last_error = Some(
            message
                .replace(&self.nonce, "[redacted]")
                .chars()
                .take(2048)
                .collect(),
        );
    }

    fn validate(&self) -> io::Result<()> {
        self.identity.validate()?;
        if self.schema != SCHEMA || !is_hex(&self.id, 32) || !is_hex(&self.nonce, 64) {
            return Err(invalid("unsupported or damaged update transaction"));
        }
        if matches!(
            self.stage,
            UpdateStage::AwaitingHealth | UpdateStage::Completed
        ) && !self
            .candidate_process
            .as_ref()
            .is_some_and(|(pid, start)| *pid > 0 && !start.is_empty())
        {
            return Err(invalid("missing candidate process identity"));
        }
        Ok(())
    }
}

/// Keeps the cross-process transaction lock alive and atomically persists state.
pub struct TransactionStore {
    directory: PathBuf,
    _lock: File,
}

impl TransactionStore {
    pub(crate) fn require_directory(&self, directory: &Path) -> io::Result<()> {
        if fs::canonicalize(&self.directory)? == directory {
            Ok(())
        } else {
            Err(invalid(
                "staging directory does not match locked transaction",
            ))
        }
    }
    /// The directory must already belong to this updater, inside its private cache.
    pub fn lock(directory: &Path) -> io::Result<Self> {
        let metadata = fs::symlink_metadata(directory)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(invalid("transaction directory must be a real directory"));
        }
        #[cfg(windows)]
        crate::update_windows::validate_private_directory(directory)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o077 != 0 {
                return Err(invalid("transaction directory must be private"));
            }
        }
        let lock_path = directory.join("transaction.lock");
        reject_link(&lock_path)?;
        #[cfg(windows)]
        if lock_path.exists() {
            crate::update_windows::validate_private_file(&lock_path)?;
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        #[cfg(not(windows))]
        let lock = options.open(&lock_path)?;
        #[cfg(windows)]
        let lock = if lock_path.exists() {
            options.open(&lock_path)?
        } else {
            crate::update_windows::create_private_file(&lock_path)?
        };
        #[cfg(windows)]
        crate::update_windows::validate_private_file(&lock_path)?;
        lock.try_lock()
            .map_err(|error| io::Error::other(error.to_string()))?;
        Ok(Self {
            directory: directory.to_path_buf(),
            _lock: lock,
        })
    }

    pub fn write(&self, transaction: &UpdateTransaction) -> io::Result<()> {
        transaction.validate()?;
        #[cfg(windows)]
        crate::update_windows::validate_private_directory(&self.directory)?;
        let path = self.directory.join("transaction.json");
        reject_link(&path)?;
        #[cfg(windows)]
        if path.exists() {
            crate::update_windows::validate_private_file(&path)?;
        }
        let raw = serde_json::to_vec(transaction).map_err(invalid)?;
        if raw.len() as u64 > MAX_STATE_BYTES {
            return Err(invalid("transaction state is too large"));
        }
        #[cfg(windows)]
        {
            crate::update_windows::write_private_atomic(&path, &raw)
        }
        #[cfg(not(windows))]
        {
            AtomicFile::new(&path, AllowOverwrite)
                .write(|file| {
                    file.write_all(&raw)?;
                    file.sync_all()
                })
                .map_err(io::Error::other)
        }
    }

    pub fn read(&self, identity: &UpdateIdentity) -> io::Result<UpdateTransaction> {
        #[cfg(windows)]
        crate::update_windows::validate_private_directory(&self.directory)?;
        let path = self.directory.join("transaction.json");
        reject_link(&path)?;
        #[cfg(windows)]
        crate::update_windows::validate_private_file(&path)?;
        if fs::metadata(&path)?.len() > MAX_STATE_BYTES {
            return Err(invalid("transaction state is too large"));
        }
        let state: UpdateTransaction = serde_json::from_slice(&fs::read(path)?).map_err(invalid)?;
        state.validate()?;
        if state.identity != *identity {
            return Err(invalid("transaction belongs to a different installation"));
        }
        Ok(state)
    }
}

fn reject_link(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.is_file() || metadata.file_type().is_symlink() => {
            Err(invalid("update state must be a regular file"))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn is_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn invalid(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> UpdateIdentity {
        UpdateIdentity {
            product: "instplot-studio".into(),
            platform: "macos-aarch64".into(),
            installed_path: std::env::temp_dir().join("InstPlot Studio.app"),
            previous_version: "0.1.3-rc.1".into(),
            candidate_version: "0.1.3-rc.2".into(),
            candidate_sha256: "a".repeat(64),
            candidate_size: 12,
        }
    }

    fn transaction() -> UpdateTransaction {
        UpdateTransaction::prepared("b".repeat(32), "c".repeat(64), identity()).unwrap()
    }

    fn receipt() -> HealthReceipt {
        HealthReceipt {
            transaction_id: "b".repeat(32),
            nonce: "c".repeat(64),
            product: "instplot-studio".into(),
            platform: "macos-aarch64".into(),
            version: "0.1.3-rc.2".into(),
            installed_path: identity().installed_path,
            process_id: 24,
            process_started: "start-identity".into(),
            initialized: true,
            window_ready: true,
        }
    }

    #[test]
    fn completion_requires_matching_initialized_process() {
        let mut state = transaction();
        assert!(state.transition(UpdateStage::Applying).is_err());
        state.transition(UpdateStage::WaitingForExit).unwrap();
        state.transition(UpdateStage::Applying).unwrap();
        state.await_health(24, "start-identity".into()).unwrap();
        assert!(state.transition(UpdateStage::Completed).is_err());
        let valid = receipt();
        for mutate in [
            |r: &mut HealthReceipt| r.process_id = 42,
            |r: &mut HealthReceipt| r.process_started = "reused-pid".into(),
            |r: &mut HealthReceipt| r.platform = "windows-x86_64".into(),
            |r: &mut HealthReceipt| r.installed_path = std::env::temp_dir().join("wrong.app"),
            |r: &mut HealthReceipt| r.version = "0.1.3-rc.1".into(),
            |r: &mut HealthReceipt| r.transaction_id = "d".repeat(32),
        ] {
            let mut bad = valid.clone();
            mutate(&mut bad);
            assert!(state.accept_health(&bad).is_err());
        }
        let mut bad = valid.clone();
        bad.nonce = "d".repeat(64);
        assert!(state.accept_health(&bad).is_err());
        let mut bad = valid.clone();
        bad.window_ready = false;
        assert!(state.accept_health(&bad).is_err());
        state.accept_health(&valid).unwrap();
        assert_eq!(state.stage(), UpdateStage::Completed);
        assert!(state.transition(UpdateStage::Restoring).is_err());
    }

    #[test]
    fn recovery_is_explicit_and_never_reports_success() {
        let mut state = transaction();
        state.transition(UpdateStage::WaitingForExit).unwrap();
        state.transition(UpdateStage::Applying).unwrap();
        assert!(state.stage().needs_recovery_inspection());
        state.transition(UpdateStage::RecoveryRequired).unwrap();
        state.transition(UpdateStage::Restoring).unwrap();
        state.transition(UpdateStage::RecoveryRequired).unwrap();
        state.transition(UpdateStage::Restoring).unwrap();
        state.transition(UpdateStage::RolledBack).unwrap();
        assert!(state.transition(UpdateStage::Applying).is_err());
    }

    #[test]
    fn rejects_wrong_product_channel_and_unbounded_path() {
        for mutate in [
            |id: &mut UpdateIdentity| id.product = "instplot-lite".into(),
            |id: &mut UpdateIdentity| id.candidate_version = "0.1.3".into(),
            |id: &mut UpdateIdentity| id.candidate_version = "0.1.2-rc.1".into(),
            |id: &mut UpdateIdentity| id.installed_path = PathBuf::from("/"),
            |id: &mut UpdateIdentity| id.installed_path = PathBuf::from("relative"),
        ] {
            let mut id = identity();
            mutate(&mut id);
            assert!(UpdateTransaction::prepared("b".repeat(32), "c".repeat(64), id).is_err());
        }
    }

    #[test]
    fn durable_store_is_exclusive_and_fails_closed() {
        let path =
            std::env::temp_dir().join(format!("instplot-update-state-{}", std::process::id()));
        #[cfg(not(windows))]
        fs::create_dir(&path).unwrap();
        #[cfg(windows)]
        crate::update_windows::create_private_directory(&path).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let store = TransactionStore::lock(&path).unwrap();
        assert!(TransactionStore::lock(&path).is_err());
        let state = transaction();
        store.write(&state).unwrap();
        assert_eq!(
            store.read(&identity()).unwrap().stage(),
            UpdateStage::Prepared
        );
        let mut other = identity();
        other.installed_path = std::env::temp_dir().join("other.app");
        assert!(store.read(&other).is_err());
        fs::write(path.join("transaction.json"), b"truncated").unwrap();
        assert!(store.read(&identity()).is_err());
        drop(store);
        fs::remove_dir_all(path).unwrap();
    }
}
