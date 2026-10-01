//! Private write-ahead installer evidence. An unresolved attempt NEVER permits
//! replay or recovery: a helper may have crashed between CreateProcess and its
//! first process checkpoint. Missing PID is not proof that nothing ran.
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::PathBuf;

use super::{VerifiedWindowsInstaller, WindowsInstallation, invalid};
use crate::update_transaction::{TransactionStore, UpdateStage, UpdateTransaction};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstallerAttemptStatus {
    Unresolved,
    Running { process_id: u32, created: u64 },
    Exited { exit_code: i32 },
    NotStarted,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Intent,
    Running,
    Exited,
    NotStarted,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Attempt {
    schema: u32,
    transaction_id: String,
    installation: PathBuf,
    installer: PathBuf,
    version: String,
    sha256: String,
    size_bytes: u64,
    phase: Phase,
    process: Option<(u32, u64)>,
    exit_code: Option<i32>,
}

pub(super) struct InstallerJournal<'a> {
    store: &'a TransactionStore,
    path: PathBuf,
    attempt: Attempt,
}

impl Attempt {
    fn expected(
        installation: &WindowsInstallation,
        installer: &VerifiedWindowsInstaller,
        transaction: &UpdateTransaction,
    ) -> io::Result<(Self, &'static str)> {
        let identity = transaction.identity();
        if identity.product != "instplot-studio"
            || identity.platform != "windows-x86_64"
            || identity.installed_path != installation.directory()
            || identity.previous_version != installation.version().to_string()
        {
            return Err(invalid("installer transaction does not match installation"));
        }
        let name = match transaction.stage() {
            UpdateStage::Applying
            | UpdateStage::AwaitingHealth
            | UpdateStage::Completed
            | UpdateStage::RecoveryRequired
                if installer.version().to_string() == identity.candidate_version
                    && installer.sha256() == identity.candidate_sha256
                    && installer.size_bytes() == identity.candidate_size =>
            {
                "apply-installer.json"
            }
            UpdateStage::Restoring | UpdateStage::RolledBack | UpdateStage::RecoveryRequired
                if installer.version() == installation.version() =>
            {
                "restore-installer.json"
            }
            _ => {
                return Err(invalid(
                    "installer asset or transaction stage does not match operation",
                ));
            }
        };
        Ok((
            Self {
                schema: 1,
                transaction_id: transaction.id().to_owned(),
                installation: installation.directory().to_owned(),
                installer: installer.path().to_owned(),
                version: installer.version().to_string(),
                sha256: installer.sha256().to_owned(),
                size_bytes: installer.size_bytes(),
                phase: Phase::Intent,
                process: None,
                exit_code: None,
            },
            name,
        ))
    }

    fn status(&self) -> io::Result<InstallerAttemptStatus> {
        match (&self.phase, self.process, self.exit_code) {
            (Phase::Intent, None, None) => Ok(InstallerAttemptStatus::Unresolved),
            (Phase::NotStarted, None, None) => Ok(InstallerAttemptStatus::NotStarted),
            (Phase::Running, Some((pid, created)), None) if pid > 0 && created > 0 => {
                Ok(InstallerAttemptStatus::Running {
                    process_id: pid,
                    created,
                })
            }
            (Phase::Exited, Some((pid, created)), Some(code)) if pid > 0 && created > 0 => {
                Ok(InstallerAttemptStatus::Exited { exit_code: code })
            }
            _ => Err(invalid("damaged installer execution checkpoint")),
        }
    }
}

impl<'a> InstallerJournal<'a> {
    pub(super) fn begin(
        store: &'a TransactionStore,
        transaction: &UpdateTransaction,
        installation: &WindowsInstallation,
        installer: &VerifiedWindowsInstaller,
    ) -> io::Result<Self> {
        if !matches!(
            transaction.stage(),
            UpdateStage::Applying | UpdateStage::Restoring
        ) {
            return Err(invalid("installer cannot launch in this transaction stage"));
        }
        let persisted = store.read(transaction.identity())?;
        if persisted.id() != transaction.id()
            || persisted.health_nonce() != transaction.health_nonce()
            || persisted.stage() != transaction.stage()
        {
            return Err(invalid("installer transaction has not been persisted"));
        }
        let (attempt, name) = Attempt::expected(installation, installer, transaction)?;
        let path = store.directory().join(name);
        // CREATE_NEW and sync BEFORE launching. Existing/partial attempts are
        // retained and refused, not repaired or interpreted as retry permission.
        let mut file = super::create_private_file(&path)?;
        file.write_all(&serde_json::to_vec(&attempt).map_err(invalid)?)?;
        file.sync_all()?;
        Ok(Self {
            store,
            path,
            attempt,
        })
    }

    fn persist(&self) -> io::Result<()> {
        self.attempt.status()?;
        self.store.require_directory(
            self.path
                .parent()
                .ok_or_else(|| invalid("missing journal parent"))?,
        )?;
        super::write_private_atomic(
            &self.path,
            &serde_json::to_vec(&self.attempt).map_err(invalid)?,
        )
    }

    pub(super) fn running(&mut self, pid: u32, created: u64) -> io::Result<()> {
        self.attempt.phase = Phase::Running;
        self.attempt.process = Some((pid, created));
        self.persist()
    }

    pub(super) fn exited(&mut self, pid: u32, created: u64, code: i32) -> io::Result<()> {
        self.attempt.phase = Phase::Exited;
        self.attempt.process = Some((pid, created));
        self.attempt.exit_code = Some(code);
        self.persist()
    }

    pub(super) fn not_started(&mut self) -> io::Result<()> {
        self.attempt.phase = Phase::NotStarted;
        self.persist()
    }

    pub(super) fn require_owned_failed_exit(
        &self,
        store: &TransactionStore,
        transaction: &UpdateTransaction,
        installation: &WindowsInstallation,
        installer: &VerifiedWindowsInstaller,
        process: (u32, u64),
        exit_code: i32,
    ) -> io::Result<()> {
        self.store.require_directory(store.directory())?;
        let persisted = read_journal(store, transaction, installation, installer)?;
        if exit_code == 0 {
            return Err(invalid(
                "successful installer exit cannot authorize failed-install recovery",
            ));
        }
        if persisted.path != self.path
            || persisted.attempt != self.attempt
            || persisted.attempt.process != Some(process)
            || persisted.attempt.status()? != (InstallerAttemptStatus::Exited { exit_code })
        {
            return Err(invalid(
                "failed installer evidence differs from retained native child",
            ));
        }
        Ok(())
    }
}

/// Read evidence under the private transaction lock. Running/Unresolved is not
/// an exit signal; a later helper must bind PID + creation time + installer path
/// before waiting. A missing/reused/inaccessible PID remains unresolved.
/// Installer exit, including code zero, does NOT prove GUI health or success.
pub fn installer_attempt_status(
    store: &TransactionStore,
    transaction: &UpdateTransaction,
    installation: &WindowsInstallation,
    installer: &VerifiedWindowsInstaller,
) -> io::Result<InstallerAttemptStatus> {
    read_journal(store, transaction, installation, installer)?
        .attempt
        .status()
}

fn read_journal<'a>(
    store: &'a TransactionStore,
    transaction: &UpdateTransaction,
    installation: &WindowsInstallation,
    installer: &VerifiedWindowsInstaller,
) -> io::Result<InstallerJournal<'a>> {
    let persisted = store.read(transaction.identity())?;
    if persisted.id() != transaction.id()
        || persisted.health_nonce() != transaction.health_nonce()
        || persisted.stage() != transaction.stage()
    {
        return Err(invalid("installer evidence transaction mismatch"));
    }
    let (mut expected, name) = Attempt::expected(installation, installer, transaction)?;
    let path = store.directory().join(name);
    super::validate_private_file(&path)?;
    let mut raw = Vec::new();
    File::open(&path)?
        .take(64 * 1024 + 1)
        .read_to_end(&mut raw)?;
    if raw.len() > 64 * 1024 {
        return Err(invalid("oversized installer evidence"));
    }
    let actual: Attempt = serde_json::from_slice(&raw).map_err(invalid)?;
    expected.phase = actual.phase.clone();
    expected.process = actual.process;
    expected.exit_code = actual.exit_code;
    if actual != expected {
        return Err(invalid("installer evidence identity mismatch"));
    }
    // The directory and file must still have their original private ACL.
    super::validate_private_directory(store.directory())?;
    super::validate_private_file(&path)?;
    actual.status()?;
    Ok(InstallerJournal {
        store,
        path,
        attempt: actual,
    })
}

/// Refresh a recorded process without killing it or replaying its installer.
/// A missing, reused or unbindable PID is an error, NOT an inferred exit.
/// The caller retains both installation access and the verified asset lease.
pub fn refresh_installer_attempt(
    store: &TransactionStore,
    transaction: &UpdateTransaction,
    installation: &WindowsInstallation,
    installer: &super::PinnedWindowsInstaller,
    access: &super::WindowsInstallAccess,
) -> io::Result<InstallerAttemptStatus> {
    access.require_exclusive(installation.directory())?;
    installer.installer().revalidate()?;
    let mut journal = read_journal(store, transaction, installation, installer.installer())?;
    let status = journal.attempt.status()?;
    let InstallerAttemptStatus::Running {
        process_id,
        created,
    } = status
    else {
        return Ok(status);
    };
    let process =
        super::TrackedWindowsProcess::bind(process_id, created, installer.installer().path())?;
    match process.exit_code_if_exited()? {
        None => Ok(status),
        Some(code) => {
            journal.exited(process_id, created, code)?;
            Ok(InstallerAttemptStatus::Exited { exit_code: code })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn uncertain_or_malformed_attempt_is_never_an_exit_signal() {
        let mut record = Attempt {
            schema: 1,
            transaction_id: "a".repeat(32),
            installation: PathBuf::from("C:\\Studio"),
            installer: PathBuf::from("C:\\cache\\setup.exe"),
            version: "0.1.2-rc.2".into(),
            sha256: "b".repeat(64),
            size_bytes: 1,
            phase: Phase::Intent,
            process: None,
            exit_code: None,
        };
        assert_eq!(record.status().unwrap(), InstallerAttemptStatus::Unresolved);
        record.phase = Phase::Running;
        assert!(record.status().is_err());
        record.process = Some((0, 1));
        assert!(record.status().is_err());
        record.process = Some((12, 34));
        assert_eq!(
            record.status().unwrap(),
            InstallerAttemptStatus::Running {
                process_id: 12,
                created: 34
            }
        );
        record.phase = Phase::Exited;
        assert!(record.status().is_err());
        record.exit_code = Some(91);
        assert_eq!(
            record.status().unwrap(),
            InstallerAttemptStatus::Exited { exit_code: 91 }
        );
    }
}
