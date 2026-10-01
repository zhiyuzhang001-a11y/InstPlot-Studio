//! Private, bound handoff to a copy of the RUNNING old application.
//! Preparing/loading this request never closes a GUI or executes an installer.
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use windows::Win32::Storage::FileSystem::FILE_SHARE_READ;

use crate::update_transaction::{TransactionStore, UpdateIdentity, UpdateStage, UpdateTransaction};

use super::{
    InstallScope, PinnedWindowsInstallerPair, PreparedWindowsInstallers, STUDIO_APP_ID,
    TrackedWindowsProcess, VerifiedWindowsInstaller, WindowsInstallRecord, WindowsInstallation,
    WindowsInstallerPair, invalid,
};

const HELPER_NAME: &str = "instplot-update-helper.exe";

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AssetBinding {
    cache: PathBuf,
    version: String,
    manifest_sha256: String,
    package_sha256: String,
    package_size: u64,
}

impl AssetBinding {
    fn from_installer(installer: &VerifiedWindowsInstaller) -> io::Result<Self> {
        Ok(Self {
            cache: fs::canonicalize(
                installer
                    .path()
                    .parent()
                    .ok_or_else(|| invalid("missing cache"))?,
            )?,
            version: installer.version().to_string(),
            manifest_sha256: installer.manifest_sha256().into(),
            package_sha256: installer.sha256().into(),
            package_size: installer.size_bytes(),
        })
    }

    fn require(&self, installer: &VerifiedWindowsInstaller) -> io::Result<()> {
        let actual = Self::from_installer(installer)?;
        if self.cache != actual.cache
            || self.version != actual.version
            || self.manifest_sha256 != actual.manifest_sha256
            || self.package_sha256 != actual.package_sha256
            || self.package_size != actual.package_size
        {
            return Err(invalid("installer differs from prepared request"));
        }
        Ok(())
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HelperRequest {
    schema: u32,
    transaction_id: String,
    nonce: String,
    identity: UpdateIdentity,
    desktop_shortcut: bool,
    old_process_id: u32,
    old_process_created: u64,
    previous_binary_sha256: String,
    previous_license_sha256: String,
    resume_project: Option<super::WindowsResumeProject>,
    recovery: AssetBinding,
    candidate: AssetBinding,
}

impl HelperRequest {
    fn transaction(&self) -> io::Result<UpdateTransaction> {
        if self.schema != 1
            || self.identity.platform != "windows-x86_64"
            || self.old_process_id == 0
            || self.old_process_created == 0
            || self.previous_binary_sha256.len() != 64
            || !self
                .previous_binary_sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || self.previous_license_sha256 != expected_license_sha256()
            || self.recovery.version != self.identity.previous_version
            || self.candidate.version != self.identity.candidate_version
            || self.candidate.package_sha256 != self.identity.candidate_sha256
            || self.candidate.package_size != self.identity.candidate_size
        {
            return Err(invalid("invalid Windows helper request binding"));
        }
        UpdateTransaction::prepared(
            self.transaction_id.clone(),
            self.nonce.clone(),
            self.identity.clone(),
        )
    }

    fn installation(&self) -> io::Result<WindowsInstallation> {
        let record = WindowsInstallRecord {
            app_id: STUDIO_APP_ID.into(),
            scope: InstallScope::CurrentUser,
            directory: self.identity.installed_path.clone(),
            version: self.identity.previous_version.clone(),
            desktop_shortcut: self.desktop_shortcut,
        };
        WindowsInstallation::bind(
            &record,
            &record.directory.join("instplot-studio.exe"),
            &record.version,
        )
    }
}

/// Holds the helper executable lease. No spawn or exit request is made here.
pub struct PreparedWindowsHelper {
    directory: PathBuf,
    request: HelperRequest,
    _helper_lease: File,
    _installers: PinnedWindowsInstallerPair,
    _resume_lease: Option<File>,
}

impl PreparedWindowsHelper {
    pub fn create(prepared: &PreparedWindowsInstallers) -> io::Result<Self> {
        let installed = prepared.installation();
        super::native::revalidate_installation(installed)?;
        if fs::canonicalize(std::env::current_exe()?)? != installed.executable()
            || installed.version().to_string() != env!("CARGO_PKG_VERSION")
        {
            return Err(invalid(
                "helper must be copied from the exact running old installation",
            ));
        }
        let candidate = prepared.installers().candidate.installer();
        let recovery = prepared.installers().recovery.installer();
        candidate.revalidate()?;
        recovery.revalidate()?;
        let previous_license_sha256 = verified_license(installed.directory())?;
        let transaction = UpdateTransaction::new(UpdateIdentity {
            product: "instplot-studio".into(),
            platform: "windows-x86_64".into(),
            installed_path: installed.directory().to_path_buf(),
            previous_version: installed.version().to_string(),
            candidate_version: candidate.version().to_string(),
            candidate_sha256: candidate.sha256().into(),
            candidate_size: candidate.size_bytes(),
        })?;
        let root = transaction_root()?;
        let directory = root.join(transaction.id());
        super::create_private_directory(&directory)?;
        let directory = fs::canonicalize(directory)?;
        let store = TransactionStore::lock(&directory)?;
        let (helper_lease, previous_binary_sha256) =
            copy_helper(installed.executable(), &directory.join(HELPER_NAME))?;
        let request = HelperRequest {
            schema: 1,
            transaction_id: transaction.id().into(),
            nonce: transaction.health_nonce().into(),
            identity: transaction.identity().clone(),
            desktop_shortcut: installed.desktop_shortcut(),
            old_process_id: std::process::id(),
            old_process_created: super::current_process_created()?,
            previous_binary_sha256,
            previous_license_sha256,
            resume_project: None,
            recovery: AssetBinding::from_installer(recovery)?,
            candidate: AssetBinding::from_installer(candidate)?,
        };
        request.transaction()?;
        let installers = request.verify_installers()?;
        write_new_json(&directory.join("request.json"), &request)?;
        store.write(&transaction)?;
        drop(store);
        Ok(Self {
            directory,
            request,
            _helper_lease: helper_lease,
            _installers: installers,
            _resume_lease: None,
        })
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }
    pub fn helper_executable(&self) -> PathBuf {
        self.directory.join(HELPER_NAME)
    }

    /// Only after saving/discarding drafts and freezing the old editor. This
    /// binds an existing saved primary file; it does not save user work itself.
    pub fn set_resume_project(&mut self, project: Option<&Path>) -> io::Result<()> {
        let store = TransactionStore::lock(&self.directory)?;
        let state = store.read(&self.request.identity)?;
        require_transaction(&self.request, &state, UpdateStage::Prepared)?;
        let (binding, lease) = match project {
            Some(path) => {
                let (binding, lease) = super::WindowsResumeProject::capture(
                    path,
                    &self.request.identity.installed_path,
                )?;
                (Some(binding), Some(lease))
            }
            None => (None, None),
        };
        let mut request = self.request.clone();
        request.resume_project = binding;
        let bytes = serde_json::to_vec(&request).map_err(invalid)?;
        if bytes.len() > 64 * 1024 {
            return Err(invalid("oversized helper project handoff"));
        }
        super::write_private_atomic(&self.directory.join("request.json"), &bytes)?;
        self.request = request;
        self._resume_lease = lease;
        Ok(())
    }

    /// Parent verifies an actual owned Child handle, not a supplied PID or a
    /// ready file alone. Success still does not request or authorize GUI exit.
    pub fn confirm_ready(&self, child: &std::process::Child) -> io::Result<()> {
        confirm_process_ready(&self.directory, &self.request, child)?;
        let installation = self.request.installation()?;
        super::native::revalidate_installation(&installation)?;
        require_original_files(&installation, &self.request)
    }

    /// Call only AFTER the work-protection controller has accepted user restart.
    /// This arms the durable wait state, but does not spawn or close anything.
    pub fn arm_waiting(&self) -> io::Result<()> {
        let installed = self.request.installation()?;
        super::native::revalidate_installation(&installed)?;
        let _old = TrackedWindowsProcess::bind(
            self.request.old_process_id,
            self.request.old_process_created,
            installed.executable(),
        )?;
        if digest_file(installed.executable())? != self.request.previous_binary_sha256
            || digest_file(&self.helper_executable())? != self.request.previous_binary_sha256
        {
            return Err(invalid(
                "old application or helper changed since preparation",
            ));
        }
        require_original_files(&installed, &self.request)?;
        let _resume = self
            .request
            .resume_project
            .as_ref()
            .map(|project| project.pin(installed.directory()))
            .transpose()?;
        let _installers = self.request.verify_installers()?;
        let store = TransactionStore::lock(&self.directory)?;
        let mut state = store.read(&self.request.identity)?;
        require_transaction(&self.request, &state, UpdateStage::Prepared)?;
        state.transition(UpdateStage::WaitingForExit)?;
        store.write(&state)
    }
}

impl HelperRequest {
    fn verify_installers(&self) -> io::Result<PinnedWindowsInstallerPair> {
        let root = fs::canonicalize(super::private_download_root()?)?;
        for cache in [&self.recovery.cache, &self.candidate.cache] {
            super::reject_redirected_path(cache)?;
            if fs::canonicalize(cache)? != *cache || !cache.starts_with(&root) || *cache == root {
                return Err(invalid(
                    "installer cache is outside the fixed private download root",
                ));
            }
        }
        let installed = self.installation()?;
        let pair = WindowsInstallerPair::from_caches(
            &installed,
            &self.recovery.cache,
            &self.candidate.cache,
            &self.candidate.version,
        )?;
        self.recovery.require(pair.recovery())?;
        self.candidate.require(pair.candidate())?;
        pair.pin()
    }
}

/// Only the exact private helper executable can construct this capability.
/// Its store, both installer leases and exact old-process handle remain owned.
pub struct WindowsHelperSession {
    directory: PathBuf,
    request: HelperRequest,
    transaction: UpdateTransaction,
    store: TransactionStore,
    installation: WindowsInstallation,
    installers: PinnedWindowsInstallerPair,
    old_process: TrackedWindowsProcess,
    _helper_lease: File,
    _resume_lease: Option<File>,
}

impl WindowsHelperSession {
    pub fn load_waiting(directory: &Path) -> io::Result<Self> {
        super::reject_redirected_path(directory)?;
        super::validate_private_directory(directory)?;
        let directory = fs::canonicalize(directory)?;
        let request: HelperRequest =
            serde_json::from_slice(&read_private(&directory.join("request.json"), 64 * 1024)?)
                .map_err(invalid)?;
        request.transaction()?;
        if directory.parent() != Some(fs::canonicalize(transaction_root()?)?.as_path())
            || directory.file_name().and_then(|s| s.to_str()) != Some(&request.transaction_id)
            || request.identity.previous_version != env!("CARGO_PKG_VERSION")
            || fs::canonicalize(std::env::current_exe()?)? != directory.join(HELPER_NAME)
        {
            return Err(invalid(
                "foreign helper path, transaction directory or source version",
            ));
        }
        let store = TransactionStore::lock(&directory)?;
        let transaction = store.read(&request.identity)?;
        require_transaction(&request, &transaction, UpdateStage::WaitingForExit)?;
        let installation = request.installation()?;
        super::native::revalidate_installation(&installation)?;
        let old_process = TrackedWindowsProcess::bind(
            request.old_process_id,
            request.old_process_created,
            installation.executable(),
        )?;
        if old_process.wait_for_exit(std::time::Duration::ZERO)? {
            return Err(invalid("old GUI exited before helper acknowledgement"));
        }
        if digest_file(installation.executable())? != request.previous_binary_sha256 {
            return Err(invalid(
                "old application changed before helper acknowledgement",
            ));
        }
        require_original_files(&installation, &request)?;
        let helper = directory.join(HELPER_NAME);
        let helper_lease = open_read_lease(&helper)?;
        if digest_file(&helper)? != request.previous_binary_sha256 {
            return Err(invalid("helper copy does not match original binary"));
        }
        let installers = request.verify_installers()?;
        let resume_lease = request
            .resume_project
            .as_ref()
            .map(|project| project.pin(installation.directory()))
            .transpose()?;
        Ok(Self {
            directory,
            request,
            transaction,
            store,
            installation,
            installers,
            old_process,
            _helper_lease: helper_lease,
            _resume_lease: resume_lease,
        })
    }

    /// Acknowledgement only. Not permission to apply, launch, or report health.
    pub fn acknowledge_ready(&self) -> io::Result<()> {
        if self.old_process.wait_for_exit(std::time::Duration::ZERO)? {
            return Err(invalid("old GUI exited before helper acknowledgement"));
        }
        super::native::revalidate_installation(&self.installation)?;
        require_original_files(&self.installation, &self.request)?;
        self.installers.recovery.installer().revalidate()?;
        self.installers.candidate.installer().revalidate()?;
        let state = self.store.read(self.transaction.identity())?;
        require_transaction(&self.request, &state, UpdateStage::WaitingForExit)?;
        write_new_json(
            &self.directory.join("helper-ready.json"),
            &HelperReady {
                transaction_id: state.id().into(),
                nonce: state.health_nonce().into(),
                process_id: std::process::id(),
                process_created: super::current_process_created()?,
            },
        )
    }

    pub fn transaction(&self) -> &UpdateTransaction {
        &self.transaction
    }
    pub fn store(&self) -> &TransactionStore {
        &self.store
    }
    pub fn installation(&self) -> &WindowsInstallation {
        &self.installation
    }
    pub fn installers(&self) -> &PinnedWindowsInstallerPair {
        &self.installers
    }
    pub fn old_process(&self) -> &TrackedWindowsProcess {
        &self.old_process
    }
    pub fn resume_project(&self) -> Option<&super::WindowsResumeProject> {
        self.request.resume_project.as_ref()
    }

    /// A bounded wait/permission failure BEFORE installation may safely end
    /// the transaction only after proving the exact old installation intact.
    /// Never use this to disguise an ambiguous or started installer as canceled.
    pub fn abort_before_apply(&self, reason: &str) -> io::Result<UpdateTransaction> {
        abort_waiting(&self.store, &self.request, reason, || {
            super::native::revalidate_installation(&self.installation)?;
            require_original_files(&self.installation, &self.request)
        })
    }

    /// Verify the current fixed release-file contract AFTER a durable successful
    /// old-installer exit. No GUI launch, deletion, configuration restoration or
    /// RolledBack transition is performed here.
    pub fn verify_restored_installation(
        &self,
        transaction: &UpdateTransaction,
        access: &super::WindowsInstallAccess,
    ) -> io::Result<()> {
        access.require_exclusive(self.installation.directory())?;
        if !self.old_process.wait_for_exit(std::time::Duration::ZERO)? {
            return Err(invalid("old application has not exited normally"));
        }
        require_transaction(&self.request, transaction, UpdateStage::Restoring)?;
        self.installers.recovery.installer().revalidate()?;
        if super::installer_attempt_status(
            &self.store,
            transaction,
            &self.installation,
            self.installers.recovery.installer(),
        )? != (super::InstallerAttemptStatus::Exited { exit_code: 0 })
        {
            return Err(invalid("recovery installer has no durable successful exit"));
        }
        super::native::revalidate_installation(&self.installation)?;
        require_original_files(&self.installation, &self.request)
    }
}

fn abort_waiting(
    store: &TransactionStore,
    request: &HelperRequest,
    reason: &str,
    verify_original: impl FnOnce() -> io::Result<()>,
) -> io::Result<UpdateTransaction> {
    let mut state = store.read(&request.identity)?;
    require_transaction(request, &state, UpdateStage::WaitingForExit)?;
    for name in [
        "apply-installer.json",
        "restore-installer.json",
        "candidate-launch.json",
    ] {
        match fs::symlink_metadata(store.directory().join(name)) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
            Ok(_) => return Err(invalid("execution evidence forbids pre-apply cancellation")),
        }
    }
    verify_original()?;
    state.record_error(reason);
    state.transition(UpdateStage::FailedBeforeApply)?;
    store.write(&state)?;
    Ok(state)
}

const RELEASE_LICENSE: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../LICENSE"));

fn expected_license_sha256() -> String {
    format!("{:x}", Sha256::digest(RELEASE_LICENSE))
}

pub(super) fn verified_license(directory: &Path) -> io::Result<String> {
    let path = directory.join("LICENSE");
    super::reject_redirected_path(&path)?;
    let metadata = fs::metadata(&path)?;
    if !metadata.is_file() || metadata.len() != RELEASE_LICENSE.len() as u64 {
        return Err(invalid(
            "installed release LICENSE has unexpected type or size",
        ));
    }
    let actual = digest_file(&path)?;
    if actual != expected_license_sha256() {
        return Err(invalid(
            "installed release LICENSE differs from the running old build",
        ));
    }
    Ok(actual)
}

fn require_original_files(
    installed: &WindowsInstallation,
    request: &HelperRequest,
) -> io::Result<()> {
    if digest_file(installed.executable())? != request.previous_binary_sha256
        || verified_license(installed.directory())? != request.previous_license_sha256
    {
        return Err(invalid("original release files are not intact"));
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HelperReady {
    transaction_id: String,
    nonce: String,
    process_id: u32,
    process_created: u64,
}

impl HelperReady {
    fn matches(&self, request: &HelperRequest, pid: u32, created: u64) -> bool {
        pid != 0
            && created != 0
            && self.transaction_id == request.transaction_id
            && self.nonce == request.nonce
            && self.process_id == pid
            && self.process_created == created
    }
}

fn confirm_process_ready(
    directory: &Path,
    request: &HelperRequest,
    child: &std::process::Child,
) -> io::Result<()> {
    let helper = TrackedWindowsProcess::bind_child(child, &directory.join(HELPER_NAME))?;
    if helper.wait_for_exit(std::time::Duration::ZERO)? {
        return Err(invalid("helper exited before parent confirmed readiness"));
    }
    let ready: HelperReady =
        serde_json::from_slice(&read_private(&directory.join("helper-ready.json"), 4096)?)
            .map_err(invalid)?;
    let created = super::process::child_process_created(child)?;
    if !ready.matches(request, child.id(), created) {
        return Err(invalid(
            "helper readiness belongs to another process or transaction",
        ));
    }
    Ok(())
}

fn require_transaction(
    request: &HelperRequest,
    state: &UpdateTransaction,
    stage: UpdateStage,
) -> io::Result<()> {
    request.transaction()?;
    if state.id() != request.transaction_id
        || state.health_nonce() != request.nonce
        || state.identity() != &request.identity
        || state.stage() != stage
    {
        return Err(invalid(
            "helper request does not match durable transaction stage",
        ));
    }
    Ok(())
}

pub(super) fn transaction_root() -> io::Result<PathBuf> {
    let root = super::native::updater_private_root()?.join("transactions");
    if let Err(error) = super::create_private_directory(&root)
        && super::validate_private_directory(&root).is_err()
    {
        return Err(error);
    }
    Ok(root)
}

fn read_private(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    super::validate_private_file(path)?;
    let mut bytes = Vec::new();
    File::open(path)?.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(invalid("oversized helper evidence"));
    }
    super::validate_private_file(path)?;
    Ok(bytes)
}

fn write_new_json(path: &Path, value: &impl Serialize) -> io::Result<()> {
    let bytes = serde_json::to_vec(value).map_err(invalid)?;
    if bytes.len() > 64 * 1024 {
        return Err(invalid("oversized helper request"));
    }
    let mut file = super::create_private_file(path)?;
    file.write_all(&bytes)?;
    file.sync_all()
}

fn open_read_lease(path: &Path) -> io::Result<File> {
    super::validate_private_file(path)?;
    let file = fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ.0)
        .open(path)?;
    super::validate_private_file(path)?;
    Ok(file)
}

fn digest_file(path: &Path) -> io::Result<String> {
    super::reject_redirected_path(path)?;
    let mut file = File::open(path)?;
    if !file.metadata()?.is_file() || file.metadata()?.len() > 512 * 1024 * 1024 {
        return Err(invalid("invalid helper binary"));
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
    Ok(format!("{:x}", hash.finalize()))
}

fn copy_helper(source: &Path, destination: &Path) -> io::Result<(File, String)> {
    super::reject_redirected_path(source)?;
    let mut input = fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ.0)
        .open(source)?;
    if !input.metadata()?.is_file()
        || input.metadata()?.len() == 0
        || input.metadata()?.len() > 512 * 1024 * 1024
    {
        return Err(invalid("invalid old helper source"));
    }
    let mut output = super::create_private_file(destination)?;
    io::copy(&mut input, &mut output)?;
    output.sync_all()?;
    drop(output);
    let lease = open_read_lease(destination)?;
    let expected = digest_file(source)?;
    if digest_file(destination)? != expected {
        return Err(invalid("helper copy hash mismatch"));
    }
    Ok((lease, expected))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> HelperRequest {
        let identity = UpdateIdentity {
            product: "instplot-studio".into(),
            platform: "windows-x86_64".into(),
            installed_path: std::env::temp_dir().join("Studio 中文 install"),
            previous_version: "0.1.2-rc.2".into(),
            candidate_version: "0.1.2-rc.3".into(),
            candidate_sha256: "a".repeat(64),
            candidate_size: 20,
        };
        let cache = |version: &str, digest: &str, size| AssetBinding {
            cache: std::env::temp_dir().join(version),
            version: version.into(),
            manifest_sha256: "c".repeat(64),
            package_sha256: digest.repeat(64),
            package_size: size,
        };
        HelperRequest {
            schema: 1,
            transaction_id: "1".repeat(32),
            nonce: "2".repeat(64),
            identity,
            desktop_shortcut: false,
            old_process_id: 123,
            old_process_created: 456,
            previous_binary_sha256: "b".repeat(64),
            previous_license_sha256: expected_license_sha256(),
            resume_project: None,
            recovery: cache("0.1.2-rc.2", "d", 10),
            candidate: cache("0.1.2-rc.3", "a", 20),
        }
    }

    #[test]
    fn helper_request_rejects_foreign_identity_nonce_stage_and_schema() {
        let mut request = request();
        let mut transaction = request.transaction().unwrap();
        require_transaction(&request, &transaction, UpdateStage::Prepared).unwrap();
        assert!(require_transaction(&request, &transaction, UpdateStage::WaitingForExit).is_err());
        transaction.transition(UpdateStage::WaitingForExit).unwrap();
        require_transaction(&request, &transaction, UpdateStage::WaitingForExit).unwrap();
        request.nonce = "3".repeat(64);
        assert!(require_transaction(&request, &transaction, UpdateStage::WaitingForExit).is_err());
        request.nonce = "2".repeat(64);
        request.identity.installed_path.push("other");
        assert!(require_transaction(&request, &transaction, UpdateStage::WaitingForExit).is_err());
        request.schema = 2;
        assert!(request.transaction().is_err());
        let mut raw = serde_json::to_value(request).unwrap();
        raw["arbitrary_command"] = "must not run".into();
        assert!(serde_json::from_value::<HelperRequest>(raw).is_err());
        let mut missing_license = serde_json::to_value(self::request()).unwrap();
        missing_license
            .as_object_mut()
            .unwrap()
            .remove("previous_license_sha256");
        assert!(serde_json::from_value::<HelperRequest>(missing_license).is_err());
        let mut changed_license = self::request();
        changed_license.previous_license_sha256 = "e".repeat(64);
        assert!(changed_license.transaction().is_err());
    }

    #[test]
    fn cancellation_requires_no_execution_and_verified_original_files() {
        let mut random = [0; 16];
        getrandom::fill(&mut random).unwrap();
        let root = std::env::temp_dir()
            .join(format!("studio-abort-{:032x}", u128::from_le_bytes(random),));
        super::super::create_private_directory(&root).unwrap();
        let store = TransactionStore::lock(&root).unwrap();
        let request = request();
        let mut waiting = request.transaction().unwrap();
        waiting.transition(UpdateStage::WaitingForExit).unwrap();
        store.write(&waiting).unwrap();
        assert!(
            abort_waiting(&store, &request, "wait failed", || Err(invalid(
                "original installation could not be verified"
            )))
            .is_err()
        );
        assert_eq!(
            store.read(&request.identity).unwrap().stage(),
            UpdateStage::WaitingForExit
        );
        // Even partial/unresolved intent is not proof that installation never ran.
        for name in [
            "apply-installer.json",
            "restore-installer.json",
            "candidate-launch.json",
        ] {
            let path = root.join(name);
            write_new_json(&path, &"partial execution evidence").unwrap();
            assert!(
                abort_waiting(&store, &request, "cancel", || panic!(
                    "must refuse before calling original-file verifier"
                ))
                .is_err()
            );
            assert!(path.exists());
            fs::remove_file(path).unwrap();
        }
        let failed = abort_waiting(&store, &request, request.nonce.as_str(), || Ok(())).unwrap();
        assert_eq!(failed.stage(), UpdateStage::FailedBeforeApply);
        assert_eq!(
            store.read(&request.identity).unwrap().stage(),
            UpdateStage::FailedBeforeApply
        );
        let raw = fs::read_to_string(root.join("transaction.json")).unwrap();
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(value["last_error"], "[redacted]");
        assert!(abort_waiting(&store, &request, "again", || Ok(())).is_err());
        let mut applying = waiting;
        applying.transition(UpdateStage::Applying).unwrap();
        store.write(&applying).unwrap();
        assert!(abort_waiting(&store, &request, "not a safe cancellation", || Ok(())).is_err());
        assert_eq!(
            store.read(&request.identity).unwrap().stage(),
            UpdateStage::Applying
        );
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn fixed_release_files_are_checked_without_touching_user_files() {
        let mut random = [0_u8; 16];
        getrandom::fill(&mut random).unwrap();
        let root = std::env::temp_dir().join(format!(
            "studio-release-files 中文-{:032x}",
            u128::from_le_bytes(random),
        ));
        super::super::create_private_directory(&root).unwrap();
        let root = fs::canonicalize(root).unwrap();
        let binary = root.join("instplot-studio.exe");
        let license = root.join("LICENSE");
        let user_file = root.join("user-project.instplot");
        for (path, bytes) in [
            (
                &binary,
                b"fake old binary; hash contract fixture only".as_slice(),
            ),
            (&license, RELEASE_LICENSE),
            (&user_file, b"user project must remain untouched".as_slice()),
        ] {
            let mut file = super::super::create_private_file(path).unwrap();
            file.write_all(bytes).unwrap();
            file.sync_all().unwrap();
        }
        let mut request = request();
        request.identity.installed_path = root.clone();
        request.previous_binary_sha256 = digest_file(&binary).unwrap();
        let installed = request.installation().unwrap();
        require_original_files(&installed, &request).unwrap();
        let user_hash = digest_file(&user_file).unwrap();
        fs::write(&license, b"modified license").unwrap();
        assert!(verified_license(&root).is_err());
        assert!(require_original_files(&installed, &request).is_err());
        assert_eq!(fs::read(&license).unwrap(), b"modified license");
        let mut same_length = RELEASE_LICENSE.to_vec();
        same_length[0] ^= 1;
        fs::write(&license, &same_length).unwrap();
        assert!(verified_license(&root).is_err());
        assert_eq!(fs::read(&license).unwrap(), same_length);
        fs::write(&license, RELEASE_LICENSE).unwrap();
        fs::write(&binary, b"different restored executable").unwrap();
        assert!(require_original_files(&installed, &request).is_err());
        assert_eq!(digest_file(&user_file).unwrap(), user_hash);
        fs::remove_file(&license).unwrap();
        assert!(verified_license(&root).is_err());
        fs::create_dir(&license).unwrap();
        assert!(verified_license(&root).is_err());
        assert_eq!(digest_file(&user_file).unwrap(), user_hash);
        fs::remove_dir_all(root).unwrap(); // Exact disposable fixture only.
    }

    #[test]
    fn fixed_release_contract_matches_the_inno_payload() {
        // A future added published file must extend the recovery contract, not
        // silently become an unknown file eligible for blanket deletion.
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packaging/windows/InstPlotStudio.iss"
        ));
        let mut section = "";
        let mut files = Vec::new();
        for line in source.lines().map(str::trim) {
            if line.starts_with('[') {
                section = line;
            } else if section == "[Files]" && !line.is_empty() && !line.starts_with(';') {
                files.push(line);
            }
        }
        assert_eq!(
            files,
            vec![
                "Source: \"{#SourceDir}\\instplot-studio.exe\"; DestDir: \"{app}\"; Flags: ignoreversion",
                "Source: \"{#SourceDir}\\LICENSE\"; DestDir: \"{app}\"; Flags: ignoreversion",
            ]
        );
    }

    #[test]
    fn ready_receipt_requires_actual_process_birth_and_transaction() {
        let request = request();
        let mut ready = HelperReady {
            transaction_id: request.transaction_id.clone(),
            nonce: request.nonce.clone(),
            process_id: 234,
            process_created: 567,
        };
        assert!(ready.matches(&request, 234, 567));
        assert!(!ready.matches(&request, 234, 568));
        assert!(!ready.matches(&request, 235, 567));
        assert!(!ready.matches(&request, 0, 0));
        ready.nonce = "4".repeat(64);
        assert!(!ready.matches(&request, 234, 567));
        let mut raw = serde_json::to_value(ready).unwrap();
        raw["initialized"] = true.into();
        assert!(
            serde_json::from_value::<HelperReady>(raw).is_err(),
            "ready is not a health receipt"
        );
    }

    #[test]
    fn helper_copy_is_private_exact_pinned_and_never_overwrites() {
        let mut random = [0_u8; 16];
        getrandom::fill(&mut random).unwrap();
        let root = std::env::temp_dir().join(format!(
            "studio-helper-copy 中文-{:032x}",
            u128::from_le_bytes(random)
        ));
        super::super::create_private_directory(&root).unwrap();
        let source = root.join("fixture old exe");
        let mut input = super::super::create_private_file(&source).unwrap();
        input
            .write_all(b"not executable; byte-copy and lease fixture only")
            .unwrap();
        drop(input);
        let destination = root.join(HELPER_NAME);
        let (lease, digest) = copy_helper(&source, &destination).unwrap();
        assert_eq!(digest, digest_file(&source).unwrap());
        assert_eq!(fs::read(&source).unwrap(), fs::read(&destination).unwrap());
        super::super::validate_private_file(&destination).unwrap();
        assert!(
            fs::OpenOptions::new()
                .write(true)
                .open(&destination)
                .is_err()
        );
        assert!(fs::remove_file(&destination).is_err());
        assert!(copy_helper(&source, &destination).is_err());
        drop(lease);
        let state = root.join("new.json");
        write_new_json(&state, &request()).unwrap();
        assert!(write_new_json(&state, &request()).is_err());
        assert!(read_private(&state, 1).is_err());
        let parsed: HelperRequest =
            serde_json::from_slice(&read_private(&state, 64 * 1024).unwrap()).unwrap();
        parsed.transaction().unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn copied_test_helper_readiness_requires_a_live_exact_child() {
        use std::process::{Command, Stdio};
        let mut random = [0_u8; 16];
        getrandom::fill(&mut random).unwrap();
        let root = std::env::temp_dir().join(format!(
            "studio-ready-child 中文-{:032x}",
            u128::from_le_bytes(random)
        ));
        super::super::create_private_directory(&root).unwrap();
        let root = fs::canonicalize(root).unwrap();
        let helper = root.join(HELPER_NAME);
        let (lease, _) = copy_helper(&std::env::current_exe().unwrap(), &helper).unwrap();
        // Real process/handle/birth-time evidence, but this is a test harness,
        // NOT a GUI or the complete updater session.
        let mut child = Command::new(&helper)
            .args(["--exact", "update_windows::assets::tests::installer_test_child_waits_for_normal_parent_pipe_close"])
            .env("STUDIO_INSTALLER_TEST_WAIT", "1")
            .stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null())
            .spawn().unwrap();
        let created = super::super::process::child_process_created(&child).unwrap();
        let ready_path = root.join("helper-ready.json");
        let request = request();
        let mut ready = HelperReady {
            transaction_id: request.transaction_id.clone(),
            nonce: request.nonce.clone(),
            process_id: child.id(),
            process_created: created,
        };
        write_new_json(&ready_path, &ready).unwrap();
        confirm_process_ready(&root, &request, &child).unwrap();
        ready.process_created += 1;
        super::super::write_private_atomic(&ready_path, &serde_json::to_vec(&ready).unwrap())
            .unwrap();
        assert!(confirm_process_ready(&root, &request, &child).is_err());
        ready.process_created = created;
        ready.nonce = "f".repeat(64);
        super::super::write_private_atomic(&ready_path, &serde_json::to_vec(&ready).unwrap())
            .unwrap();
        assert!(confirm_process_ready(&root, &request, &child).is_err());
        ready.nonce = request.nonce.clone();
        super::super::write_private_atomic(&ready_path, &serde_json::to_vec(&ready).unwrap())
            .unwrap();
        drop(child.stdin.take());
        assert!(child.wait().unwrap().success());
        assert!(
            confirm_process_ready(&root, &request, &child).is_err(),
            "a stale receipt from an exited helper never authorizes exit"
        );
        drop(lease);
        fs::remove_dir_all(root).unwrap();
    }
}
