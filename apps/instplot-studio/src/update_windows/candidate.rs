//! Write-ahead single-launch and native health witness. Does not spawn a GUI.
//! The future helper must retain its Child on every checkpoint error and must
//! never retry a launch when this reservation already exists.
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::windows::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::process::Child;
use std::time::{Duration, Instant};
use windows::Win32::Storage::FileSystem::FILE_SHARE_READ;

use serde::{Deserialize, Serialize};

use super::{
    InstallerAttemptStatus, PinnedWindowsInstaller, TrackedWindowsProcess, WindowsInstallAccess,
    WindowsInstallation, invalid,
};
use crate::update_transaction::{
    HealthReceipt, TransactionStore, UpdateIdentity, UpdateStage, UpdateTransaction,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LaunchRecord {
    schema: u32,
    transaction_id: String,
    nonce: String,
    identity: UpdateIdentity,
    executable: PathBuf,
    binary_sha256: String,
    // None is deliberately ambiguous: crash before/after CreateProcess cannot
    // be distinguished. It never authorizes another launch or restoration.
    process: Option<(u32, u64)>,
}

impl LaunchRecord {
    fn expected(
        transaction: &UpdateTransaction,
        executable: PathBuf,
        binary_sha256: String,
    ) -> io::Result<Self> {
        if transaction.identity().platform != "windows-x86_64"
            || transaction.stage() != UpdateStage::Applying
        {
            return Err(invalid("candidate launch requires Windows applying stage"));
        }
        Ok(Self {
            schema: 1,
            transaction_id: transaction.id().into(),
            nonce: transaction.health_nonce().into(),
            identity: transaction.identity().clone(),
            executable,
            binary_sha256,
            process: None,
        })
    }
}

pub struct WindowsCandidateLaunch<'a> {
    store: &'a TransactionStore,
    record: LaunchRecord,
    process: Option<TrackedWindowsProcess>,
    _candidate_lease: Option<&'a PinnedWindowsInstaller>,
    _binary_lease: Option<File>,
}

impl<'a> WindowsCandidateLaunch<'a> {
    /// Reserve BEFORE CreateProcess, only after a durable successful installer
    /// exit and native installed identity verification. This is NOT a product/
    /// protocol probe or permission to skip release-file/configuration checks.
    pub fn reserve(
        store: &'a TransactionStore,
        transaction: &UpdateTransaction,
        previous: &WindowsInstallation,
        candidate_lease: &'a PinnedWindowsInstaller,
        access: &WindowsInstallAccess,
    ) -> io::Result<Self> {
        access.require_exclusive(previous.directory())?;
        let candidate = candidate_lease.installer();
        candidate.revalidate()?;
        if super::installer_attempt_status(store, transaction, previous, candidate)?
            != (InstallerAttemptStatus::Exited { exit_code: 0 })
        {
            return Err(invalid(
                "candidate installer has no durable successful exit",
            ));
        }
        let installed = WindowsInstallation::bind(
            &super::WindowsInstallRecord {
                app_id: super::STUDIO_APP_ID.into(),
                scope: super::InstallScope::CurrentUser,
                directory: previous.directory().into(),
                version: candidate.version().to_string(),
                desktop_shortcut: previous.desktop_shortcut(),
            },
            previous.executable(),
            &candidate.version().to_string(),
        )?;
        super::native::revalidate_installation(&installed)?;
        let mut launch = Self::begin(store, transaction, installed.executable().into())?;
        launch._candidate_lease = Some(candidate_lease);
        Ok(launch)
    }

    fn begin(
        store: &'a TransactionStore,
        transaction: &UpdateTransaction,
        executable: PathBuf,
    ) -> io::Result<Self> {
        let persisted = store.read(transaction.identity())?;
        require_same(transaction, &persisted, UpdateStage::Applying)?;
        let (binary_lease, binary_sha256) = pin_binary(&executable)?;
        let record = LaunchRecord::expected(transaction, executable, binary_sha256)?;
        let bytes = serde_json::to_vec(&record).map_err(invalid)?;
        if bytes.len() > 64 * 1024 {
            return Err(invalid("oversized candidate launch intent"));
        }
        let mut file =
            super::create_private_file(&store.directory().join("candidate-launch.json"))?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        Ok(Self {
            store,
            record,
            process: None,
            _candidate_lease: None,
            _binary_lease: Some(binary_lease),
        })
    }

    /// The caller owns this exact Child and keeps it alive on ANY error. This
    /// method never spawns, kills, waits unboundedly, or retries a GUI.
    pub fn bind_child(&mut self, child: &Child) -> io::Result<UpdateTransaction> {
        if self.record.process.is_some() || self.process.is_some() {
            return Err(invalid("candidate process is already bound"));
        }
        let created = super::process::child_process_created(child)?;
        let process = TrackedWindowsProcess::bind(child.id(), created, &self.record.executable)?;
        if process.wait_for_exit(Duration::ZERO)? {
            return Err(invalid("candidate already exited before health binding"));
        }
        let state = self.store.read(&self.record.identity)?;
        self.require_record(&state)?;
        if state.stage() != UpdateStage::Applying {
            return Err(invalid("candidate launch stage changed"));
        }
        // Retain the native witness even if either durable write fails. An
        // unresolved reservation blocks a second attempt after interruption.
        self.process = Some(process);
        self.record.process = Some((child.id(), created));
        super::write_private_atomic(
            &self.store.directory().join("candidate-launch.json"),
            &serde_json::to_vec(&self.record).map_err(invalid)?,
        )?;
        let mut waiting = state;
        waiting.await_health(child.id(), created.to_string())?;
        self.store.write(&waiting)?;
        Ok(waiting)
    }

    fn require_record(&self, state: &UpdateTransaction) -> io::Result<()> {
        if state.id() != self.record.transaction_id
            || state.health_nonce() != self.record.nonce
            || state.identity() != &self.record.identity
        {
            return Err(invalid("candidate launch transaction mismatch"));
        }
        let actual: LaunchRecord = read_private_json(
            &self.store.directory().join("candidate-launch.json"),
            64 * 1024,
        )?;
        if actual != self.record {
            return Err(invalid("candidate launch checkpoint mismatch"));
        }
        Ok(())
    }

    /// Health requires a still-live native process witness, exact persisted
    /// launch and transaction, and the candidate's private bounded receipt.
    /// Only a successful durable Completed write is returned as success.
    pub fn accept_health(&self) -> io::Result<()> {
        self.commit_health(|state| self.store.write(state))
    }

    fn commit_health(
        &self,
        persist: impl FnOnce(&UpdateTransaction) -> io::Result<()>,
    ) -> io::Result<()> {
        let process = self
            .process
            .as_ref()
            .ok_or_else(|| invalid("candidate not bound"))?;
        if process.wait_for_exit(Duration::ZERO)? {
            return Err(invalid("candidate exited before health commit"));
        }
        let (pid, created) = self
            .record
            .process
            .ok_or_else(|| invalid("candidate identity missing"))?;
        let receipt: HealthReceipt =
            read_private_json(&self.store.directory().join("health.json"), 16 * 1024)?;
        if receipt.process_id != pid || receipt.process_started != created.to_string() {
            return Err(invalid("health receipt does not match native candidate"));
        }
        let mut state = self.store.read(&self.record.identity)?;
        self.require_record(&state)?;
        state.accept_health(&receipt)?;
        // Recheck immediately before committing; no launch/exit signal alone
        // can substitute for the initialized first-window receipt.
        if process.wait_for_exit(Duration::ZERO)? {
            return Err(invalid("candidate exited during health validation"));
        }
        persist(&state)
    }

    /// A recovery controller needs this native exit proof AND its installation
    /// exclusive lock before restoring. A missing witness is never "exited".
    pub fn has_exited(&self) -> io::Result<bool> {
        let state = self.store.read(&self.record.identity)?;
        self.require_record(&state)?;
        self.process
            .as_ref()
            .ok_or_else(|| invalid("candidate not bound"))?
            .wait_for_exit(Duration::ZERO)
    }

    /// Release ONLY the installed binary lease after confirmed native exit so
    /// the recovery installer can replace it. Keep process/journal evidence and
    /// package leases; this is not permission to restore without exclusive access.
    pub fn release_binary_after_exit(&mut self) -> io::Result<()> {
        if !self.has_exited()? {
            return Err(invalid("candidate is still running"));
        }
        self._binary_lease.take();
        Ok(())
    }
}

fn require_same(
    expected: &UpdateTransaction,
    actual: &UpdateTransaction,
    stage: UpdateStage,
) -> io::Result<()> {
    if expected.id() != actual.id()
        || expected.health_nonce() != actual.health_nonce()
        || expected.identity() != actual.identity()
        || actual.stage() != stage
    {
        return Err(invalid("candidate transaction has not been persisted"));
    }
    Ok(())
}

fn pin_binary(path: &std::path::Path) -> io::Result<(File, String)> {
    super::reject_redirected_path(path)?;
    let mut file = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ.0)
        .open(path)?;
    if !file.metadata()?.is_file()
        || file.metadata()?.len() == 0
        || file.metadata()?.len() > 512 * 1024 * 1024
    {
        return Err(invalid("invalid installed candidate binary"));
    }
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok((file, format!("{:x}", hash.finalize())))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowsHealthFrame {
    Pending,
    Committed,
    StopRequested,
}

/// Candidate-side receipt capability. Only call after an actual first canvas;
/// Pending must keep editing/configuration writes disabled. No GUI is opened
/// by this type, and it never commits the helper's transaction itself.
pub struct WindowsHealthStartup {
    directory: PathBuf,
    record: LaunchRecord,
    process: (u32, u64),
    waiting_since: Instant,
    receipt_written: bool,
    _binary_lease: File,
}

impl WindowsHealthStartup {
    pub fn load(directory: &std::path::Path) -> io::Result<Self> {
        super::reject_redirected_path(directory)?;
        super::validate_private_directory(directory)?;
        let directory = std::fs::canonicalize(directory)?;
        let root = std::fs::canonicalize(super::helper_request::transaction_root()?)?;
        let record: LaunchRecord =
            read_private_json(&directory.join("candidate-launch.json"), 64 * 1024)?;
        if directory.parent() != Some(root.as_path())
            || directory.file_name() != Some(std::ffi::OsStr::new(&record.transaction_id))
        {
            return Err(invalid(
                "candidate health directory is outside its exact transaction",
            ));
        }
        UpdateTransaction::prepared(
            record.transaction_id.clone(),
            record.nonce.clone(),
            record.identity.clone(),
        )?;
        let installed = super::discover_current_installation()?;
        if record.schema != 1
            || record.identity.platform != "windows-x86_64"
            || record.identity.candidate_version != env!("CARGO_PKG_VERSION")
            || installed.directory() != record.identity.installed_path
            || installed.executable() != record.executable
        {
            return Err(invalid("candidate health startup identity mismatch"));
        }
        let (binary_lease, hash) = pin_binary(installed.executable())?;
        if hash != record.binary_sha256 {
            return Err(invalid("candidate binary differs from launch reservation"));
        }
        super::helper_request::verified_license(installed.directory())?;
        let startup = Self {
            directory,
            record,
            process: (std::process::id(), super::current_process_created()?),
            waiting_since: Instant::now(),
            receipt_written: false,
            _binary_lease: binary_lease,
        };
        let state =
            crate::update_transaction::read_snapshot(&startup.directory, &startup.record.identity)?;
        startup.require_state(&state)?;
        Ok(startup)
    }

    fn require_state(&self, state: &UpdateTransaction) -> io::Result<()> {
        if state.id() != self.record.transaction_id
            || state.health_nonce() != self.record.nonce
            || state.identity() != &self.record.identity
            || !matches!(
                state.stage(),
                UpdateStage::Applying | UpdateStage::AwaitingHealth | UpdateStage::Completed
            )
        {
            return Err(invalid("candidate health transaction mismatch"));
        }
        Ok(())
    }

    pub fn first_canvas_ready(&mut self) -> io::Result<WindowsHealthFrame> {
        let latest: LaunchRecord =
            read_private_json(&self.directory.join("candidate-launch.json"), 64 * 1024)?;
        let mut expected = self.record.clone();
        expected.process = latest.process;
        if expected != latest
            || latest
                .process
                .is_some_and(|process| process != self.process)
        {
            return Err(invalid("candidate launch identity changed"));
        }
        let state =
            crate::update_transaction::read_snapshot(&self.directory, &self.record.identity)?;
        self.require_state(&state)?;
        let stop = self.directory.join("stop-requested.json");
        if stop.exists() {
            if !read_private_json::<bool>(&stop, 16)? {
                return Err(invalid("invalid candidate stop request"));
            }
            return Ok(WindowsHealthFrame::StopRequested);
        }
        // The GUI can initialize before its parent persists the owned Child.
        // Never write a receipt until BOTH launch and transaction bind us.
        if latest.process.is_none() || state.stage() == UpdateStage::Applying {
            if self.waiting_since.elapsed() >= Duration::from_secs(5) {
                return Err(invalid("candidate process checkpoint was not committed"));
            }
            return Ok(WindowsHealthFrame::Pending);
        }
        let started = self.process.1.to_string();
        if state.candidate_process() != Some((self.process.0, started.as_str())) {
            return Err(invalid("candidate transaction is bound to another process"));
        }
        if state.stage() == UpdateStage::Completed {
            if !self.receipt_written {
                return Err(invalid(
                    "candidate completed before its own first-canvas receipt",
                ));
            }
            return Ok(WindowsHealthFrame::Committed);
        }
        if self.waiting_since.elapsed() >= Duration::from_secs(75) {
            return Err(invalid("helper did not commit candidate health in time"));
        }
        if !self.receipt_written {
            let receipt = HealthReceipt {
                transaction_id: self.record.transaction_id.clone(),
                nonce: self.record.nonce.clone(),
                product: "instplot-studio".into(),
                platform: "windows-x86_64".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                installed_path: self.record.identity.installed_path.clone(),
                process_id: self.process.0,
                process_started: started,
                initialized: true,
                window_ready: true,
            };
            let mut file = super::create_private_file(&self.directory.join("health.json"))?;
            file.write_all(&serde_json::to_vec(&receipt).map_err(invalid)?)?;
            file.sync_all()?;
            self.receipt_written = true;
        }
        Ok(WindowsHealthFrame::Pending)
    }
}

fn read_private_json<T: serde::de::DeserializeOwned>(
    path: &std::path::Path,
    limit: u64,
) -> io::Result<T> {
    super::validate_private_file(path)?;
    let mut raw = Vec::new();
    File::open(path)?.take(limit + 1).read_to_end(&mut raw)?;
    if raw.len() as u64 > limit {
        return Err(invalid("oversized candidate evidence"));
    }
    super::validate_private_file(path)?;
    serde_json::from_slice(&raw).map_err(invalid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::{Command, Stdio};

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let mut random = [0_u8; 16];
            getrandom::fill(&mut random).unwrap();
            let path = std::env::temp_dir().join(format!(
                "studio-launch 中文-{:032x}",
                u128::from_le_bytes(random),
            ));
            super::super::create_private_directory(&path).unwrap();
            Self(fs::canonicalize(path).unwrap())
        }
        fn transaction(&self) -> UpdateTransaction {
            let mut state = UpdateTransaction::new(UpdateIdentity {
                product: "instplot-studio".into(),
                platform: "windows-x86_64".into(),
                installed_path: self.0.join("Studio"),
                previous_version: "0.1.2-rc.2".into(),
                candidate_version: "0.1.2-rc.3".into(),
                candidate_sha256: "a".repeat(64),
                candidate_size: 20,
            })
            .unwrap();
            state.transition(UpdateStage::WaitingForExit).unwrap();
            state.transition(UpdateStage::Applying).unwrap();
            state
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn executable() -> PathBuf {
        fs::canonicalize(std::env::current_exe().unwrap()).unwrap()
    }
    fn child(exe: &std::path::Path) -> Child {
        Command::new(exe)
            .args(["update_windows::assets::tests::installer_test_child_waits_for_normal_parent_pipe_close", "--exact"])
            .env("STUDIO_INSTALLER_TEST_WAIT", "1")
            .stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null())
            .spawn().unwrap()
    }
    fn receipt(state: &UpdateTransaction, launch: &WindowsCandidateLaunch<'_>) -> HealthReceipt {
        let (pid, created) = launch.record.process.unwrap();
        HealthReceipt {
            transaction_id: state.id().into(),
            nonce: state.health_nonce().into(),
            product: "instplot-studio".into(),
            platform: "windows-x86_64".into(),
            version: "0.1.2-rc.3".into(),
            installed_path: state.identity().installed_path.clone(),
            process_id: pid,
            process_started: created.to_string(),
            initialized: true,
            window_ready: true,
        }
    }
    fn write_receipt(path: &std::path::Path, receipt: &HealthReceipt) {
        super::super::write_private_atomic(path, &serde_json::to_vec(receipt).unwrap()).unwrap();
    }

    #[test]
    fn reservation_is_single_use_even_without_a_process_checkpoint() {
        let fixture = Fixture::new();
        let store = TransactionStore::lock(&fixture.0).unwrap();
        let state = fixture.transaction();
        store.write(&state).unwrap();
        let launch = WindowsCandidateLaunch::begin(&store, &state, executable()).unwrap();
        assert!(launch.accept_health().is_err());
        assert!(launch.has_exited().is_err());
        drop(launch);
        assert!(WindowsCandidateLaunch::begin(&store, &state, executable()).is_err());
        // Partial existing intent is retained, never repaired or replayed.
        let path = fixture.0.join("candidate-launch.json");
        super::super::write_private_atomic(&path, b"{").unwrap();
        assert!(WindowsCandidateLaunch::begin(&store, &state, executable()).is_err());
        assert_eq!(fs::read(path).unwrap(), b"{");
        assert_eq!(
            store.read(state.identity()).unwrap().stage(),
            UpdateStage::Applying
        );
    }

    #[test]
    fn launch_requires_exact_persisted_transaction_and_strict_record() {
        let fixture = Fixture::new();
        let store = TransactionStore::lock(&fixture.0).unwrap();
        let state = fixture.transaction();
        store.write(&state).unwrap();
        let another = fixture.transaction();
        assert!(WindowsCandidateLaunch::begin(&store, &another, executable()).is_err());
        assert!(!fixture.0.join("candidate-launch.json").exists());
        let launch = WindowsCandidateLaunch::begin(&store, &state, executable()).unwrap();
        assert!(launch.require_record(&another).is_err());
        let mut raw = serde_json::to_value(&launch.record).unwrap();
        raw["unexpected"] = serde_json::json!(true);
        assert!(serde_json::from_value::<LaunchRecord>(raw).is_err());
        let mut wrong = launch.record.clone();
        wrong.identity.candidate_sha256 = "b".repeat(64);
        super::super::write_private_atomic(
            &fixture.0.join("candidate-launch.json"),
            &serde_json::to_vec(&wrong).unwrap(),
        )
        .unwrap();
        assert!(launch.require_record(&state).is_err());
    }

    #[test]
    fn native_live_child_and_initialized_matching_receipt_are_required() {
        // Actual native child identity; NOT a Studio GUI or actual installation.
        let fixture = Fixture::new();
        let store = TransactionStore::lock(&fixture.0).unwrap();
        let state = fixture.transaction();
        store.write(&state).unwrap();
        let exe = executable();
        let mut launch = WindowsCandidateLaunch::begin(&store, &state, exe.clone()).unwrap();
        let mut child = child(&exe);
        assert_eq!(
            launch.bind_child(&child).unwrap().stage(),
            UpdateStage::AwaitingHealth
        );
        assert!(launch.bind_child(&child).is_err());
        let receipt = receipt(&state, &launch);
        let receipt_path = fixture.0.join("health.json");
        for index in 0..5 {
            let mut bad = receipt.clone();
            match index {
                0 => bad.nonce = "0".repeat(64),
                1 => bad.process_id = 0,
                2 => bad.process_started = "wrong-creation-time".into(),
                3 => bad.window_ready = false,
                _ => bad.initialized = false,
            }
            write_receipt(&receipt_path, &bad);
            assert!(launch.accept_health().is_err());
            assert_eq!(
                store.read(state.identity()).unwrap().stage(),
                UpdateStage::AwaitingHealth
            );
        }
        write_receipt(&receipt_path, &receipt);
        drop(child.stdin.take());
        assert!(child.wait().unwrap().success());
        assert!(launch.accept_health().is_err());
        assert_eq!(
            store.read(state.identity()).unwrap().stage(),
            UpdateStage::AwaitingHealth
        );
    }

    #[test]
    fn matching_live_native_witness_commits_health_durably() {
        let fixture = Fixture::new();
        let store = TransactionStore::lock(&fixture.0).unwrap();
        let state = fixture.transaction();
        store.write(&state).unwrap();
        let exe = executable();
        let mut launch = WindowsCandidateLaunch::begin(&store, &state, exe.clone()).unwrap();
        let mut child = child(&exe);
        launch.bind_child(&child).unwrap();
        write_receipt(&fixture.0.join("health.json"), &receipt(&state, &launch));
        assert!(!launch.has_exited().unwrap());
        assert!(launch.release_binary_after_exit().is_err());
        assert!(launch._binary_lease.is_some());
        assert!(
            launch
                .commit_health(|_| Err(io::Error::other("injected health commit failure")))
                .is_err()
        );
        assert_eq!(
            store.read(state.identity()).unwrap().stage(),
            UpdateStage::AwaitingHealth
        );
        assert!(!launch.has_exited().unwrap());
        launch.accept_health().unwrap();
        assert_eq!(
            store.read(state.identity()).unwrap().stage(),
            UpdateStage::Completed
        );
        assert!(launch.accept_health().is_err());
        drop(child.stdin.take());
        assert!(child.wait().unwrap().success());
        assert!(launch.has_exited().unwrap());
        launch.release_binary_after_exit().unwrap();
        assert!(launch._binary_lease.is_none());
    }

    #[test]
    fn candidate_receipt_waits_for_native_binding_and_helper_commit() {
        // Candidate-side component with a real CURRENT native process, not GUI.
        let fixture = Fixture::new();
        let store = TransactionStore::lock(&fixture.0).unwrap();
        let mut state = UpdateTransaction::new(UpdateIdentity {
            product: "instplot-studio".into(),
            platform: "windows-x86_64".into(),
            installed_path: fixture.0.join("Studio"),
            previous_version: "0.1.2-rc.1".into(),
            candidate_version: env!("CARGO_PKG_VERSION").into(),
            candidate_sha256: "a".repeat(64),
            candidate_size: 20,
        })
        .unwrap();
        state.transition(UpdateStage::WaitingForExit).unwrap();
        state.transition(UpdateStage::Applying).unwrap();
        store.write(&state).unwrap();
        let exe = executable();
        let (lease, hash) = pin_binary(&exe).unwrap();
        let record = LaunchRecord::expected(&state, exe, hash).unwrap();
        super::super::write_private_atomic(
            &fixture.0.join("candidate-launch.json"),
            &serde_json::to_vec(&record).unwrap(),
        )
        .unwrap();
        let process = (
            std::process::id(),
            super::super::current_process_created().unwrap(),
        );
        let mut startup = WindowsHealthStartup {
            directory: fixture.0.clone(),
            record: record.clone(),
            process,
            waiting_since: Instant::now(),
            receipt_written: false,
            _binary_lease: lease,
        };
        assert_eq!(
            startup.first_canvas_ready().unwrap(),
            WindowsHealthFrame::Pending
        );
        assert!(!fixture.0.join("health.json").exists());
        let mut bound = record;
        bound.process = Some(process);
        super::super::write_private_atomic(
            &fixture.0.join("candidate-launch.json"),
            &serde_json::to_vec(&bound).unwrap(),
        )
        .unwrap();
        state
            .await_health(process.0, process.1.to_string())
            .unwrap();
        store.write(&state).unwrap();
        startup.waiting_since = Instant::now() - Duration::from_secs(76);
        assert!(startup.first_canvas_ready().is_err());
        assert!(!fixture.0.join("health.json").exists());
        startup.waiting_since = Instant::now();
        // A stale receipt must never be replaced with this process's receipt.
        let health_path = fixture.0.join("health.json");
        super::super::write_private_atomic(&health_path, b"stale receipt").unwrap();
        assert!(startup.first_canvas_ready().is_err());
        assert_eq!(std::fs::read(&health_path).unwrap(), b"stale receipt");
        assert!(!startup.receipt_written);
        std::fs::remove_file(&health_path).unwrap();
        assert_eq!(
            startup.first_canvas_ready().unwrap(),
            WindowsHealthFrame::Pending
        );
        let receipt: HealthReceipt =
            read_private_json(&fixture.0.join("health.json"), 16 * 1024).unwrap();
        assert_eq!(receipt.process_id, process.0);
        assert_eq!(receipt.process_started, process.1.to_string());
        assert_eq!(
            store.read(state.identity()).unwrap().stage(),
            UpdateStage::AwaitingHealth
        );
        assert_eq!(
            startup.first_canvas_ready().unwrap(),
            WindowsHealthFrame::Pending
        );
        state.accept_health(&receipt).unwrap();
        store.write(&state).unwrap();
        assert_eq!(
            startup.first_canvas_ready().unwrap(),
            WindowsHealthFrame::Committed
        );
        super::super::write_private_atomic(&fixture.0.join("stop-requested.json"), b"true")
            .unwrap();
        assert_eq!(
            startup.first_canvas_ready().unwrap(),
            WindowsHealthFrame::StopRequested
        );
        bound.process = Some((process.0, process.1 + 1));
        super::super::write_private_atomic(
            &fixture.0.join("candidate-launch.json"),
            &serde_json::to_vec(&bound).unwrap(),
        )
        .unwrap();
        assert!(startup.first_canvas_ready().is_err());
    }

    #[test]
    fn candidate_checkpoint_timeout_never_writes_health() {
        let fixture = Fixture::new();
        let store = TransactionStore::lock(&fixture.0).unwrap();
        let state = fixture.transaction();
        store.write(&state).unwrap();
        let exe = executable();
        let (lease, hash) = pin_binary(&exe).unwrap();
        let record = LaunchRecord::expected(&state, exe, hash).unwrap();
        super::super::write_private_atomic(
            &fixture.0.join("candidate-launch.json"),
            &serde_json::to_vec(&record).unwrap(),
        )
        .unwrap();
        let mut startup = WindowsHealthStartup {
            directory: fixture.0.clone(),
            record,
            process: (
                std::process::id(),
                super::super::current_process_created().unwrap(),
            ),
            waiting_since: Instant::now() - Duration::from_secs(6),
            receipt_written: false,
            _binary_lease: lease,
        };
        assert!(startup.first_canvas_ready().is_err());
        assert!(!fixture.0.join("health.json").exists());
    }
}
