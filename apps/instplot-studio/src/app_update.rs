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
}

#[derive(Clone, Debug)]
enum CheckOutcome {
    UpToDate { version: String },
    Available(AvailableUpdate),
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
    },
}

enum UpdateEvent {
    Checked(Result<CheckOutcome, String>),
    Progress(u64),
    Downloaded(Result<PathBuf, String>),
}

pub(super) struct AppUpdateState {
    pub(super) open: bool,
    pub(super) focus: bool,
    phase: UpdatePhase,
    receiver: Option<mpsc::Receiver<UpdateEvent>>,
    cancel: Arc<AtomicBool>,
}

impl Default for AppUpdateState {
    fn default() -> Self {
        Self {
            open: false,
            focus: false,
            phase: UpdatePhase::Idle,
            receiver: None,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Drop for AppUpdateState {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

impl AppUpdateState {
    pub(super) fn request_check(&mut self, context: &egui::Context) {
        self.open = true;
        self.focus = true;
        if matches!(
            self.phase,
            UpdatePhase::Checking | UpdatePhase::Downloading { .. }
        ) {
            return;
        }
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
                UpdateEvent::Checked(Ok(CheckOutcome::UpToDate { version })) => {
                    self.phase = UpdatePhase::UpToDate { version };
                    keep_receiver = false;
                }
                UpdateEvent::Checked(Ok(CheckOutcome::Available(update))) => {
                    self.phase = UpdatePhase::Available(update);
                    keep_receiver = false;
                }
                UpdateEvent::Checked(Ok(CheckOutcome::Unsupported { explanation })) => {
                    self.phase = UpdatePhase::Unsupported { explanation };
                    keep_receiver = false;
                }
                UpdateEvent::Checked(Err(explanation)) => {
                    self.phase = UpdatePhase::Failed { explanation };
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
                UpdateEvent::Downloaded(Ok(path)) => {
                    self.phase = UpdatePhase::Downloaded { path };
                    keep_receiver = false;
                }
                UpdateEvent::Downloaded(Err(explanation)) => {
                    self.phase = UpdatePhase::Failed { explanation };
                    keep_receiver = false;
                }
            }
        }
        if keep_receiver {
            self.receiver = Some(receiver);
        }
    }

    pub(super) fn window(&mut self, context: &egui::Context) {
        self.poll(context);
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
                .show(context, |ui| self.fields(ui));
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
                    .show(ui, |ui| self.fields(ui));
                close_requested
            });
            if close_requested {
                self.open = false;
            }
        }
    }

    fn fields(&mut self, ui: &mut egui::Ui) {
        match self.phase.clone() {
            UpdatePhase::Idle => {
                ui.label("仅在你主动操作时联网；不会自动安装或收集遥测。");
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
                    if ui.button("下载并验证…").clicked()
                        && let Some(path) = rfd::FileDialog::new()
                            .set_title("保存 InstPlot Studio 更新包")
                            .set_file_name(&update.package.file_name)
                            .save_file()
                    {
                        self.start_download(ui.ctx(), update.clone(), path);
                    }
                    if ui.button("查看发布说明").clicked() {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(update.notes_url));
                    }
                    if ui.button("重新检查").clicked() {
                        self.request_check(ui.ctx());
                    }
                });
                ui.weak("下载完成后仍需由你手动运行安装包。软件不会自动覆盖或重启。");
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
            UpdatePhase::Downloaded { path } => {
                ui.heading("下载和校验已完成");
                ui.label(path.display().to_string());
                ui.weak("请手动运行安装包完成升级。下载文件已经通过大小和 SHA-256 校验。");
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
            let _ = sender.send(UpdateEvent::Downloaded(result));
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
    Ok(CheckOutcome::Available(AvailableUpdate {
        version: manifest.version,
        notes_url: manifest.notes_url,
        package,
    }))
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
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    let mut output = options
        .open(temporary)
        .map_err(|_| "无法创建临时下载文件。".to_owned())?;
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
        fs::create_dir(&root).unwrap();
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
        fs::create_dir(&root).unwrap();
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
