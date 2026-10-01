//! macOS adapter: signed DMG preparation, out-of-process application and handshake.
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use atomicwrites::{AllowOverwrite, AtomicFile};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use instplot_studio::update_bundle::{BundleAccess, BundleExchange};
use instplot_studio::update_transaction::{
    HealthReceipt, TransactionStore, UpdateIdentity, UpdateStage, UpdateTransaction,
};
use instplot_studio::{
    AllowedUpdateRoot, PRODUCTION_PUBLIC_ROOT, PRODUCTION_TRUSTED_KEYS, UpdateChannel,
    verify_signed_manifest,
};

const EXECUTABLE: &str = "Contents/MacOS/instplot-studio";
const HEALTH_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HelperReady {
    transaction_id: String,
    nonce: String,
    process_id: u32,
}

impl HelperReady {
    fn matches(&self, transaction: &UpdateTransaction, process_id: u32) -> bool {
        self.transaction_id == transaction.id()
            && self.nonce == transaction.health_nonce()
            && self.process_id == process_id
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PreparedMacUpdate {
    directory: PathBuf,
    transaction: UpdateTransaction,
    previous_binary_hash: String,
    candidate_binary_hash: String,
    resume_project: Option<PathBuf>,
}

impl std::fmt::Debug for PreparedMacUpdate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedMacUpdate")
            .field("directory", &self.directory)
            .field("version", &self.transaction.identity().candidate_version)
            .finish_non_exhaustive()
    }
}

pub(super) fn current_bundle() -> Result<PathBuf, String> {
    let executable = fs::canonicalize(std::env::current_exe().map_err(error)?).map_err(error)?;
    let target = executable
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .ok_or_else(|| "无法确定应用安装位置。".to_owned())?;
    if target
        .extension()
        .is_none_or(|extension| extension != "app")
        || target.join(EXECUTABLE) != executable
    {
        return Err("请先将应用安装到持久可写目录，开发程序或便携二进制不能原位更新。".into());
    }
    let temporary = fs::canonicalize(std::env::temp_dir()).map_err(error)?;
    if target.starts_with(&temporary)
        || target.starts_with("/Volumes")
        || target.to_string_lossy().contains("AppTranslocation")
    {
        return Err("从磁盘映像或临时位置运行时不能原位更新，请先安装应用。".into());
    }
    Ok(target.to_path_buf())
}

pub(super) fn has_unfinished_apply(target: &Path) -> bool {
    let Some(parent) = target.parent() else {
        return true;
    };
    let Ok(entries) = fs::read_dir(parent) else {
        return true;
    };
    for entry in entries.flatten().filter(|entry| {
        entry
            .file_name()
            .to_string_lossy()
            .starts_with(".instplot-studio-update-")
    }) {
        let Ok(raw) = read_bounded(&entry.path().join("transaction.json"), 64 * 1024) else {
            if entry.path().join("transaction.json").exists() {
                return true;
            }
            continue;
        };
        let Ok(state) = serde_json::from_slice::<UpdateTransaction>(&raw) else {
            return true;
        };
        if state.identity().installed_path == target && state.stage().needs_recovery_inspection() {
            return true;
        }
    }
    false
}

pub(super) fn failure_notice() -> Option<String> {
    let target = current_bundle().ok()?;
    let mut entries = fs::read_dir(target.parent()?)
        .ok()?
        .flatten()
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.metadata().ok().and_then(|m| m.modified().ok()));
    for entry in entries.into_iter().rev().filter(|entry| {
        entry
            .file_name()
            .to_string_lossy()
            .starts_with(".instplot-studio-update-")
    }) {
        let path = entry.path();
        let raw = match read_bounded(&path.join("transaction.json"), 64 * 1024) {
            Ok(raw) => raw,
            Err(_) => continue,
        };
        let state: UpdateTransaction = match serde_json::from_slice(&raw) {
            Ok(state) => state,
            Err(_) => continue,
        };
        if state.identity().installed_path != target || path.join("reported").exists() {
            continue;
        }
        if !matches!(
            state.stage(),
            UpdateStage::RolledBack | UpdateStage::FailedBeforeApply
        ) {
            continue;
        }
        use std::os::unix::fs::OpenOptionsExt;
        if OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path.join("reported"))
            .is_err()
        {
            continue;
        }
        return Some(format!(
            "上次原位升级未完成，当前使用旧版。{}\n事务与日志：{}",
            state.last_error().unwrap_or("已保留旧应用。"),
            path.display()
        ));
    }
    None
}

pub(super) fn prepare(
    package_path: &Path,
    expected_version: &str,
) -> Result<PreparedMacUpdate, String> {
    let target = current_bundle()?;
    verify_bundle(&target, env!("CARGO_PKG_VERSION"), None)?;
    let cache = package_path
        .parent()
        .ok_or_else(|| "更新缓存无效。".to_owned())?;
    let raw = read_bounded(&cache.join("manifest.json"), 256 * 1024)?;
    let signature = read_bounded(&cache.join("manifest.json.sig"), 64)?;
    let root = AllowedUpdateRoot::parse(PRODUCTION_PUBLIC_ROOT).map_err(error)?;
    let current = semver::Version::parse(env!("CARGO_PKG_VERSION")).map_err(error)?;
    let channel = UpdateChannel::for_version(&current);
    let manifest =
        verify_signed_manifest(&raw, &signature, &PRODUCTION_TRUSTED_KEYS, &root, channel)
            .map_err(error)?;
    if manifest.version != expected_version
        || semver::Version::parse(&manifest.version).map_err(error)? <= current
    {
        return Err("更新版本与已验证清单不一致。".into());
    }
    let packages = manifest
        .platforms
        .get("macos-aarch64")
        .ok_or_else(|| "缺少 macOS 更新包。".to_owned())?;
    let package = packages
        .packages
        .iter()
        .find(|package| package.id == packages.preferred)
        .ok_or_else(|| "缺少首选更新包。".to_owned())?;
    if package_path
        .file_name()
        .is_none_or(|name| name != package.file_name.as_str())
        || fs::metadata(package_path).map_err(error)?.len() != package.size_bytes
        || digest(package_path)? != package.sha256
        || package.package_type != "dmg"
    {
        return Err("更新包未通过重新校验。".into());
    }
    let transaction = UpdateTransaction::new(UpdateIdentity {
        product: "instplot-studio".into(),
        platform: "macos-aarch64".into(),
        installed_path: target.clone(),
        previous_version: current.to_string(),
        candidate_version: manifest.version,
        candidate_sha256: package.sha256.clone(),
        candidate_size: package.size_bytes,
    })
    .map_err(error)?;
    let directory = target
        .parent()
        .unwrap()
        .join(format!(".instplot-studio-update-{}", transaction.id()));
    private_directory(&directory)?;
    fs::write(directory.join(".metadata_never_index"), []).map_err(error)?;
    let store = TransactionStore::lock(&directory).map_err(error)?;
    store.write(&transaction).map_err(error)?;
    let mount = directory.join("mount");
    private_directory(&mount)?;
    run(
        "/usr/bin/hdiutil",
        &[
            "attach".as_ref(),
            "-readonly".as_ref(),
            "-nobrowse".as_ref(),
            "-mountpoint".as_ref(),
            mount.as_os_str(),
            package_path.as_os_str(),
        ],
    )?;
    let staged = (|| {
        let source = mount.join("InstPlot Studio.app");
        verify_bundle(&source, &transaction.identity().candidate_version, None)?;
        let source_hash = digest(&source.join(EXECUTABLE))?;
        run(
            "/usr/bin/ditto",
            &[
                source.as_os_str(),
                directory.join("candidate.app").as_os_str(),
            ],
        )?;
        verify_bundle(
            &directory.join("candidate.app"),
            &transaction.identity().candidate_version,
            Some(&source_hash),
        )?;
        let protocol = output(
            &directory.join("candidate.app").join(EXECUTABLE),
            &["--update-protocol".as_ref()],
        )?;
        if protocol.trim() != "1" {
            return Err("候选应用不支持本次安全更新协议。".to_owned());
        }
        Ok(source_hash)
    })();
    let detached = run("/usr/bin/hdiutil", &["detach".as_ref(), mount.as_os_str()]);
    let candidate_binary_hash = staged?;
    detached?;
    let previous_binary_hash = digest(&target.join(EXECUTABLE))?;
    let helper = directory.join("update-helper");
    fs::copy(target.join(EXECUTABLE), &helper).map_err(error)?;
    if digest(&helper)? != previous_binary_hash {
        return Err("更新助手复制校验失败。".into());
    }
    let prepared = PreparedMacUpdate {
        directory,
        transaction,
        previous_binary_hash,
        candidate_binary_hash,
        resume_project: None,
    };
    prepared.write_request()?;
    Ok(prepared)
}

impl PreparedMacUpdate {
    fn write_request(&self) -> Result<(), String> {
        write_json(&self.directory.join("request.json"), self)
    }

    pub(super) fn launch(mut self, project: Option<PathBuf>) -> Result<(), String> {
        // Recheck identity after the user has saved/confirmed the exit.
        if current_bundle()? != self.transaction.identity().installed_path {
            return Err("安装位置已改变。".into());
        }
        verify_bundle(
            &self.transaction.identity().installed_path,
            &self.transaction.identity().previous_version,
            Some(&self.previous_binary_hash),
        )?;
        verify_bundle(
            &self.directory.join("candidate.app"),
            &self.transaction.identity().candidate_version,
            Some(&self.candidate_binary_hash),
        )?;
        self.resume_project = project;
        self.write_request()?;
        let store = TransactionStore::lock(&self.directory).map_err(error)?;
        self.transaction
            .transition(UpdateStage::WaitingForExit)
            .map_err(error)?;
        store.write(&self.transaction).map_err(error)?;
        drop(store);
        let helper = self.directory.join("update-helper");
        if digest(&helper)? != self.previous_binary_hash {
            return Err("更新助手已改变。".into());
        }
        // Helper startup acknowledgment is intentionally required before closing.
        let ready = self.directory.join("helper-ready");
        if ready.try_exists().map_err(error)? {
            return Err("更新助手已启动，请勿重复重启。".into());
        }
        let log = private_log(&self.directory)?;
        let mut child = Command::new(helper)
            .arg("--apply-update")
            .arg(&self.directory)
            .stdin(Stdio::null())
            .stdout(log.try_clone().map_err(error)?)
            .stderr(log)
            .spawn()
            .map_err(error)?;
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(5) {
            if let Ok(raw) = read_bounded(&ready, 4096)
                && let Ok(receipt) = serde_json::from_slice::<HelperReady>(&raw)
                && receipt.matches(&self.transaction, child.id())
            {
                return Ok(());
            }
            if child.try_wait().map_err(error)?.is_some() {
                return Err("更新助手提前退出；当前应用不会退出，请查看更新日志。".into());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        Err("更新助手没有确认就绪；当前应用不会退出。".into())
    }
}

pub(super) fn apply(directory: &Path) -> Result<(), String> {
    let result = apply_inner(directory);
    if let Err(ref failure) = result
        && let Ok(request) = load_request(directory)
        && let Ok(store) = TransactionStore::lock(directory)
        && let Ok(mut state) = store.read(request.transaction.identity())
    {
        state.record_error(failure);
        if state.stage().needs_recovery_inspection() {
            if state.stage() != UpdateStage::RecoveryRequired {
                state
                    .transition(UpdateStage::RecoveryRequired)
                    .map_err(error)?;
            }
            write_json(&directory.join("stop-requested"), &true)?;
        } else if state.stage() == UpdateStage::WaitingForExit {
            state
                .transition(UpdateStage::FailedBeforeApply)
                .map_err(error)?;
        }
        store.write(&state).map_err(error)?;
    }
    result
}

fn apply_inner(directory: &Path) -> Result<(), String> {
    let request = load_request(directory)?;
    if digest(&std::env::current_exe().map_err(error)?)? != request.previous_binary_hash {
        return Err("更新助手身份不匹配。".into());
    }
    let store = TransactionStore::lock(directory).map_err(error)?;
    let state = store.read(request.transaction.identity()).map_err(error)?;
    let mut exchange = BundleExchange::open(&store, state, directory).map_err(error)?;
    if exchange.state().stage() == UpdateStage::Completed
        || exchange.state().stage() == UpdateStage::RolledBack
    {
        return Ok(());
    }
    let recovering = exchange.state().stage().needs_recovery_inspection();
    if !recovering && exchange.state().stage() != UpdateStage::WaitingForExit {
        return Err("更新事务阶段无效。".into());
    }
    write_json(
        &directory.join("helper-ready"),
        &HelperReady {
            transaction_id: exchange.state().id().to_owned(),
            nonce: exchange.state().health_nonce().to_owned(),
            process_id: std::process::id(),
        },
    )?;
    if recovering {
        write_json(&directory.join("stop-requested"), &true)?;
    }
    let target = request.transaction.identity().installed_path.clone();
    let deadline = Instant::now();
    let access = loop {
        if let Ok(access) = BundleAccess::exclusive(&target) {
            break access;
        }
        if deadline.elapsed() >= Duration::from_secs(30) {
            return Err("其他实例没有退出，尚未替换应用。".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let verifier = |path: &Path, version: &str| {
        let hash = if version == request.transaction.identity().previous_version {
            &request.previous_binary_hash
        } else {
            &request.candidate_binary_hash
        };
        verify_bundle(path, version, Some(hash)).map_err(std::io::Error::other)
    };
    if recovering {
        exchange.restore(&access, verifier).map_err(error)?;
        drop(access);
        restart_previous(&request)?;
        return Ok(());
    }
    if let Err(failure) = exchange.apply(&access, verifier) {
        if exchange.state().stage().needs_recovery_inspection() {
            exchange.restore(&access, verifier).map_err(error)?;
        }
        drop(access);
        let _ = restart_previous(&request);
        return Err(error(failure));
    }
    // Release exclusive access so only the candidate can acquire a lifetime shared guard.
    drop(access);
    let mut candidate = match Command::new(target.join(EXECUTABLE))
        .arg("--update-health")
        .arg(directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(failure) => {
            let access = BundleAccess::exclusive(&target).map_err(error)?;
            exchange.restore(&access, verifier).map_err(error)?;
            drop(access);
            let _ = restart_previous(&request);
            return Err(error(failure));
        }
    };
    let started = process_start(candidate.id())?;
    exchange
        .await_health(candidate.id(), started)
        .map_err(error)?;
    let wait = Instant::now();
    let mut success = false;
    while wait.elapsed() < HEALTH_TIMEOUT {
        let path = directory.join("health.json");
        if path.is_file()
            && let Ok(raw) = read_bounded(&path, 16 * 1024)
            && let Ok(receipt) = serde_json::from_slice::<HealthReceipt>(&raw)
            && process_start(candidate.id()).ok().as_deref()
                == Some(receipt.process_started.as_str())
            && exchange.accept_health(&receipt).is_ok()
        {
            success = true;
            break;
        }
        if candidate.try_wait().map_err(error)?.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    if success {
        return Ok(());
    }
    // A pre-health candidate has never enabled editing; ask it to exit normally.
    write_json(&directory.join("stop-requested"), &true)?;
    let wait = Instant::now();
    while candidate.try_wait().map_err(error)?.is_none() && wait.elapsed() < Duration::from_secs(10)
    {
        std::thread::sleep(Duration::from_millis(100));
    }
    let access = BundleAccess::exclusive(&target)
        .map_err(|_| "候选仍在运行；恢复已暂停，未强制关闭或覆盖。".to_owned())?;
    exchange.restore(&access, verifier).map_err(error)?;
    drop(access);
    restart_previous(&request)?;
    Err("新版没有完成健康确认，已恢复旧版。".into())
}

fn restart_previous(request: &PreparedMacUpdate) -> Result<(), String> {
    let mut command = Command::new(
        request
            .transaction
            .identity()
            .installed_path
            .join(EXECUTABLE),
    );
    if let Some(project) = &request.resume_project {
        command.arg(project);
    }
    command.spawn().map_err(error)?;
    Ok(())
}

pub(super) struct HealthStartup {
    request: PreparedMacUpdate,
    receipt_written: bool,
}
impl HealthStartup {
    pub(super) fn load(directory: &Path) -> Result<Self, String> {
        let request = load_request(directory)?;
        if current_bundle()? != request.transaction.identity().installed_path
            || env!("CARGO_PKG_VERSION") != request.transaction.identity().candidate_version
        {
            return Err("更新启动身份不匹配。".into());
        }
        verify_bundle(
            &current_bundle()?,
            env!("CARGO_PKG_VERSION"),
            Some(&request.candidate_binary_hash),
        )?;
        if let Some(project) = &request.resume_project {
            let (document, _) = instplot_studio::FigureDocument::open(project).map_err(error)?;
            document.layout_figure().map_err(error)?;
        }
        Ok(Self {
            request,
            receipt_written: false,
        })
    }
    pub(super) fn project(&self) -> Option<PathBuf> {
        self.request.resume_project.clone()
    }
    /// Called after the first initialized main canvas has been produced; returns
    /// true only after the helper commits, keeping editing disabled until then.
    pub(super) fn frame_ready(&mut self, context: &eframe::egui::Context) -> Result<bool, String> {
        let directory = &self.request.directory;
        if directory.join("stop-requested").is_file() {
            context.send_viewport_cmd(eframe::egui::ViewportCommand::Close);
            return Ok(false);
        }
        if !self.receipt_written {
            let receipt = HealthReceipt {
                transaction_id: self.request.transaction.id().into(),
                nonce: self.request.transaction.health_nonce().into(),
                product: "instplot-studio".into(),
                platform: "macos-aarch64".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                installed_path: current_bundle()?,
                process_id: std::process::id(),
                process_started: process_start(std::process::id())?,
                initialized: true,
                window_ready: true,
            };
            write_json(&directory.join("health.json"), &receipt)?;
            self.receipt_written = true;
        }
        // The helper holds the store lock; read only its atomic journal snapshot.
        let state: UpdateTransaction = serde_json::from_slice(&read_bounded(
            &directory.join("transaction.json"),
            64 * 1024,
        )?)
        .map_err(error)?;
        context.request_repaint_after(Duration::from_millis(100));
        Ok(state.id() == self.request.transaction.id()
            && state.identity() == self.request.transaction.identity()
            && state.stage() == UpdateStage::Completed)
    }
}

fn load_request(directory: &Path) -> Result<PreparedMacUpdate, String> {
    let directory = fs::canonicalize(directory).map_err(error)?;
    use std::os::unix::fs::PermissionsExt;
    if fs::metadata(&directory)
        .map_err(error)?
        .permissions()
        .mode()
        & 0o077
        != 0
    {
        return Err("更新目录权限无效。".into());
    }
    let request: PreparedMacUpdate =
        serde_json::from_slice(&read_bounded(&directory.join("request.json"), 64 * 1024)?)
            .map_err(error)?;
    if request.directory != directory
        || directory.parent() != request.transaction.identity().installed_path.parent()
        || directory.file_name().is_none_or(|name| {
            name != format!(".instplot-studio-update-{}", request.transaction.id()).as_str()
        })
        || request.transaction.identity().product != "instplot-studio"
    {
        return Err("更新事务路径或产品身份无效。".into());
    }
    Ok(request)
}

fn verify_bundle(path: &Path, version: &str, expected_hash: Option<&str>) -> Result<(), String> {
    if !fs::symlink_metadata(path).map_err(error)?.is_dir() {
        return Err("应用 bundle 无效。".into());
    }
    let executable = path.join(EXECUTABLE);
    if expected_hash.is_some_and(|expected| digest(&executable).as_deref() != Ok(expected)) {
        return Err("应用程序哈希已改变。".into());
    }
    run(
        "/usr/bin/codesign",
        &[
            "--verify".as_ref(),
            "--deep".as_ref(),
            "--strict".as_ref(),
            path.as_os_str(),
        ],
    )?;
    let plist = path.join("Contents/Info.plist");
    let bundle_id = output(
        Path::new("/usr/bin/plutil"),
        &[
            "-extract".as_ref(),
            "CFBundleIdentifier".as_ref(),
            "raw".as_ref(),
            "-o".as_ref(),
            "-".as_ref(),
            plist.as_os_str(),
        ],
    )?;
    if bundle_id.trim() != "com.instplot.studio" {
        return Err("应用 bundle ID 不匹配。".into());
    }
    let bundle_version = output(
        Path::new("/usr/bin/plutil"),
        &[
            "-extract".as_ref(),
            "CFBundleShortVersionString".as_ref(),
            "raw".as_ref(),
            "-o".as_ref(),
            "-".as_ref(),
            plist.as_os_str(),
        ],
    )?;
    if bundle_version.trim() != version {
        return Err("应用 bundle 版本不匹配。".into());
    }
    let architecture = output(
        Path::new("/usr/bin/lipo"),
        &["-archs".as_ref(), executable.as_os_str()],
    )?;
    if architecture.trim() != "arm64" {
        return Err("更新包不是 macOS arm64。".into());
    }
    let identity = output(&executable, &["--product-info".as_ref()])?;
    if identity.trim() != format!("InstPlot Studio\tinstplot-studio\t{version}") {
        return Err("应用产品或版本不匹配。".into());
    }
    Ok(())
}

fn process_start(pid: u32) -> Result<String, String> {
    let value = output(
        Path::new("/bin/ps"),
        &[
            "-p".as_ref(),
            pid.to_string().as_ref(),
            "-o".as_ref(),
            "lstart=".as_ref(),
        ],
    )?;
    if value.trim().is_empty() {
        return Err("无法确认更新进程身份。".into());
    }
    Ok(value.trim().to_owned())
}

fn digest(path: &Path) -> Result<String, String> {
    let metadata = fs::symlink_metadata(path).map_err(error)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("更新文件不能是链接或目录。".into());
    }
    let mut file = File::open(path).map_err(error)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(error)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{digest:x}", digest = digest.finalize()))
}

fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let metadata = fs::symlink_metadata(path).map_err(error)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > limit {
        return Err("更新状态文件无效。".into());
    }
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(error)?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(error)?;
    if bytes.len() as u64 > limit {
        return Err("更新状态超出大小限制。".into());
    }
    Ok(bytes)
}

fn private_directory(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::DirBuilderExt;
    fs::DirBuilder::new()
        .mode(0o700)
        .create(path)
        .map_err(error)
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    if fs::symlink_metadata(path)
        .is_ok_and(|metadata| !metadata.is_file() || metadata.file_type().is_symlink())
    {
        return Err("更新状态文件不能是链接或目录。".into());
    }
    let raw = serde_json::to_vec(value).map_err(error)?;
    AtomicFile::new(path, AllowOverwrite)
        .write(|file| {
            file.write_all(&raw)?;
            file.sync_all()
        })
        .map_err(error)
}

fn run(executable: &str, arguments: &[&std::ffi::OsStr]) -> Result<(), String> {
    output(Path::new(executable), arguments).map(|_| ())
}

fn output(executable: &Path, arguments: &[&std::ffi::OsStr]) -> Result<String, String> {
    let mut child = Command::new(executable)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(error)?;
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(error)? {
            break status;
        }
        if started.elapsed() >= Duration::from_secs(40) {
            // Only our bounded verification subprocess is terminated, never a
            // GUI application or an updater candidate with editable work.
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("{} 验证超时。", executable.display()));
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    if !status.success() {
        return Err(format!("{} 操作失败（{}）。", executable.display(), status));
    }
    let mut raw = Vec::new();
    child
        .stdout
        .take()
        .unwrap()
        .take(64 * 1024 + 1)
        .read_to_end(&mut raw)
        .map_err(error)?;
    if raw.len() > 64 * 1024 {
        return Err("验证输出过大。".into());
    }
    String::from_utf8(raw).map_err(error)
}

fn private_log(directory: &Path) -> Result<File, String> {
    use std::os::unix::fs::OpenOptionsExt;
    let path = directory.join("update.log");
    if fs::symlink_metadata(&path).is_ok_and(|m| !m.is_file() || m.file_type().is_symlink()) {
        return Err("更新日志路径无效。".into());
    }
    OpenOptions::new()
        .append(true)
        .create(true)
        .mode(0o600)
        .open(path)
        .map_err(error)
}

fn error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        root: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let mut bytes = [0_u8; 16];
            getrandom::fill(&mut bytes).unwrap();
            let root = std::env::temp_dir().join(format!(
                "instplot-macos-adapter-{:x}",
                Sha256::digest(bytes)
            ));
            private_directory(&root).unwrap();
            Self {
                root: fs::canonicalize(root).unwrap(),
            }
        }
        fn bundle(&self, name: &str, version: &str) -> PathBuf {
            let bundle = self.root.join(name);
            fs::create_dir_all(bundle.join("Contents/MacOS")).unwrap();
            let executable = bundle.join(EXECUTABLE);
            let source =
                Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/update_probe.c");
            let define = format!("-DINSTPLOT_PROBE_VERSION=\"{version}\"");
            run(
                "/usr/bin/clang",
                &[
                    define.as_ref(),
                    source.as_os_str(),
                    "-o".as_ref(),
                    executable.as_os_str(),
                ],
            )
            .unwrap();
            let plist = format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?><plist version=\"1.0\"><dict><key>CFBundleIdentifier</key><string>com.instplot.studio</string><key>CFBundleExecutable</key><string>instplot-studio</string><key>CFBundlePackageType</key><string>APPL</string><key>CFBundleShortVersionString</key><string>{version}</string></dict></plist>"
            );
            fs::write(bundle.join("Contents/Info.plist"), plist).unwrap();
            run(
                "/usr/bin/codesign",
                &[
                    "--force".as_ref(),
                    "--sign".as_ref(),
                    "-".as_ref(),
                    bundle.as_os_str(),
                ],
            )
            .unwrap();
            bundle
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    #[test]
    fn helper_readiness_is_bound_to_transaction_nonce_and_process() {
        let fixture = Fixture::new();
        let transaction = UpdateTransaction::new(UpdateIdentity {
            product: "instplot-studio".into(),
            platform: "macos-aarch64".into(),
            installed_path: fixture.root.join("Studio.app"),
            previous_version: "0.1.2-rc.2".into(),
            candidate_version: "0.1.2-rc.3".into(),
            candidate_sha256: "a".repeat(64),
            candidate_size: 100,
        })
        .unwrap();
        let mut receipt = HelperReady {
            transaction_id: transaction.id().into(),
            nonce: transaction.health_nonce().into(),
            process_id: 123,
        };
        assert!(receipt.matches(&transaction, 123));
        assert!(!receipt.matches(&transaction, 124));
        receipt.nonce = "0".repeat(64);
        assert!(!receipt.matches(&transaction, 123));
        receipt.nonce = transaction.health_nonce().into();
        receipt.transaction_id = "0".repeat(32);
        assert!(!receipt.matches(&transaction, 123));
    }

    #[test]
    fn real_adhoc_bundle_verification_rejects_version_hash_and_tampering() {
        let fixture = Fixture::new();
        let bundle = fixture.bundle("Probe.app", "0.1.3-rc.1");
        let hash = digest(&bundle.join(EXECUTABLE)).unwrap();
        verify_bundle(&bundle, "0.1.3-rc.1", Some(&hash)).unwrap();
        assert!(verify_bundle(&bundle, "0.1.3-rc.2", Some(&hash)).is_err());
        assert!(verify_bundle(&bundle, "0.1.3-rc.1", Some(&"0".repeat(64))).is_err());
        fs::write(bundle.join("Contents/Info.plist"), b"tampered").unwrap();
        assert!(verify_bundle(&bundle, "0.1.3-rc.1", None).is_err());
    }

    #[test]
    fn real_signed_bundle_exchange_restores_previous_identity() {
        let fixture = Fixture::new();
        let target = fixture.bundle("InstPlot Studio.app", "0.1.3-rc.1");
        let candidate = fixture.bundle("Candidate.app", "0.1.3-rc.2");
        let staging = fixture.root.join("transaction");
        private_directory(&staging).unwrap();
        fs::rename(&candidate, staging.join("candidate.app")).unwrap();
        let mut state = UpdateTransaction::new(UpdateIdentity {
            product: "instplot-studio".into(),
            platform: "macos-aarch64".into(),
            installed_path: target.clone(),
            previous_version: "0.1.3-rc.1".into(),
            candidate_version: "0.1.3-rc.2".into(),
            candidate_sha256: "a".repeat(64),
            candidate_size: 1,
        })
        .unwrap();
        state.transition(UpdateStage::WaitingForExit).unwrap();
        let store = TransactionStore::lock(&staging).unwrap();
        let access = BundleAccess::exclusive(&target).unwrap();
        let mut exchange = BundleExchange::open(&store, state, &staging).unwrap();
        let verify = |path: &Path, version: &str| {
            verify_bundle(path, version, None).map_err(std::io::Error::other)
        };
        exchange.apply(&access, verify).unwrap();
        verify_bundle(&target, "0.1.3-rc.2", None).unwrap();
        exchange
            .await_health(100, "probe-not-healthy".into())
            .unwrap();
        exchange.restore(&access, verify).unwrap();
        verify_bundle(&target, "0.1.3-rc.1", None).unwrap();
        assert_eq!(exchange.state().stage(), UpdateStage::RolledBack);
    }

    #[test]
    fn state_reader_rejects_links_and_oversized_files() {
        let fixture = Fixture::new();
        let file = fixture.root.join("state");
        fs::write(&file, b"abcd").unwrap();
        assert!(read_bounded(&file, 3).is_err());
        let link = fixture.root.join("link");
        std::os::unix::fs::symlink(file, &link).unwrap();
        assert!(read_bounded(&link, 10).is_err());
    }
}
