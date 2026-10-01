use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use atomicwrites::{AllowOverwrite, AtomicFile};
use directories::ProjectDirs;
use eframe::egui;
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;

use instplot_studio::{
    AllowedUpdateRoot, GITHUB_RELEASES_ROOT, PRODUCTION_PUBLIC_ROOT, PRODUCTION_TRUSTED_KEYS,
    UpdateChannel, UpdatePackage, production_latest_url, signature_url_from_manifest_bytes,
    verify_signed_manifest,
};

use crate::ui_chrome::{studio_card_frame, studio_surface};

const MAX_MANIFEST_BYTES: usize = 256 * 1024;
const MAX_SIGNATURE_BYTES: usize = 64;
const MAX_PACKAGE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_REDIRECTS: usize = 3;
const NETWORK_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Clone, Debug)]
pub(super) struct AvailableUpdate {
    pub(super) version: String,
    pub(super) notes_url: String,
    pub(super) package: UpdatePackage,
    pub(super) signed_manifest: Vec<u8>,
    pub(super) manifest_signature: Vec<u8>,
}

#[derive(Clone, Debug)]
enum CheckOutcome {
    UpToDate { version: String },
    Available(Box<AvailableUpdate>),
    Unsupported { explanation: String },
}

#[derive(Clone, Debug)]
enum UpdatePhase {
    Idle,
    Checking,
    UpToDate {
        version: String,
    },
    Available(AvailableUpdate),
    Unsupported {
        explanation: String,
    },
    Failed {
        explanation: String,
    },
    Downloading {
        update: AvailableUpdate,
        destination: PathBuf,
        downloaded: u64,
    },
    Downloaded {
        path: PathBuf,
        update: AvailableUpdate,
    },
    #[cfg(target_os = "macos")]
    PreparingMac,
    #[cfg(target_os = "macos")]
    ReadyMac(crate::update_macos::PreparedMacUpdate),
    #[cfg(target_os = "macos")]
    LaunchingHelper,
}

enum UpdateEvent {
    BackgroundSkipped,
    Checked(Result<CheckOutcome, String>),
    Progress(u64),
    Downloaded(Box<Result<(AvailableUpdate, PathBuf), String>>),
    #[cfg(target_os = "macos")]
    PreparedMac(Box<Result<crate::update_macos::PreparedMacUpdate, String>>),
    #[cfg(target_os = "macos")]
    HelperReady(Result<(), String>),
}

pub(super) struct AppUpdateState {
    pub(super) open: bool,
    pub(super) focus: bool,
    phase: UpdatePhase,
    receiver: Option<mpsc::Receiver<UpdateEvent>>,
    cancel: Arc<AtomicBool>,
    background_started: bool,
    background_check: bool,
    restart_requested: bool,
    close_requested: bool,
    #[cfg(target_os = "macos")]
    instance_guard: Result<instplot_studio::update_bundle::BundleAccess, String>,
    #[cfg(all(windows, feature = "in-place-update-preview"))]
    _windows_instance_guard: Result<instplot_studio::update_windows::WindowsInstallAccess, String>,
}

impl Default for AppUpdateState {
    fn default() -> Self {
        Self {
            open: false,
            focus: false,
            phase: UpdatePhase::Idle,
            receiver: None,
            cancel: Arc::new(AtomicBool::new(false)),
            background_started: false,
            background_check: false,
            restart_requested: false,
            close_requested: false,
            #[cfg(target_os = "macos")]
            instance_guard: Err("尚未初始化安装进程锁。".into()),
            #[cfg(all(windows, feature = "in-place-update-preview"))]
            _windows_instance_guard: Err("尚未初始化安装进程锁。".into()),
        }
    }
}

impl Drop for AppUpdateState {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

impl AppUpdateState {
    #[cfg(all(windows, feature = "in-place-update-preview"))]
    pub(super) fn set_windows_instance_guard(
        &mut self,
        guard: Result<instplot_studio::update_windows::WindowsInstallAccess, String>,
    ) {
        // Lifetime ownership only. Windows update UI remains disabled until
        // full installer/health/recovery acceptance is complete.
        self._windows_instance_guard = guard;
    }

    #[cfg(target_os = "macos")]
    pub(super) fn set_instance_guard(
        &mut self,
        guard: Result<instplot_studio::update_bundle::BundleAccess, String>,
    ) {
        self.instance_guard = guard;
        if let Some(explanation) = crate::update_macos::failure_notice() {
            self.explain_blocked(explanation);
        }
    }
    pub(super) fn needs_work_inventory(&self) -> bool {
        if !self.open {
            return false;
        }
        match self.phase {
            UpdatePhase::Downloaded { .. } => true,
            #[cfg(target_os = "macos")]
            UpdatePhase::ReadyMac(_) => true,
            _ => false,
        }
    }

    pub(super) fn is_launching(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            matches!(self.phase, UpdatePhase::LaunchingHelper)
        }
        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }
    fn is_preparing(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            matches!(
                self.phase,
                UpdatePhase::PreparingMac | UpdatePhase::ReadyMac(_)
            )
        }
        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }
    pub(super) fn take_restart_request(&mut self) -> bool {
        std::mem::take(&mut self.restart_requested)
    }
    pub(super) fn take_close_request(&mut self) -> bool {
        std::mem::take(&mut self.close_requested)
    }
    pub(super) fn explain_blocked(&mut self, explanation: String) {
        self.open = true;
        self.phase = UpdatePhase::Failed { explanation };
    }

    pub(super) fn launch_helper(&mut self, project: Option<PathBuf>) {
        #[cfg(target_os = "macos")]
        {
            let UpdatePhase::ReadyMac(prepared) = self.phase.clone() else {
                return;
            };
            match self
                .instance_guard
                .as_ref()
                .map_err(Clone::clone)
                .and_then(|guard| guard.other_instances().map_err(|e| e.to_string()))
            {
                Ok(false) => {}
                Ok(true) => {
                    self.explain_blocked("请先正常关闭同一安装的其他实例。".into());
                    return;
                }
                Err(error) => {
                    self.explain_blocked(error);
                    return;
                }
            }
            self.phase = UpdatePhase::LaunchingHelper;
            let (sender, receiver) = mpsc::channel();
            self.receiver = Some(receiver);
            std::thread::spawn(move || {
                let _ = sender.send(UpdateEvent::HelperReady(prepared.launch(project)));
            });
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = project;
            self.explain_blocked("该安装方式的安全原位升级尚未通过平台验证。".into());
        }
    }

    pub(super) fn request_check(&mut self, context: &egui::Context) {
        self.open = true;
        self.focus = true;
        if matches!(
            self.phase,
            UpdatePhase::Checking | UpdatePhase::Downloading { .. }
        ) || self.is_launching()
            || self.is_preparing()
        {
            return;
        }
        self.background_check = false;
        self.begin_check(context);
    }

    fn begin_check(&mut self, context: &egui::Context) {
        self.cancel.store(false, Ordering::Relaxed);
        self.phase = UpdatePhase::Checking;
        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        let context = context.clone();
        std::thread::spawn(move || {
            let result = check_for_update();
            let _ = sender.send(UpdateEvent::Checked(result));
            context.request_repaint();
        });
    }

    fn start_background_check(&mut self, context: &egui::Context) {
        if self.background_started {
            return;
        }
        self.background_started = true;
        if cfg!(debug_assertions) || !matches!(self.phase, UpdatePhase::Idle) {
            return;
        }
        self.background_check = true;
        self.phase = UpdatePhase::Checking;
        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        let context = context.clone();
        std::thread::spawn(move || {
            let result = reserve_background_check().and_then(|due| {
                if due {
                    check_for_update().map(Some)
                } else {
                    Ok(None)
                }
            });
            let event = match result {
                Ok(Some(outcome)) => UpdateEvent::Checked(Ok(outcome)),
                Ok(None) => UpdateEvent::BackgroundSkipped,
                Err(error) => UpdateEvent::Checked(Err(error)),
            };
            let _ = sender.send(event);
            context.request_repaint();
        });
    }

    fn poll(&mut self, context: &egui::Context) {
        let Some(receiver) = self.receiver.take() else {
            return;
        };
        let mut keep_receiver = true;
        loop {
            let event = match receiver.try_recv() {
                Ok(event) => event,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    if keep_receiver {
                        self.phase = UpdatePhase::Failed {
                            explanation: "更新任务意外终止，请重试。".to_owned(),
                        };
                        keep_receiver = false;
                    }
                    break;
                }
            };
            match event {
                #[cfg(target_os = "macos")]
                UpdateEvent::PreparedMac(result) => {
                    self.phase = match *result {
                        Ok(prepared) => UpdatePhase::ReadyMac(prepared),
                        Err(explanation) => UpdatePhase::Failed { explanation },
                    };
                    keep_receiver = false;
                }
                #[cfg(target_os = "macos")]
                UpdateEvent::HelperReady(result) => {
                    match result {
                        Ok(()) => self.close_requested = true,
                        Err(explanation) => self.phase = UpdatePhase::Failed { explanation },
                    }
                    keep_receiver = false;
                }
                UpdateEvent::BackgroundSkipped => {
                    self.phase = UpdatePhase::Idle;
                    keep_receiver = false;
                }
                UpdateEvent::Checked(Ok(CheckOutcome::UpToDate { version })) => {
                    self.phase = UpdatePhase::UpToDate { version };
                    keep_receiver = false;
                }
                UpdateEvent::Checked(Ok(CheckOutcome::Available(update))) => {
                    self.phase = UpdatePhase::Available(*update);
                    self.open = true;
                    self.focus = true;
                    keep_receiver = false;
                }
                UpdateEvent::Checked(Ok(CheckOutcome::Unsupported { explanation })) => {
                    self.phase = UpdatePhase::Unsupported { explanation };
                    keep_receiver = false;
                }
                UpdateEvent::Checked(Err(explanation)) => {
                    self.phase = if self.background_check {
                        UpdatePhase::Idle
                    } else {
                        UpdatePhase::Failed { explanation }
                    };
                    keep_receiver = false;
                }
                UpdateEvent::Progress(downloaded) => {
                    if let UpdatePhase::Downloading {
                        downloaded: current,
                        ..
                    } = &mut self.phase
                    {
                        *current = downloaded;
                    }
                    context.request_repaint();
                }
                UpdateEvent::Downloaded(result) => {
                    self.phase = match *result {
                        Ok((update, path)) => UpdatePhase::Downloaded { path, update },
                        Err(explanation) => UpdatePhase::Failed { explanation },
                    };
                    keep_receiver = false;
                }
            }
        }
        if keep_receiver {
            self.receiver = Some(receiver);
        }
    }

    pub(super) fn window(&mut self, context: &egui::Context, pending_drafts: &[String]) {
        self.start_background_check(context);
        self.poll(context);
        if self.receiver.is_some() {
            context.request_repaint_after(Duration::from_millis(100));
        }
        if !self.open {
            return;
        }
        let embedded_id = egui::Id::new("update-window");
        let viewport_id = egui::ViewportId::from_hash_of("update-viewport");
        if std::mem::take(&mut self.focus) {
            instplot_ui::ToolWindowPolicy::raise(context, viewport_id, embedded_id);
        }
        let spec = instplot_ui::ToolWindowSpec::new([520.0, 320.0], [420.0, 220.0]);
        if instplot_ui::ToolWindowPolicy::mode(context) == instplot_ui::ToolWindowMode::Embedded {
            let mut open = self.open;
            spec.embedded("检查更新", embedded_id)
                .open(&mut open)
                .frame(studio_card_frame(context.theme() == egui::Theme::Dark))
                .show(context, |ui| self.fields(ui, pending_drafts));
            self.open = open;
        } else {
            let builder = spec.viewport("检查更新");
            let close_requested = context.show_viewport_immediate(viewport_id, builder, |ui, _| {
                let child_context = ui.ctx().clone();
                let close_requested = instplot_ui::viewport_close_requested(&child_context);
                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::new()
                            .fill(studio_surface(child_context.theme() == egui::Theme::Dark))
                            .inner_margin(egui::Margin::same(16)),
                    )
                    .show(ui, |ui| self.fields(ui, pending_drafts));
                close_requested
            });
            if close_requested {
                self.open = false;
            }
        }
    }

    fn fields(&mut self, ui: &mut egui::Ui, pending_drafts: &[String]) {
        match self.phase.clone() {
            #[cfg(target_os = "macos")]
            UpdatePhase::LaunchingHelper => {
                ui.spinner();
                ui.label("更新助手准备中；确认就绪后将正常退出当前应用。请勿关闭电源。");
            }
            #[cfg(target_os = "macos")]
            UpdatePhase::PreparingMac => {
                ui.spinner();
                ui.label("正在验证原安装与候选应用；当前工作不会关闭。");
            }
            #[cfg(target_os = "macos")]
            UpdatePhase::ReadyMac(_) => {
                ui.heading("可以重启并更新");
                ui.label("将更新当前安装位置，保留设置和数据。未保存项目将在退出前确认。");
                for draft in pending_drafts {
                    ui.label(draft);
                }
                if ui
                    .add_enabled(pending_drafts.is_empty(), egui::Button::new("重启并更新"))
                    .clicked()
                {
                    self.restart_requested = true;
                }
            }
            UpdatePhase::Idle => {
                ui.label("后台检查新版；下载和重启需确认，不收集遥测。");
                if ui.button("检查更新").clicked() {
                    self.request_check(ui.ctx());
                }
            }
            UpdatePhase::Checking => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("正在从 OSS 检查签名更新……");
                });
            }
            UpdatePhase::UpToDate { version } => {
                ui.heading("已经是最新版");
                ui.label(format!("当前版本：{version}"));
                self.common_actions(ui);
            }
            UpdatePhase::Available(update) => {
                ui.heading(format!("发现新版本 {}", update.version));
                ui.label(format!("适用安装包：{}", update.package.file_name));
                if let Some(minimum) = &update.package.minimum_system {
                    ui.weak(format!("最低系统要求：{minimum}"));
                }
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    if ui.button("下载并验证").clicked() {
                        match update_cache_destination(&update.package.file_name) {
                            Ok(path) => self.start_download(ui.ctx(), update.clone(), path),
                            Err(explanation) => self.phase = UpdatePhase::Failed { explanation },
                        }
                    }
                    if ui.button("查看发布说明").clicked() {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(update.notes_url));
                    }
                    if ui.button("重新检查").clicked() {
                        self.request_check(ui.ctx());
                    }
                });
                ui.weak("安装包保存于应用私有缓存；下载不会关闭当前工作。");
            }
            UpdatePhase::Unsupported { explanation } => {
                ui.heading("当前平台暂无更新包");
                ui.label(explanation);
                self.common_actions(ui);
            }
            UpdatePhase::Failed { explanation } => {
                ui.heading("无法确认更新状态");
                ui.label(explanation);
                self.common_actions(ui);
            }
            UpdatePhase::Downloading {
                update,
                destination,
                downloaded,
            } => {
                ui.heading(format!("正在下载 {}", update.version));
                let total = update.package.size_bytes.max(1);
                let fraction = (downloaded as f64 / total as f64).clamp(0.0, 1.0) as f32;
                ui.add(
                    egui::ProgressBar::new(fraction)
                        .show_percentage()
                        .text(format!(
                            "{} / {}",
                            byte_count(downloaded),
                            byte_count(total)
                        )),
                );
                ui.weak(destination.display().to_string());
                if ui.button("取消下载").clicked() {
                    self.cancel.store(true, Ordering::Relaxed);
                }
            }
            UpdatePhase::Downloaded { path, update } => {
                ui.heading("下载和校验已完成");
                ui.label(path.display().to_string());
                ui.label(format!("已验证版本 {}", update.version));
                for draft in pending_drafts {
                    ui.label(draft);
                }
                #[cfg(target_os = "macos")]
                if cfg!(feature = "in-place-update-preview") && self.instance_guard.is_ok() {
                    if ui.button("准备原位升级").clicked() {
                        self.phase = UpdatePhase::PreparingMac;
                        let (sender, receiver) = mpsc::channel();
                        self.receiver = Some(receiver);
                        let context = ui.ctx().clone();
                        let path = path.clone();
                        std::thread::spawn(move || {
                            let result = crate::update_macos::prepare(&path, &update.version);
                            let _ = sender.send(UpdateEvent::PreparedMac(Box::new(result)));
                            context.request_repaint();
                        });
                    }
                } else {
                    ui.weak("原位升级仍在平台验证中，当前保留已校验安装包，不会自动退出或覆盖。");
                }
                #[cfg(not(target_os = "macos"))]
                ui.weak("该安装方式的安全原位升级尚在验证；可显示安装包手动升级。");
                ui.horizontal_wrapped(|ui| {
                    if ui.button("在文件夹中显示").clicked() && reveal_in_folder(&path).is_err()
                    {
                        self.phase = UpdatePhase::Failed {
                            explanation: "无法打开文件所在位置；下载文件仍然安全保留。".to_owned(),
                        };
                    }
                    if ui.button("重新检查").clicked() {
                        self.request_check(ui.ctx());
                    }
                });
            }
        }
    }

    fn common_actions(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if ui.button("重新检查").clicked() {
                self.request_check(ui.ctx());
            }
            if ui.button("打开 GitHub Releases").clicked() {
                ui.ctx()
                    .open_url(egui::OpenUrl::new_tab(GITHUB_RELEASES_ROOT));
            }
        });
    }

    fn start_download(
        &mut self,
        context: &egui::Context,
        update: AvailableUpdate,
        destination: PathBuf,
    ) {
        if destination.exists() {
            self.phase = UpdatePhase::Failed {
                explanation: "目标文件已经存在；请选择新的文件名，软件不会静默覆盖。".to_owned(),
            };
            return;
        }
        self.cancel.store(false, Ordering::Relaxed);
        self.phase = UpdatePhase::Downloading {
            update: update.clone(),
            destination: destination.clone(),
            downloaded: 0,
        };
        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        let cancel = Arc::clone(&self.cancel);
        let context = context.clone();
        std::thread::spawn(move || {
            let progress_sender = sender.clone();
            let result = download_package(&update.package, &destination, &cancel, |downloaded| {
                let _ = progress_sender.send(UpdateEvent::Progress(downloaded));
                context.request_repaint();
            });
            let result = result.and_then(|path| {
                let parent = path
                    .parent()
                    .ok_or_else(|| "更新缓存位置无效。".to_owned())?;
                write_cache_evidence(parent, &update)?;
                Ok((update, path))
            });
            let _ = sender.send(UpdateEvent::Downloaded(Box::new(result)));
            context.request_repaint();
        });
    }
}

fn check_for_update() -> Result<CheckOutcome, String> {
    let current =
        Version::parse(env!("CARGO_PKG_VERSION")).map_err(|_| "当前应用版本无效。".to_owned())?;
    let channel = UpdateChannel::for_version(&current);
    let root = AllowedUpdateRoot::parse(PRODUCTION_PUBLIC_ROOT)
        .map_err(|_| "内置更新地址无效。".to_owned())?;
    let manifest_url = production_latest_url(channel);
    let raw = fetch_bounded(&manifest_url, &root, MAX_MANIFEST_BYTES, BodyKind::Json)?;
    let signature_url =
        signature_url_from_manifest_bytes(&raw).map_err(|_| "更新清单格式无效。".to_owned())?;
    root.permits(&signature_url)
        .map_err(|_| "更新签名地址不受信任。".to_owned())?;
    let signature = fetch_bounded(
        &signature_url,
        &root,
        MAX_SIGNATURE_BYTES,
        BodyKind::Signature,
    )?;
    let manifest =
        verify_signed_manifest(&raw, &signature, &PRODUCTION_TRUSTED_KEYS, &root, channel)
            .map_err(|_| "更新清单未通过签名或安全校验。".to_owned())?;
    enforce_sequence(channel, manifest.release_sequence)?;
    let remote = Version::parse(&manifest.version).map_err(|_| "更新版本号无效。".to_owned())?;
    if remote < current {
        return Err("服务器返回了旧版本，已拒绝降级。".to_owned());
    }
    if remote == current {
        return Ok(CheckOutcome::UpToDate {
            version: current.to_string(),
        });
    }
    let Some(platform) = current_platform_key() else {
        return Ok(CheckOutcome::Unsupported {
            explanation: "此操作系统或处理器架构不在当前发布范围内。".to_owned(),
        });
    };
    let Some(platform_assets) = manifest.platforms.get(platform) else {
        return Ok(CheckOutcome::Unsupported {
            explanation: format!("版本 {} 没有 {platform} 安装包。", manifest.version),
        });
    };
    let package = platform_assets
        .packages
        .iter()
        .find(|package| package.id == platform_assets.preferred)
        .cloned()
        .ok_or_else(|| "更新清单没有有效的首选安装包。".to_owned())?;
    if package.size_bytes > MAX_PACKAGE_BYTES {
        return Err("更新包超过允许的最大大小。".to_owned());
    }
    Ok(CheckOutcome::Available(Box::new(AvailableUpdate {
        version: manifest.version,
        notes_url: manifest.notes_url,
        package,
        signed_manifest: raw,
        manifest_signature: signature,
    })))
}

fn private_update_cache() -> Result<PathBuf, String> {
    #[cfg(windows)]
    {
        instplot_studio::update_windows::private_download_root()
            .map_err(|_| "无法创建或验证私有更新缓存权限。".to_owned())
    }
    #[cfg(not(windows))]
    {
        let project = ProjectDirs::from("com", "InstPlot", "InstPlot Studio")
            .ok_or_else(|| "无法确定用户更新缓存目录。".to_owned())?;
        let root = project.cache_dir().join("updates");
        fs::create_dir_all(&root).map_err(|_| "无法创建更新缓存目录。".to_owned())?;
        if fs::symlink_metadata(&root)
            .map_err(|_| "无法读取更新缓存。".to_owned())?
            .file_type()
            .is_symlink()
        {
            return Err("更新缓存不能是符号链接。".to_owned());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
                .map_err(|_| "无法保护更新缓存权限。".to_owned())?;
        }
        Ok(root)
    }
}

fn update_cache_destination(file_name: &str) -> Result<PathBuf, String> {
    update_cache_destination_at(&private_update_cache()?, file_name)
}

fn update_cache_destination_at(root: &Path, file_name: &str) -> Result<PathBuf, String> {
    if file_name.is_empty()
        || file_name.contains(['/', '\\'])
        || file_name == "."
        || file_name == ".."
        || matches!(
            file_name,
            "manifest.json" | "manifest.json.sig" | "transaction.json"
        )
        || file_name
            .chars()
            .any(|c| c.is_control() || "<>:\"|?*".contains(c))
    {
        return Err("更新安装包文件名无效。".to_owned());
    }
    let mut random = [0_u8; 16];
    getrandom::fill(&mut random).map_err(|_| "无法生成更新缓存标识。".to_owned())?;
    let id = random
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let directory = root.join(id);
    #[cfg(not(windows))]
    fs::create_dir(&directory).map_err(|_| "无法创建私有更新目录。".to_owned())?;
    #[cfg(windows)]
    instplot_studio::update_windows::create_private_directory(&directory)
        .map_err(|_| "无法创建私有更新目录。".to_owned())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
            .map_err(|_| "无法保护更新缓存权限。".to_owned())?;
    }
    Ok(directory.join(file_name))
}

fn write_cache_evidence(directory: &Path, update: &AvailableUpdate) -> Result<(), String> {
    for (name, bytes) in [
        ("manifest.json", update.signed_manifest.as_slice()),
        ("manifest.json.sig", update.manifest_signature.as_slice()),
    ] {
        #[cfg(not(windows))]
        let mut options = OpenOptions::new();
        #[cfg(not(windows))]
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        #[cfg(not(windows))]
        let mut file = options
            .open(directory.join(name))
            .map_err(|_| "无法保留签名更新证据。".to_owned())?;
        #[cfg(windows)]
        let mut file = instplot_studio::update_windows::create_private_file(&directory.join(name))
            .map_err(|_| "无法保留签名更新证据。".to_owned())?;
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|_| "无法保存签名更新证据。".to_owned())?;
    }
    Ok(())
}

fn reserve_background_check() -> Result<bool, String> {
    let root = private_update_cache()?;
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let path = root.join("background-check.lock");
    if fs::symlink_metadata(&path).is_ok_and(|m| !m.is_file() || m.file_type().is_symlink()) {
        return Err("后台检查状态路径无效。".to_owned());
    }
    let file = options
        .open(path)
        .map_err(|_| "无法锁定后台检查。".to_owned())?;
    file.try_lock()
        .map_err(|_| "其他实例正在检查更新。".to_owned())?;
    let state = root.join("background-check.json");
    if fs::symlink_metadata(&state).is_ok_and(|m| !m.is_file() || m.file_type().is_symlink()) {
        return Err("后台检查状态路径无效。".to_owned());
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "系统时间无效。".to_owned())?
        .as_secs();
    let previous: Option<u64> = if state.exists() {
        if fs::metadata(&state)
            .map_err(|_| "无法读取检查时间。".to_owned())?
            .len()
            > 64
        {
            return Err("后台检查记录损坏。".to_owned());
        }
        Some(
            serde_json::from_slice(&fs::read(&state).map_err(|_| "无法读取检查时间。".to_owned())?)
                .map_err(|_| "后台检查记录损坏。".to_owned())?,
        )
    } else {
        None
    };
    if !background_check_due(previous, now) {
        return Ok(false);
    }
    AtomicFile::new(state, AllowOverwrite)
        .write(|f| {
            f.write_all(now.to_string().as_bytes())?;
            f.sync_all()
        })
        .map_err(|_| "无法保存后台检查时间。".to_owned())?;
    Ok(true)
}

fn background_check_due(previous: Option<u64>, now: u64) -> bool {
    previous.is_none_or(|last| now >= last && now - last >= 24 * 60 * 60)
}

#[derive(Clone, Copy)]
enum BodyKind {
    Json,
    Signature,
}

fn fetch_bounded(
    initial_url: &str,
    root: &AllowedUpdateRoot,
    limit: usize,
    kind: BodyKind,
) -> Result<Vec<u8>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .max_redirects(0)
        .http_status_as_error(false)
        .timeout_global(Some(NETWORK_TIMEOUT))
        .build()
        .into();
    let mut url = Url::parse(initial_url).map_err(|_| "更新地址无效。".to_owned())?;
    for redirect_count in 0..=MAX_REDIRECTS {
        root.permits(url.as_str())
            .map_err(|_| "更新地址超出受信任范围。".to_owned())?;
        let mut response = agent
            .get(url.as_str())
            .call()
            .map_err(|_| "无法连接更新服务器。".to_owned())?;
        let status = response.status().as_u16();
        if (300..400).contains(&status) {
            if redirect_count == MAX_REDIRECTS {
                return Err("更新服务器重定向次数过多。".to_owned());
            }
            let location = response
                .headers()
                .get("location")
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| "更新服务器返回了无效重定向。".to_owned())?;
            url = url
                .join(location)
                .map_err(|_| "更新服务器返回了无效重定向。".to_owned())?;
            continue;
        }
        if status != 200 {
            return Err(format!("更新服务器返回 HTTP {status}。"));
        }
        validate_response_headers(&response, kind, limit)?;
        let bytes = response
            .body_mut()
            .with_config()
            .limit((limit + 1) as u64)
            .read_to_vec()
            .map_err(|_| "读取更新数据失败。".to_owned())?;
        if bytes.len() > limit {
            return Err("更新数据超过允许大小。".to_owned());
        }
        return Ok(bytes);
    }
    Err("更新服务器重定向次数过多。".to_owned())
}

fn validate_response_headers(
    response: &ureq::http::Response<ureq::Body>,
    kind: BodyKind,
    limit: usize,
) -> Result<(), String> {
    if let Some(encoding) = response
        .headers()
        .get("content-encoding")
        .and_then(|value| value.to_str().ok())
        && !encoding.eq_ignore_ascii_case("identity")
    {
        return Err("更新服务器返回了不支持的内容编码。".to_owned());
    }
    if let Some(length) = response
        .headers()
        .get("content-length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok())
        && length > limit
    {
        return Err("更新数据超过允许大小。".to_owned());
    }
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| "更新服务器未提供内容类型。".to_owned())?;
    let valid = match kind {
        BodyKind::Json => content_type.starts_with("application/json"),
        BodyKind::Signature => {
            content_type.starts_with("application/octet-stream")
                || content_type.starts_with("binary/octet-stream")
        }
    };
    if !valid {
        return Err("更新服务器返回了错误的内容类型。".to_owned());
    }
    Ok(())
}

fn current_platform_key() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Some("windows-x86_64"),
        ("macos", "aarch64") => Some("macos-aarch64"),
        ("macos", "x86_64") => Some("macos-x86_64"),
        ("linux", "x86_64") => Some("linux-x86_64"),
        _ => None,
    }
}

#[derive(Default, Deserialize, Serialize)]
struct SequenceState {
    channels: BTreeMap<String, u64>,
}

fn enforce_sequence(channel: UpdateChannel, sequence: u64) -> Result<(), String> {
    let Some(project) = ProjectDirs::from("com", "InstPlot", "InstPlot Studio") else {
        return Err("无法确定应用配置目录。".to_owned());
    };
    enforce_sequence_at(project.config_dir(), channel, sequence)
}

fn enforce_sequence_at(
    directory: &Path,
    channel: UpdateChannel,
    sequence: u64,
) -> Result<(), String> {
    fs::create_dir_all(directory).map_err(|_| "无法创建更新状态目录。".to_owned())?;
    let lock_path = directory.join("update-sequences.lock");
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)
        .map_err(|_| "无法打开更新状态锁。".to_owned())?;
    lock.lock().map_err(|_| "无法锁定更新状态。".to_owned())?;
    let path = directory.join("update-sequences.json");
    let mut state = match fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|_| "更新状态已损坏；为防止降级，已停止检查。".to_owned())?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => SequenceState::default(),
        Err(_) => return Err("无法读取更新状态。".to_owned()),
    };
    let key = channel.as_str().to_owned();
    if state
        .channels
        .get(&key)
        .is_some_and(|highest| sequence < *highest)
    {
        return Err("检测到更新清单序列回退，已拒绝使用。".to_owned());
    }
    if state.channels.get(&key).copied().unwrap_or_default() == sequence {
        return Ok(());
    }
    state.channels.insert(key, sequence);
    let bytes = serde_json::to_vec(&state).map_err(|_| "无法保存更新状态。".to_owned())?;
    AtomicFile::new(&path, AllowOverwrite)
        .write(|file| {
            file.write_all(&bytes)?;
            file.sync_all()
        })
        .map_err(|_| "无法保存更新状态。".to_owned())
}

fn download_package(
    package: &UpdatePackage,
    destination: &Path,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64),
) -> Result<PathBuf, String> {
    if destination.exists() || package.size_bytes > MAX_PACKAGE_BYTES {
        return Err("目标文件已存在或更新包大小无效。".to_owned());
    }
    let parent = destination
        .parent()
        .filter(|path| path.exists())
        .ok_or_else(|| "目标文件夹不存在。".to_owned())?;
    let file_name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "目标文件名无效。".to_owned())?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = parent.join(format!(".{file_name}.part-{}-{nonce}", std::process::id()));
    let result = download_to_temporary(package, &temporary, cancel, &mut progress)
        .and_then(|()| install_no_clobber(&temporary, destination));
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map(|()| destination.to_path_buf())
}

fn download_to_temporary(
    package: &UpdatePackage,
    temporary: &Path,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(u64),
) -> Result<(), String> {
    let root = AllowedUpdateRoot::parse(PRODUCTION_PUBLIC_ROOT)
        .map_err(|_| "内置更新地址无效。".to_owned())?;
    root.permits(&package.url)
        .map_err(|_| "更新包地址不受信任。".to_owned())?;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .max_redirects(0)
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(120)))
        .build()
        .into();
    let mut url = Url::parse(&package.url).map_err(|_| "更新包地址无效。".to_owned())?;
    let mut final_response = None;
    for redirect_count in 0..=MAX_REDIRECTS {
        root.permits(url.as_str())
            .map_err(|_| "更新包重定向到了不受信任的地址。".to_owned())?;
        let response = agent
            .get(url.as_str())
            .call()
            .map_err(|_| "下载更新包失败。".to_owned())?;
        let status = response.status().as_u16();
        if (300..400).contains(&status) {
            if redirect_count == MAX_REDIRECTS {
                return Err("更新服务器重定向次数过多。".to_owned());
            }
            let location = response
                .headers()
                .get("location")
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| "更新服务器返回了无效重定向。".to_owned())?;
            url = url
                .join(location)
                .map_err(|_| "更新服务器返回了无效重定向。".to_owned())?;
            continue;
        }
        if status != 200 {
            return Err(format!("更新服务器返回 HTTP {status}。"));
        }
        final_response = Some(response);
        break;
    }
    let mut response = final_response.ok_or_else(|| "更新服务器重定向次数过多。".to_owned())?;
    if let Some(length) = response
        .headers()
        .get("content-length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        && length != package.size_bytes
    {
        return Err("更新包大小与清单不一致。".to_owned());
    }
    if let Some(encoding) = response
        .headers()
        .get("content-encoding")
        .and_then(|value| value.to_str().ok())
        && !encoding.eq_ignore_ascii_case("identity")
    {
        return Err("更新包使用了不支持的内容编码。".to_owned());
    }
    let mut reader = response.body_mut().as_reader();
    write_verified_stream(&mut reader, package, temporary, cancel, progress)
}

fn write_verified_stream(
    reader: &mut impl Read,
    package: &UpdatePackage,
    temporary: &Path,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(u64),
) -> Result<(), String> {
    #[cfg(not(windows))]
    let mut options = OpenOptions::new();
    #[cfg(not(windows))]
    options.write(true).create_new(true);
    #[cfg(not(windows))]
    let mut output = options
        .open(temporary)
        .map_err(|_| "无法创建临时下载文件。".to_owned())?;
    #[cfg(windows)]
    let mut output = instplot_studio::update_windows::create_private_file(temporary)
        .map_err(|_| "无法创建或验证私有临时下载文件。".to_owned())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        output
            .set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|_| "无法设置临时文件权限。".to_owned())?;
    }
    let mut hasher = Sha256::new();
    let mut downloaded = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err("下载已取消。".to_owned());
        }
        let count = reader
            .read(&mut buffer)
            .map_err(|_| "读取更新包失败。".to_owned())?;
        if count == 0 {
            break;
        }
        downloaded = downloaded
            .checked_add(count as u64)
            .ok_or_else(|| "更新包大小无效。".to_owned())?;
        if downloaded > package.size_bytes || downloaded > MAX_PACKAGE_BYTES {
            return Err("更新包大小与清单不一致。".to_owned());
        }
        output
            .write_all(&buffer[..count])
            .map_err(|_| "写入更新包失败。".to_owned())?;
        hasher.update(&buffer[..count]);
        progress(downloaded);
    }
    if downloaded != package.size_bytes {
        return Err("更新包大小与清单不一致。".to_owned());
    }
    let digest = format!("{:x}", hasher.finalize());
    if digest != package.sha256 {
        return Err("更新包 SHA-256 校验失败。".to_owned());
    }
    output
        .sync_all()
        .map_err(|_| "无法同步更新包。".to_owned())?;
    drop(output);
    #[cfg(unix)]
    fs::set_permissions(temporary, {
        use std::os::unix::fs::PermissionsExt;
        fs::Permissions::from_mode(0o644)
    })
    .map_err(|_| "无法设置更新包权限。".to_owned())?;
    Ok(())
}

fn install_no_clobber(temporary: &Path, destination: &Path) -> Result<(), String> {
    fs::hard_link(temporary, destination)
        .map_err(|_| "目标文件已经存在，或无法完成无覆盖保存。".to_owned())?;
    fs::remove_file(temporary).map_err(|_| "无法清理临时下载文件。".to_owned())
}

fn reveal_in_folder(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = std::process::Command::new("open");
        command.arg("-R").arg(path);
        command
    };
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = std::process::Command::new("explorer.exe");
        command.arg(format!("/select,{}", path.display()));
        command
    };
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    let mut command = {
        let mut command = std::process::Command::new("xdg-open");
        command.arg(path.parent().unwrap_or_else(|| Path::new(".")));
        command
    };
    command
        .spawn()
        .map(|_| ())
        .map_err(|_| "cannot reveal downloaded file".to_owned())
}

fn byte_count(value: u64) -> String {
    if value >= 1024 * 1024 {
        format!("{:.1} MB", value as f64 / (1024.0 * 1024.0))
    } else if value >= 1024 {
        format!("{:.1} KB", value as f64 / 1024.0)
    } else {
        format!("{value} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_events_stay_compact_on_every_platform() {
        // macOS-only variants must not mask oversized common payloads on other targets.
        assert!(std::mem::size_of::<UpdateEvent>() <= 64);
    }

    #[test]
    fn boxed_download_failure_keeps_the_original_error() {
        let mut state = AppUpdateState::default();
        let (sender, receiver) = mpsc::channel();
        state.receiver = Some(receiver);
        sender
            .send(UpdateEvent::Downloaded(Box::new(Err(
                "hash mismatch".into()
            ))))
            .unwrap();
        state.poll(&egui::Context::default());
        assert!(
            matches!(&state.phase, UpdatePhase::Failed { explanation } if explanation == "hash mismatch")
        );
        assert!(state.receiver.is_none());
    }

    #[test]
    fn background_schedule_is_throttled_and_clock_reversal_is_safe() {
        assert!(background_check_due(None, 10));
        assert!(!background_check_due(Some(100), 50));
        assert!(!background_check_due(Some(100), 100));
        assert!(!background_check_due(Some(100), 100 + 86_399));
        assert!(background_check_due(Some(100), 100 + 86_400));
    }

    #[test]
    fn skipped_check_is_not_reported_as_up_to_date() {
        let mut state = AppUpdateState::default();
        state.background_started = true;
        let (sender, receiver) = mpsc::channel();
        state.receiver = Some(receiver);
        state.phase = UpdatePhase::Checking;
        sender.send(UpdateEvent::BackgroundSkipped).unwrap();
        state.poll(&egui::Context::default());
        assert!(matches!(state.phase, UpdatePhase::Idle));
        assert!(!state.open);
    }

    #[test]
    fn background_failure_is_quiet_but_manual_failure_is_visible() {
        for background in [true, false] {
            let mut state = AppUpdateState::default();
            state.background_check = background;
            let (sender, receiver) = mpsc::channel();
            state.receiver = Some(receiver);
            sender
                .send(UpdateEvent::Checked(Err("offline".into())))
                .unwrap();
            state.poll(&egui::Context::default());
            assert_eq!(
                matches!(state.phase, UpdatePhase::Failed { .. }),
                !background
            );
        }
    }

    #[test]
    fn cache_allocations_are_unique_and_reject_unsafe_names() {
        let root =
            std::env::temp_dir().join(format!("instplot-update-cache-test-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        for name in ["", "..", "../escape", "a/b", "a\\b", "x:y", "manifest.json"] {
            assert!(update_cache_destination_at(&root, name).is_err());
        }
        let first = update_cache_destination_at(&root, "update.dmg").unwrap();
        let second = update_cache_destination_at(&root, "update.dmg").unwrap();
        assert_ne!(first, second);
        assert_eq!(first.file_name().unwrap(), "update.dmg");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(first.parent().unwrap())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o077,
                0
            );
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn production_trust_root_and_keys_are_fixed() {
        assert_eq!(
            production_latest_url(UpdateChannel::Stable),
            "https://instplot-release.oss-cn-beijing.aliyuncs.com/instplot-studio/channels/stable/latest.json"
        );
        assert_eq!(PRODUCTION_TRUSTED_KEYS.len(), 2);
        assert_ne!(
            PRODUCTION_TRUSTED_KEYS[0].bytes,
            PRODUCTION_TRUSTED_KEYS[1].bytes
        );
    }

    #[test]
    fn byte_count_is_compact() {
        assert_eq!(byte_count(12), "12 B");
        assert_eq!(byte_count(2048), "2.0 KB");
    }

    #[test]
    fn verified_stream_checks_hash_size_and_never_clobbers_destination() {
        let root = std::env::temp_dir().join(format!(
            "instplot-update-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        #[cfg(not(windows))]
        fs::create_dir(&root).unwrap();
        #[cfg(windows)]
        instplot_studio::update_windows::create_private_directory(&root).unwrap();
        let payload = b"verified update payload";
        let package = UpdatePackage {
            file_name: "update.bin".to_owned(),
            id: "portable".to_owned(),
            minimum_system: None,
            package_type: "archive".to_owned(),
            sha256: format!("{:x}", Sha256::digest(payload)),
            size_bytes: payload.len() as u64,
            url: format!("{PRODUCTION_PUBLIC_ROOT}/releases/test/update.bin"),
        };
        let temporary = root.join(".download.part");
        write_verified_stream(
            &mut std::io::Cursor::new(payload),
            &package,
            &temporary,
            &AtomicBool::new(false),
            &mut |_| {},
        )
        .unwrap();
        let destination = root.join("update.bin");
        install_no_clobber(&temporary, &destination).unwrap();
        assert_eq!(fs::read(&destination).unwrap(), payload);
        #[cfg(windows)]
        instplot_studio::update_windows::validate_private_file(&destination).unwrap();

        let second = root.join(".second.part");
        fs::write(&second, b"different").unwrap();
        assert!(install_no_clobber(&second, &destination).is_err());
        assert_eq!(fs::read(&destination).unwrap(), payload);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn verified_stream_rejects_wrong_size_and_cancellation() {
        let root = std::env::temp_dir().join(format!(
            "instplot-update-failure-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        #[cfg(not(windows))]
        fs::create_dir(&root).unwrap();
        #[cfg(windows)]
        instplot_studio::update_windows::create_private_directory(&root).unwrap();
        let package = UpdatePackage {
            file_name: "update.bin".to_owned(),
            id: "portable".to_owned(),
            minimum_system: None,
            package_type: "archive".to_owned(),
            sha256: "00".repeat(32),
            size_bytes: 99,
            url: format!("{PRODUCTION_PUBLIC_ROOT}/releases/test/update.bin"),
        };
        assert!(
            write_verified_stream(
                &mut std::io::Cursor::new(b"short"),
                &package,
                &root.join("size.part"),
                &AtomicBool::new(false),
                &mut |_| {},
            )
            .is_err()
        );
        assert!(
            write_verified_stream(
                &mut std::io::Cursor::new(b"short"),
                &package,
                &root.join("cancel.part"),
                &AtomicBool::new(true),
                &mut |_| {},
            )
            .is_err()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn signed_cache_evidence_is_owner_only_and_never_overwrites() {
        let mut random = [0_u8; 16];
        getrandom::fill(&mut random).unwrap();
        let root = std::env::temp_dir().join(format!(
            "studio-evidence-acl-{:032x}",
            u128::from_le_bytes(random)
        ));
        instplot_studio::update_windows::create_private_directory(&root).unwrap();
        let update = AvailableUpdate {
            version: "0.1.2-rc.3".into(),
            notes_url: String::new(),
            package: UpdatePackage {
                id: "inno-setup".into(),
                package_type: "inno-setup".into(),
                file_name: "fixture.exe".into(),
                minimum_system: None,
                sha256: "00".repeat(32),
                size_bytes: 1,
                url: String::new(),
            },
            // Evidence-writer fixture only; not accepted as signed metadata.
            signed_manifest: b"fixture metadata".to_vec(),
            manifest_signature: vec![7; 64],
        };
        write_cache_evidence(&root, &update).unwrap();
        for name in ["manifest.json", "manifest.json.sig"] {
            instplot_studio::update_windows::validate_private_file(&root.join(name)).unwrap();
        }
        assert!(write_cache_evidence(&root, &update).is_err());
        assert_eq!(
            fs::read(root.join("manifest.json")).unwrap(),
            update.signed_manifest
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn sequence_state_is_monotonic_across_concurrent_writers() {
        let root = std::env::temp_dir().join(format!(
            "instplot-update-sequence-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let handles = (1..=16)
            .rev()
            .map(|sequence| {
                let root = root.clone();
                std::thread::spawn(move || {
                    let _ = enforce_sequence_at(&root, UpdateChannel::Prerelease, sequence);
                })
            })
            .collect::<Vec<_>>();
        for handle in handles {
            handle.join().unwrap();
        }
        let state: SequenceState =
            serde_json::from_slice(&fs::read(root.join("update-sequences.json")).unwrap()).unwrap();
        assert_eq!(state.channels.get("prerelease"), Some(&16));
        assert!(enforce_sequence_at(&root, UpdateChannel::Prerelease, 15).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn damaged_sequence_state_fails_closed() {
        let root = std::env::temp_dir().join(format!(
            "instplot-update-damaged-sequence-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir(&root).unwrap();
        fs::write(root.join("update-sequences.json"), b"not json").unwrap();
        assert!(enforce_sequence_at(&root, UpdateChannel::Stable, 1).is_err());
        assert_eq!(
            fs::read(root.join("update-sequences.json")).unwrap(),
            b"not json"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
