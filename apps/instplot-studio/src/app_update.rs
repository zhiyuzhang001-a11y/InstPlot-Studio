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

#[cfg(all(windows, feature = "in-place-update-preview"))]
pub(crate) fn windows_health_frame(
    health: &mut instplot_studio::update_windows::WindowsHealthStartup,
    frame: &mut eframe::Frame,
    context: &egui::Context,
) -> std::io::Result<instplot_studio::update_windows::WindowsHealthFrame> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    if health.take_window_activation_request() {
        context.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        context.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        context.send_viewport_cmd(egui::ViewportCommand::Focus);
    }
    let window = frame
        .winit_window()
        .ok_or_else(|| std::io::Error::other("GUI root window missing"))?;
    let handle = window.window_handle().map_err(std::io::Error::other)?;
    match handle.as_raw() {
        RawWindowHandle::Win32(handle) => health.first_canvas_ready_for_window(handle.hwnd.get()),
        _ => Err(std::io::Error::other("GUI root window is not Win32")),
    }
}

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
    #[cfg(all(windows, feature = "in-place-update-preview"))]
    PreparingWindows {
        downloaded: u64,
        total: u64,
    },
    #[cfg(all(windows, feature = "in-place-update-preview"))]
    PreparedWindows(Arc<instplot_studio::update_windows::PreparedWindowsInstallers>),
    #[cfg(all(windows, feature = "in-place-update-preview"))]
    StartingWindowsHelper,
    #[cfg(all(windows, feature = "in-place-update-preview"))]
    WindowsHelper {
        cancelling: bool,
        explanation: Option<String>,
    },
}

#[cfg(all(windows, feature = "in-place-update-preview"))]
enum WindowsParentOwned {
    Active(Box<instplot_studio::update_windows::WindowsParentHelper>),
    FailedStart(Box<instplot_studio::update_windows::WindowsParentStartFailure>),
}

enum UpdateEvent {
    #[cfg(all(windows, feature = "in-place-update-preview"))]
    WindowsHelperStarted(Box<WindowsHelperStarted>),
    #[cfg(all(windows, feature = "in-place-update-preview"))]
    WindowsParentPolled(Box<WindowsParentPolled>),
    BackgroundSkipped,
    Checked(Result<CheckOutcome, String>),
    Progress(u64),
    Downloaded(Box<Result<(AvailableUpdate, PathBuf), String>>),
    #[cfg(target_os = "macos")]
    PreparedMac(Box<Result<crate::update_macos::PreparedMacUpdate, String>>),
    #[cfg(target_os = "macos")]
    HelperReady(Result<(), String>),
    #[cfg(all(windows, feature = "in-place-update-preview"))]
    WindowsRecoveryProgress {
        downloaded: u64,
        total: u64,
    },
    #[cfg(all(windows, feature = "in-place-update-preview"))]
    PreparedWindows(
        Box<Result<instplot_studio::update_windows::PreparedWindowsInstallers, String>>,
    ),
}

#[cfg(all(windows, feature = "in-place-update-preview"))]
struct WindowsHelperStarted {
    guard: instplot_studio::update_windows::WindowsInstallAccess,
    parent: Result<WindowsParentOwned, String>,
    explanation: Option<String>,
}

#[cfg(all(windows, feature = "in-place-update-preview"))]
struct WindowsParentPolled {
    guard: instplot_studio::update_windows::WindowsInstallAccess,
    parent: WindowsParentOwned,
    cancelling: bool,
    result: Result<bool, String>,
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
    update_and_restart: bool,
    close_requested: bool,
    #[cfg(target_os = "macos")]
    instance_guard: Result<instplot_studio::update_bundle::BundleAccess, String>,
    #[cfg(all(windows, feature = "in-place-update-preview"))]
    windows_instance_guard: Result<instplot_studio::update_windows::WindowsInstallAccess, String>,
    #[cfg(all(windows, feature = "in-place-update-preview"))]
    windows_parent: Option<WindowsParentOwned>,
    #[cfg(all(windows, feature = "in-place-update-preview"))]
    windows_parent_poll: std::time::Instant,
    #[cfg(all(windows, feature = "in-place-update-preview"))]
    windows_exit_requested: bool,
    #[cfg(all(windows, feature = "in-place-update-preview"))]
    windows_close_dispatched: bool,
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
            update_and_restart: false,
            close_requested: false,
            #[cfg(target_os = "macos")]
            instance_guard: Err("尚未初始化安装进程锁。".into()),
            #[cfg(all(windows, feature = "in-place-update-preview"))]
            windows_instance_guard: Err("尚未初始化安装进程锁。".into()),
            #[cfg(all(windows, feature = "in-place-update-preview"))]
            windows_parent: None,
            #[cfg(all(windows, feature = "in-place-update-preview"))]
            windows_parent_poll: std::time::Instant::now(),
            #[cfg(all(windows, feature = "in-place-update-preview"))]
            windows_exit_requested: false,
            #[cfg(all(windows, feature = "in-place-update-preview"))]
            windows_close_dispatched: false,
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
        // Lifetime ownership. Preview may prepare packages, but installation
        // and restart remain disabled pending full helper/health/recovery acceptance.
        self.windows_instance_guard = guard;
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
            #[cfg(all(windows, feature = "in-place-update-preview"))]
            UpdatePhase::PreparedWindows(_) => true,
            _ => false,
        }
    }

    pub(super) fn is_launching(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            matches!(self.phase, UpdatePhase::LaunchingHelper)
        }
        #[cfg(all(windows, feature = "in-place-update-preview"))]
        {
            self.windows_parent.is_some()
                || matches!(
                    self.phase,
                    UpdatePhase::StartingWindowsHelper | UpdatePhase::WindowsHelper { .. }
                )
        }
        #[cfg(not(any(target_os = "macos", all(windows, feature = "in-place-update-preview"))))]
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
        #[cfg(all(windows, feature = "in-place-update-preview"))]
        {
            matches!(
                self.phase,
                UpdatePhase::PreparingWindows { .. } | UpdatePhase::PreparedWindows(_)
            )
        }
        #[cfg(not(any(target_os = "macos", all(windows, feature = "in-place-update-preview"))))]
        {
            false
        }
    }
    pub(super) fn take_restart_request(&mut self) -> bool {
        std::mem::take(&mut self.restart_requested)
    }
    pub(super) fn take_close_request(&mut self) -> bool {
        #[cfg(all(windows, feature = "in-place-update-preview"))]
        if self.close_requested {
            let proof = (|| {
                let Some(WindowsParentOwned::Active(owner)) = self.windows_parent.as_mut() else {
                    return Err("缺少实际助手，不能请求退出。".to_owned());
                };
                owner
                    .confirm_recent_ready()
                    .map_err(|error| error.to_string())
            })();
            if let Err(error) = proof {
                self.close_requested = false;
                self.windows_exit_requested = false;
                self.explain_blocked(error);
                return false;
            }
            self.windows_close_dispatched = true;
        }
        std::mem::take(&mut self.close_requested)
    }

    pub(super) fn allows_helper_close(&self) -> bool {
        #[cfg(all(windows, feature = "in-place-update-preview"))]
        {
            self.windows_close_dispatched
                && self.windows_exit_requested
                && matches!(
                    self.phase,
                    UpdatePhase::WindowsHelper {
                        cancelling: false,
                        ..
                    }
                )
                && matches!(self.windows_parent, Some(WindowsParentOwned::Active(_)))
        }
        #[cfg(not(all(windows, feature = "in-place-update-preview")))]
        {
            true
        }
    }
    pub(super) fn explain_blocked(&mut self, explanation: String) {
        self.open = true;
        #[cfg(all(windows, feature = "in-place-update-preview"))]
        if self.is_launching() {
            self.close_requested = false;
            self.windows_exit_requested = false;
            self.windows_close_dispatched = false;
            self.phase = UpdatePhase::WindowsHelper {
                cancelling: true,
                explanation: Some(explanation),
            };
            return;
        }
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
        #[cfg(all(windows, feature = "in-place-update-preview"))]
        {
            self.launch_windows_helper(project);
        }
        #[cfg(not(any(target_os = "macos", all(windows, feature = "in-place-update-preview"))))]
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
        self.update_and_restart = false;
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
            let result = reserve_background_check().and_then(|reservation| {
                let Some(reservation) = reservation else {
                    return Ok(None);
                };
                // Keep the cross-instance lock until the network and signature
                // checks finish. Failed checks must not suppress the next launch.
                let outcome = match check_for_update() {
                    Ok(outcome) => outcome,
                    Err(error) => {
                        reservation.record_failure();
                        return Err(error);
                    }
                };
                reservation.record_success()?;
                Ok(Some(outcome))
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
        #[cfg(all(windows, feature = "in-place-update-preview"))]
        self.poll_windows_parent(context);
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
                        #[cfg(all(windows, feature = "in-place-update-preview"))]
                        if self.is_launching() {
                            // Unknown worker result must not unfreeze protected
                            // work or permit a second helper to start.
                            self.close_requested = false;
                            self.windows_exit_requested = false;
                            self.phase = UpdatePhase::WindowsHelper {
                                cancelling: true,
                                explanation: Some(
                                    "后台助手任务意外终止，尚未确认助手退出；保留安装与恢复证据，当前工作不会自动解锁。".into(),
                                ),
                            };
                            break;
                        }
                        self.phase = UpdatePhase::Failed {
                            explanation: "更新任务意外终止，请重试。".to_owned(),
                        };
                        keep_receiver = false;
                    }
                    break;
                }
            };
            match event {
                #[cfg(all(windows, feature = "in-place-update-preview"))]
                UpdateEvent::WindowsParentPolled(result) => {
                    let WindowsParentPolled {
                        guard,
                        parent,
                        cancelling,
                        result,
                    } = *result;
                    self.windows_instance_guard = Ok(guard);
                    self.windows_parent = Some(parent);
                    self.accept_windows_parent_result(result, cancelling);
                    keep_receiver = false;
                }
                #[cfg(all(windows, feature = "in-place-update-preview"))]
                UpdateEvent::WindowsHelperStarted(result) => {
                    let WindowsHelperStarted {
                        guard,
                        parent,
                        explanation,
                    } = *result;
                    self.windows_instance_guard = Ok(guard);
                    match parent {
                        Ok(owner) => {
                            let (already_cancelling, previous_explanation) = match &self.phase {
                                UpdatePhase::WindowsHelper {
                                    cancelling: true,
                                    explanation,
                                } => (true, explanation.clone()),
                                _ => (false, None),
                            };
                            let explanation = previous_explanation.or(explanation);
                            let cancelling = already_cancelling || explanation.is_some();
                            // A successful start result already contains the
                            // full readiness proof; do not hash the installation
                            // again just to request normal GUI close. The final
                            // cheap freshness/owned-child gate still runs.
                            let ready =
                                !cancelling && matches!(owner, WindowsParentOwned::Active(_));
                            self.windows_exit_requested = ready;
                            self.close_requested = ready;
                            self.windows_parent = Some(owner);
                            self.windows_parent_poll = std::time::Instant::now();
                            self.phase = UpdatePhase::WindowsHelper {
                                cancelling,
                                explanation,
                            };
                        }
                        Err(explanation) => self.phase = UpdatePhase::Failed { explanation },
                    }
                    keep_receiver = false;
                }
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
                    if !matches!(self.phase, UpdatePhase::LaunchingHelper) {
                        self.close_requested = false;
                        self.phase = UpdatePhase::Failed {
                            explanation: "更新助手回执与当前重启阶段不一致；原窗口保持打开。"
                                .into(),
                        };
                    } else {
                        match result {
                            Ok(()) => self.close_requested = true,
                            Err(explanation) => self.phase = UpdatePhase::Failed { explanation },
                        }
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
                    #[cfg(all(windows, feature = "in-place-update-preview"))]
                    if self.update_and_restart {
                        if let UpdatePhase::Downloaded { path, update } = self.phase.clone() {
                            self.start_windows_preparation(context, &path, &update.version);
                        } else {
                            self.update_and_restart = false;
                        }
                    }
                }
                #[cfg(all(windows, feature = "in-place-update-preview"))]
                UpdateEvent::WindowsRecoveryProgress { downloaded, total } => {
                    if let UpdatePhase::PreparingWindows {
                        downloaded: current,
                        total: expected,
                    } = &mut self.phase
                    {
                        *current = downloaded;
                        *expected = total;
                    }
                }
                #[cfg(all(windows, feature = "in-place-update-preview"))]
                UpdateEvent::PreparedWindows(result) => {
                    self.phase = match *result {
                        Ok(prepared) => UpdatePhase::PreparedWindows(Arc::new(prepared)),
                        Err(explanation) => UpdatePhase::Failed { explanation },
                    };
                    if std::mem::take(&mut self.update_and_restart)
                        && matches!(self.phase, UpdatePhase::PreparedWindows(_))
                    {
                        // Reuse the shell's existing save/draft-protection flow;
                        // one update click never skips that final work gate.
                        self.restart_requested = true;
                    }
                    keep_receiver = false;
                }
            }
            // A terminal result consumes this task's receiver. Never let a
            // queued duplicate/progress event overwrite its accepted outcome.
            if !keep_receiver {
                break;
            }
        }
        if keep_receiver {
            self.receiver = Some(receiver);
        }
    }

    pub(super) fn window(&mut self, context: &egui::Context, pending_drafts: &[String]) {
        self.start_background_check(context);
        self.poll(context);
        #[cfg(all(windows, feature = "in-place-update-preview"))]
        if self.is_launching() {
            self.open = true;
        }
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
            #[cfg(all(windows, feature = "in-place-update-preview"))]
            if self.is_launching() {
                self.open = true;
            }
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
                #[cfg(all(windows, feature = "in-place-update-preview"))]
                if self.is_launching() {
                    self.open = true;
                }
            }
        }
    }

    fn fields(&mut self, ui: &mut egui::Ui, pending_drafts: &[String]) {
        match self.phase.clone() {
            #[cfg(all(windows, feature = "in-place-update-preview"))]
            UpdatePhase::StartingWindowsHelper => {
                ui.spinner();
                ui.label("正在后台校验并准备更新助手；当前工作已保护，请稍候。");
                ui.weak(format!(
                    "本次交接已等待 {} 秒。",
                    self.windows_parent_poll.elapsed().as_secs()
                ));
            }
            #[cfg(all(windows, feature = "in-place-update-preview"))]
            UpdatePhase::WindowsHelper {
                cancelling,
                explanation,
            } => {
                ui.spinner();
                ui.label(if cancelling {
                    "正在取消更新并等待助手正常退出；当前工作保持锁定。"
                } else {
                    "正在确认唯一实例与更新助手；当前工作保持锁定。"
                });
                if let Some(explanation) = explanation {
                    ui.colored_label(egui::Color32::LIGHT_RED, explanation);
                }
                if !cancelling
                    && !self.windows_close_dispatched
                    && ui.button("取消更新，返回当前工作").clicked()
                {
                    self.close_requested = false;
                    self.windows_exit_requested = false;
                    self.windows_close_dispatched = false;
                    self.phase = UpdatePhase::WindowsHelper {
                        cancelling: true,
                        explanation: None,
                    };
                }
                ui.weak("只有持久取消、真实助手退出及安装锁恢复全部确认后才解除锁定；不会强制终止进程。");
            }
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
                    let one_click = cfg!(all(windows, feature = "in-place-update-preview"));
                    if ui
                        .button(if one_click {
                            "更新并重启"
                        } else {
                            "下载并验证"
                        })
                        .clicked()
                    {
                        self.update_and_restart = one_click;
                        match update_cache_destination(&update.package.file_name) {
                            Ok(path) => self.start_download(ui.ctx(), update.clone(), path),
                            Err(explanation) => {
                                self.update_and_restart = false;
                                self.phase = UpdatePhase::Failed { explanation };
                            }
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
                    self.update_and_restart = false;
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
                #[cfg(not(any(
                    target_os = "macos",
                    all(windows, feature = "in-place-update-preview")
                )))]
                ui.weak("该安装方式的安全原位升级尚在验证；可显示安装包手动升级。");
                #[cfg(all(windows, feature = "in-place-update-preview"))]
                if self.windows_instance_guard.is_ok() && ui.button("继续更新").clicked() {
                    self.update_and_restart = true;
                    self.start_windows_preparation(ui.ctx(), &path, &update.version);
                }
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
            #[cfg(all(windows, feature = "in-place-update-preview"))]
            UpdatePhase::PreparingWindows { downloaded, total } => {
                ui.heading("正在准备更新");
                ui.label("旧应用继续运行；准备失败或取消不会安装、退出或替换。");
                if total > 0 {
                    ui.add(
                        egui::ProgressBar::new(
                            (downloaded as f64 / total as f64).clamp(0.0, 1.0) as f32
                        )
                        .show_percentage()
                        .text(format!(
                            "{} / {}",
                            byte_count(downloaded),
                            byte_count(total)
                        )),
                    );
                } else {
                    ui.spinner();
                }
                if ui.button("取消准备").clicked() {
                    self.update_and_restart = false;
                    self.cancel.store(true, Ordering::Relaxed);
                }
            }
            #[cfg(all(windows, feature = "in-place-update-preview"))]
            UpdatePhase::PreparedWindows(prepared) => {
                ui.heading("可以重启并更新");
                ui.label(format!(
                    "{} → {}",
                    prepared.installation().version(),
                    prepared.installers().candidate.installer().version()
                ));
                ui.weak("仅用于受控测试，尚未通过 Windows 实机验收；默认发布构建不开放此入口。");
                ui.label("更新原安装位置，未保存项目将在退出前确认；失败时保留恢复证据。");
                for draft in pending_drafts {
                    ui.label(draft);
                }
                if let Err(error) = &self.windows_instance_guard {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                }
                let (restart, end) = windows_preview_restart_controls(
                    ui,
                    pending_drafts,
                    self.windows_instance_guard.is_ok(),
                    self.windows_parent.is_some(),
                );
                if restart.clicked() {
                    // Only request the existing saved/draft-protected flow;
                    // the button itself never spawns a helper or closes work.
                    self.restart_requested = true;
                } else if end.clicked() {
                    self.restart_requested = false;
                    self.phase = UpdatePhase::Idle;
                }
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

    #[cfg(all(windows, feature = "in-place-update-preview"))]
    fn launch_windows_helper(&mut self, project: Option<PathBuf>) {
        use instplot_studio::update_windows::{PreparedWindowsHelper, WindowsParentHelper};
        let UpdatePhase::PreparedWindows(prepared) = &self.phase else {
            return;
        };
        if self.windows_parent.is_some() {
            return;
        }
        if let Err(error) = &self.windows_instance_guard {
            self.explain_blocked(error.clone());
            return;
        }
        // Called only after the existing saved/draft-protected replacement
        // flow. The preview control does not enable public release capability.
        let prepared = Arc::clone(prepared);
        let Ok(mut guard) = std::mem::replace(
            &mut self.windows_instance_guard,
            Err("安装锁由后台更新准备任务持有。".into()),
        ) else {
            return;
        };
        self.windows_exit_requested = false;
        self.windows_close_dispatched = false;
        self.close_requested = false;
        self.phase = UpdatePhase::StartingWindowsHelper;
        self.windows_parent_poll = std::time::Instant::now();
        self.open = true;
        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        std::thread::spawn(move || {
            let preparation_started = std::time::Instant::now();
            let helper = PreparedWindowsHelper::create(&prepared).and_then(|mut helper| {
                let creation_ms = preparation_started.elapsed().as_millis();
                let resume_started = std::time::Instant::now();
                helper.set_resume_project(project.as_deref())?;
                helper.record_parent_preparation(creation_ms, resume_started.elapsed().as_millis());
                Ok(helper)
            });
            let helper = match helper {
                Ok(helper) => helper,
                Err(error) => {
                    let _ = sender.send(UpdateEvent::WindowsHelperStarted(Box::new(
                        WindowsHelperStarted {
                            guard,
                            parent: Err(error.to_string()),
                            explanation: None,
                        },
                    )));
                    return;
                }
            };
            let (owned, explanation) = match WindowsParentHelper::start_after_work_protection(
                helper,
            ) {
                Ok(mut owner) => {
                    let promoted = owner.promote_installation_access(&mut guard);
                    let explanation = promoted.err().map(|error| error.to_string()).or_else(|| {
                        loop {
                            match owner.poll_readiness(&guard) {
                                Ok(instplot_studio::update_windows::WindowsParentReadiness::Ready) => break None,
                                Ok(instplot_studio::update_windows::WindowsParentReadiness::Waiting) => {
                                    std::thread::sleep(Duration::from_millis(100));
                                }
                                Err(error) => break Some(error.to_string()),
                            }
                        }
                    });
                    (WindowsParentOwned::Active(Box::new(owner)), explanation)
                }
                Err(failure) => {
                    let explanation = Some(failure.to_string());
                    (WindowsParentOwned::FailedStart(failure), explanation)
                }
            };
            let _ = sender.send(UpdateEvent::WindowsHelperStarted(Box::new(
                WindowsHelperStarted {
                    guard,
                    parent: Ok(owned),
                    explanation,
                },
            )));
        });
    }

    #[cfg(all(windows, feature = "in-place-update-preview"))]
    fn poll_windows_parent(&mut self, context: &egui::Context) {
        use instplot_studio::update_windows::WindowsParentReadiness;
        if !self.is_launching()
            || self.windows_exit_requested
            || matches!(self.phase, UpdatePhase::StartingWindowsHelper)
            || self.receiver.is_some()
        {
            return;
        }
        context.request_repaint_after(Duration::from_millis(250));
        if self.windows_parent_poll.elapsed() < Duration::from_millis(250) {
            return;
        }
        self.windows_parent_poll = std::time::Instant::now();
        let UpdatePhase::WindowsHelper {
            cancelling,
            explanation,
        } = self.phase.clone()
        else {
            // A lost phase must not drop an owned process or unlock work.
            self.phase = UpdatePhase::WindowsHelper {
                cancelling: true,
                explanation: Some("更新阶段异常，保持锁定并尝试安全取消。".into()),
            };
            return;
        };
        let Some(mut parent) = self.windows_parent.take() else {
            self.accept_windows_parent_result(Err("缺失实际助手对象，保持工作锁定。".into()), true);
            return;
        };
        let Ok(mut guard) = std::mem::replace(
            &mut self.windows_instance_guard,
            Err("后台助手核验持有安装锁。".into()),
        ) else {
            self.windows_parent = Some(parent);
            return;
        };
        let _ = explanation;
        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        std::thread::spawn(move || {
            let result = (|| {
                if cancelling {
                    match &mut parent {
                        WindowsParentOwned::Active(owner) => owner
                            .cancel_and_poll_exit(&mut guard)
                            .map_err(|error| error.to_string()),
                        WindowsParentOwned::FailedStart(failure) => {
                            failure
                                .cancel_before_any_child()
                                .map_err(|error| error.to_string())?;
                            guard
                                .require_idle_startup()
                                .map_err(|error| error.to_string())?;
                            Ok(true)
                        }
                    }
                } else {
                    match &mut parent {
                        WindowsParentOwned::Active(owner) => owner
                            .poll_readiness(&guard)
                            .map(|readiness| readiness == WindowsParentReadiness::Ready)
                            .map_err(|error| error.to_string()),
                        WindowsParentOwned::FailedStart(_) => {
                            Err("助手未启动，不能请求退出。".into())
                        }
                    }
                }
            })();
            let _ = sender.send(UpdateEvent::WindowsParentPolled(Box::new(
                WindowsParentPolled {
                    guard,
                    parent,
                    cancelling,
                    result,
                },
            )));
        });
    }

    #[cfg(all(windows, feature = "in-place-update-preview"))]
    fn accept_windows_parent_result(&mut self, result: Result<bool, String>, cancelling: bool) {
        if !cancelling
            && matches!(
                self.phase,
                UpdatePhase::WindowsHelper {
                    cancelling: true,
                    ..
                }
            )
        {
            // User cancellation wins over a readiness proof already in flight.
            return;
        }
        let explanation = match &self.phase {
            UpdatePhase::WindowsHelper { explanation, .. } => explanation.clone(),
            _ => None,
        };
        match result {
            Ok(true) if cancelling => {
                // Drop the start-failure project's lease BEFORE unfreezing.
                self.windows_parent = None;
                self.close_requested = false;
                self.phase = match explanation {
                    Some(explanation) => UpdatePhase::Failed {
                        explanation: format!("更新未开始，原工作已恢复。\n{explanation}"),
                    },
                    None => UpdatePhase::Idle,
                };
            }
            Ok(true) => {
                self.windows_exit_requested = true;
                self.close_requested = true;
            }
            Ok(false) => {}
            Err(error) => {
                self.close_requested = false;
                self.phase = UpdatePhase::WindowsHelper {
                    cancelling: true,
                    explanation: Some(error),
                };
            }
        }
    }

    #[cfg(all(windows, feature = "in-place-update-preview"))]
    fn start_windows_preparation(&mut self, context: &egui::Context, path: &Path, version: &str) {
        let Some(directory) = path.parent().map(Path::to_path_buf) else {
            self.explain_blocked("更新缓存位置无效。".into());
            return;
        };
        self.cancel.store(false, Ordering::Relaxed);
        self.phase = UpdatePhase::PreparingWindows {
            downloaded: 0,
            total: 0,
        };
        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        let cancel = Arc::clone(&self.cancel);
        let context = context.clone();
        let version = version.to_owned();
        std::thread::spawn(move || {
            let result = instplot_studio::update_windows::prepare_installers(
                &directory,
                &version,
                &cancel,
                |package, destination| {
                    download_package(package, destination, &cancel, |downloaded| {
                        let _ = sender.send(UpdateEvent::WindowsRecoveryProgress {
                            downloaded,
                            total: package.size_bytes,
                        });
                        context.request_repaint();
                    })
                    .map(|_| ())
                    .map_err(std::io::Error::other)
                },
            )
            .map_err(|error| format!("无法准备可信新旧安装包；旧应用继续运行：{error}"));
            let _ = sender.send(UpdateEvent::PreparedWindows(Box::new(result)));
            context.request_repaint();
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
    enforce_sequence(&root, channel, manifest.release_sequence)?;
    let remote = Version::parse(&manifest.version).map_err(|_| "更新版本号无效。".to_owned())?;
    if remote < current {
        return Err("服务器返回了旧版本，已拒绝降级。".to_owned());
    }
    if remote == current {
        #[cfg(all(windows, feature = "in-place-update-preview"))]
        instplot_studio::update_windows::remember_installed_manifest(&raw, &signature).map_err(
            |error| {
                format!(
                    "当前版本已是最新，但无法保留可信恢复信息；原位更新暂不可用。具体原因：{error}"
                )
            },
        )?;
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

struct BackgroundCheckReservation {
    _lock: fs::File,
    state: PathBuf,
    now: u64,
}

impl BackgroundCheckReservation {
    fn record_failure(&self) {
        // No server response, URLs, credentials or project data are retained.
        // This diagnostic is never read to authorize an update or throttle it.
        let path = self.state.with_file_name("background-check-result.json");
        if fs::symlink_metadata(&path).is_ok_and(|m| !m.is_file() || m.file_type().is_symlink()) {
            return;
        }
        let bytes = format!(
            "{{\"schema\":1,\"scope\":\"diagnostic-only\",\"started_at\":{},\"result\":\"check-failed\"}}",
            self.now
        );
        let _ = AtomicFile::new(path, AllowOverwrite).write(|f| {
            f.write_all(bytes.as_bytes())?;
            f.sync_all()
        });
    }

    fn record_success(&self) -> Result<(), String> {
        AtomicFile::new(&self.state, AllowOverwrite)
            .write(|f| {
                f.write_all(self.now.to_string().as_bytes())?;
                f.sync_all()
            })
            .map_err(|_| "无法保存后台检查时间。".to_owned())
    }
}

fn reserve_background_check() -> Result<Option<BackgroundCheckReservation>, String> {
    let root = private_update_cache()?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "系统时间无效。".to_owned())?
        .as_secs();
    reserve_background_check_at(&root, now)
}

fn reserve_background_check_at(
    root: &Path,
    now: u64,
) -> Result<Option<BackgroundCheckReservation>, String> {
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
        return Ok(None);
    }
    Ok(Some(BackgroundCheckReservation {
        _lock: file,
        state,
        now,
    }))
}

fn background_check_due(previous: Option<u64>, now: u64) -> bool {
    // Reopening Studio should discover a newly activated release that day.
    // Keep a short cross-instance throttle to avoid repeated requests/popups
    // when the user immediately relaunches several times.
    previous.is_none_or(|last| now >= last && now - last >= 5 * 60)
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

fn enforce_sequence(
    source: &AllowedUpdateRoot,
    channel: UpdateChannel,
    sequence: u64,
) -> Result<(), String> {
    let Some(project) = ProjectDirs::from("com", "InstPlot", "InstPlot Studio") else {
        return Err("无法确定应用配置目录。".to_owned());
    };
    enforce_sequence_at(project.config_dir(), source, channel, sequence)
}

fn enforce_sequence_at(
    directory: &Path,
    source: &AllowedUpdateRoot,
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
    let legacy_key = channel.as_str();
    let key = format!("{}|{legacy_key}", source.state_namespace());
    let mut highest = state.channels.get(&key).copied().unwrap_or_default();
    if source.owns_legacy_state() {
        highest = highest.max(state.channels.get(legacy_key).copied().unwrap_or_default());
    }
    if sequence < highest {
        return Err("检测到更新清单序列回退，已拒绝使用。".to_owned());
    }
    if state.channels.get(&key).copied() == Some(sequence)
        && (!source.owns_legacy_state()
            || state.channels.get(legacy_key).copied() == Some(sequence))
    {
        return Ok(());
    }
    state.channels.insert(key, sequence);
    // Keep old production clients protected too; never lower or delete the
    // legacy watermark and never import it into an isolated fixture feed.
    if source.owns_legacy_state() {
        state.channels.insert(legacy_key.to_owned(), sequence);
    }
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

// Also compiled for platform-independent headless control tests. Production
// Windows builds without the preview feature cannot display these controls.
#[cfg(any(all(windows, feature = "in-place-update-preview"), test))]
fn windows_preview_restart_controls(
    ui: &mut egui::Ui,
    pending_drafts: &[String],
    installation_ready: bool,
    helper_active: bool,
) -> (egui::Response, egui::Response) {
    ui.horizontal_wrapped(|ui| {
        let restart = ui.add_enabled(
            pending_drafts.is_empty() && installation_ready && !helper_active,
            egui::Button::new("重启并更新（测试）"),
        );
        let end = ui.add_enabled(!helper_active, egui::Button::new("结束准备，保留缓存"));
        (restart, end)
    })
    .inner
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

    #[cfg(all(windows, feature = "in-place-update-preview"))]
    #[test]
    fn background_helper_preparation_or_lost_worker_keeps_work_frozen() {
        let context = egui::Context::default();
        let mut state = AppUpdateState::default();
        state.phase = UpdatePhase::StartingWindowsHelper;
        let (sender, receiver) = mpsc::channel();
        state.receiver = Some(receiver);
        assert!(state.is_launching());
        state.request_check(&context);
        assert!(matches!(state.phase, UpdatePhase::StartingWindowsHelper));
        drop(sender);
        state.poll(&context);
        assert!(state.is_launching());
        assert!(state.receiver.is_some());
        assert!(!state.take_close_request());
    }

    #[cfg(all(windows, feature = "in-place-update-preview"))]
    #[test]
    fn stale_ready_result_cannot_override_user_cancellation() {
        let mut state = AppUpdateState::default();
        state.phase = UpdatePhase::WindowsHelper {
            cancelling: true,
            explanation: None,
        };
        // UI's decision must win over a successful background readiness result.
        // The event handler deliberately skips accepting readiness in this case.
        state.accept_windows_parent_result(Ok(true), false);
        assert!(matches!(
            state.phase,
            UpdatePhase::WindowsHelper {
                cancelling: true,
                ..
            }
        ));
        assert!(!state.close_requested);
        state.accept_windows_parent_result(Ok(false), true);
        assert!(state.is_launching());
        assert!(!state.take_close_request());
    }

    #[test]
    fn windows_preview_restart_controls_require_clean_drafts_guard_and_no_helper() {
        // Control behavior only; no native Windows installation is simulated.
        for has_drafts in [false, true] {
            for installation_ready in [false, true] {
                for helper_active in [false, true] {
                    let context = egui::Context::default();
                    let drafts = if has_drafts {
                        vec!["unapplied".into()]
                    } else {
                        vec![]
                    };
                    let mut output = context.run_ui(egui::RawInput::default(), |ui| {
                        let (restart, end) = windows_preview_restart_controls(
                            ui,
                            &drafts,
                            installation_ready,
                            helper_active,
                        );
                        assert_eq!(
                            restart.enabled(),
                            !has_drafts && installation_ready && !helper_active
                        );
                        assert_eq!(end.enabled(), !helper_active);
                        assert!(!restart.clicked());
                        assert!(!end.clicked());
                    });
                    output.textures_delta.clear();
                }
            }
        }
    }

    #[test]
    fn windows_preview_restart_clicks_cannot_bypass_disabled_controls() {
        for has_drafts in [false, true] {
            for installation_ready in [false, true] {
                for helper_active in [false, true] {
                    let context = egui::Context::default();
                    let drafts = if has_drafts {
                        vec!["unapplied".into()]
                    } else {
                        vec![]
                    };
                    let mut center = egui::Pos2::ZERO;
                    let mut output = context.run_ui(egui::RawInput::default(), |ui| {
                        let (restart, _) = windows_preview_restart_controls(
                            ui,
                            &drafts,
                            installation_ready,
                            helper_active,
                        );
                        center = restart.rect.center();
                    });
                    output.textures_delta.clear();
                    for pressed in [true, false] {
                        let input = egui::RawInput {
                            events: vec![
                                egui::Event::PointerMoved(center),
                                egui::Event::PointerButton {
                                    pos: center,
                                    button: egui::PointerButton::Primary,
                                    pressed,
                                    modifiers: egui::Modifiers::NONE,
                                },
                            ],
                            ..Default::default()
                        };
                        let mut output = context.run_ui(input, |ui| {
                            let (restart, _) = windows_preview_restart_controls(
                                ui,
                                &drafts,
                                installation_ready,
                                helper_active,
                            );
                            assert_eq!(
                                restart.clicked(),
                                !pressed && !has_drafts && installation_ready && !helper_active
                            );
                        });
                        output.textures_delta.clear();
                    }
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn frozen_shell_blocks_early_close_but_not_authorized_helper_exit() {
        let context = egui::Context::default();
        let creation = eframe::CreationContext::_new_kittest(context.clone());
        let mut app = crate::StudioApp::new(&creation, std::time::Instant::now(), None);
        app.update.phase = UpdatePhase::LaunchingHelper;
        app.update.open = false;
        let mut frame = eframe::Frame::_new_kittest();
        let close_input = || {
            let mut input = egui::RawInput::default();
            input
                .viewports
                .get_mut(&egui::ViewportId::ROOT)
                .unwrap()
                .events
                .push(egui::ViewportEvent::Close);
            input
        };
        let mut output = context.run_ui(close_input(), |ui| {
            eframe::App::ui(&mut app, ui, &mut frame)
        });
        assert!(
            output
                .viewport_output
                .get(&egui::ViewportId::ROOT)
                .unwrap()
                .commands
                .contains(&egui::ViewportCommand::CancelClose)
        );
        assert!(!app.allow_close);
        output.textures_delta.clear();
        // Simulate only the already-authorized close gate, not a real helper or
        // native health proof. The shell must not veto that normal exit.
        app.allow_close = true;
        let mut output = context.run_ui(close_input(), |ui| {
            eframe::App::ui(&mut app, ui, &mut frame)
        });
        assert!(
            !output
                .viewport_output
                .get(&egui::ViewportId::ROOT)
                .unwrap()
                .commands
                .contains(&egui::ViewportCommand::CancelClose)
        );
        output.textures_delta.clear();
    }

    #[cfg(all(windows, feature = "in-place-update-preview"))]
    #[test]
    fn windows_missing_owner_and_failure_never_unlock_or_authorize_close() {
        let mut state = AppUpdateState::default();
        state.phase = UpdatePhase::WindowsHelper {
            cancelling: false,
            explanation: None,
        };
        assert!(state.is_launching());
        state.close_requested = true;
        state.windows_exit_requested = true;
        assert!(!state.take_close_request());
        assert!(!state.windows_exit_requested);
        assert!(!state.allows_helper_close());
        assert!(state.is_launching());
        state.explain_blocked("inspection".into());
        assert!(matches!(
            state.phase,
            UpdatePhase::WindowsHelper {
                cancelling: true,
                ..
            }
        ));
        assert!(state.is_launching());
        let context = egui::Context::default();
        state.request_check(&context);
        assert!(matches!(state.phase, UpdatePhase::WindowsHelper { .. }));
        assert!(state.receiver.is_none());
        state.windows_parent_poll = std::time::Instant::now() - Duration::from_secs(1);
        state.poll_windows_parent(&context);
        assert!(state.is_launching());
        assert!(!state.take_close_request());
    }

    #[test]
    fn terminal_update_result_consumes_receiver_without_duplicate_replay() {
        let mut state = AppUpdateState::default();
        let (sender, receiver) = mpsc::channel();
        state.receiver = Some(receiver);
        for message in ["first verified failure", "queued duplicate"] {
            sender
                .send(UpdateEvent::Downloaded(Box::new(Err(message.into()))))
                .unwrap();
        }
        state.poll(&egui::Context::default());
        assert!(matches!(&state.phase, UpdatePhase::Failed { explanation }
            if explanation == "first verified failure"));
        assert!(state.receiver.is_none());
        assert!(!state.take_close_request());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn helper_wait_freezes_work_without_authorizing_exit_and_failure_unfreezes() {
        let mut state = AppUpdateState::default();
        state.phase = UpdatePhase::LaunchingHelper;
        let (sender, receiver) = mpsc::channel();
        state.receiver = Some(receiver);
        state.poll(&egui::Context::default());
        assert!(state.is_launching());
        assert!(state.receiver.is_some());
        assert!(!state.take_close_request());
        assert!(!state.take_restart_request());
        sender
            .send(UpdateEvent::HelperReady(Err(
                "helper identity mismatch".into()
            )))
            .unwrap();
        state.poll(&egui::Context::default());
        assert!(!state.is_launching());
        assert!(matches!(&state.phase, UpdatePhase::Failed { explanation }
            if explanation == "helper identity mismatch"));
        assert!(!state.take_close_request());
        assert!(state.receiver.is_none());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn disconnected_helper_never_authorizes_exit() {
        let mut state = AppUpdateState::default();
        state.phase = UpdatePhase::LaunchingHelper;
        let (sender, receiver) = mpsc::channel();
        state.receiver = Some(receiver);
        drop(sender);
        state.poll(&egui::Context::default());
        assert!(matches!(state.phase, UpdatePhase::Failed { .. }));
        assert!(!state.is_launching());
        assert!(!state.take_close_request());
        assert!(state.receiver.is_none());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn ready_helper_requests_close_once_and_keeps_work_frozen() {
        let mut state = AppUpdateState::default();
        state.phase = UpdatePhase::LaunchingHelper;
        let (sender, receiver) = mpsc::channel();
        state.receiver = Some(receiver);
        sender.send(UpdateEvent::HelperReady(Ok(()))).unwrap();
        sender
            .send(UpdateEvent::HelperReady(Err(
                "duplicate must not overwrite".into(),
            )))
            .unwrap();
        state.poll(&egui::Context::default());
        assert!(state.is_launching());
        assert!(state.take_close_request());
        assert!(!state.take_close_request());
        assert!(state.receiver.is_none());
        state.poll(&egui::Context::default());
        assert!(!state.take_close_request());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn helper_acknowledgement_outside_restart_phase_keeps_old_window_open() {
        let mut state = AppUpdateState::default();
        let (sender, receiver) = mpsc::channel();
        state.receiver = Some(receiver);
        sender.send(UpdateEvent::HelperReady(Ok(()))).unwrap();
        state.poll(&egui::Context::default());
        assert!(matches!(state.phase, UpdatePhase::Failed { .. }));
        assert!(!state.is_launching());
        assert!(!state.take_close_request());
        assert!(state.receiver.is_none());
    }

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

    #[cfg(all(windows, feature = "in-place-update-preview"))]
    #[test]
    fn recovery_preparation_progress_keeps_editing_and_blocks_another_check() {
        let mut state = AppUpdateState::default();
        state.phase = UpdatePhase::PreparingWindows {
            downloaded: 0,
            total: 0,
        };
        let (sender, receiver) = mpsc::channel();
        state.receiver = Some(receiver);
        sender
            .send(UpdateEvent::WindowsRecoveryProgress {
                downloaded: 25,
                total: 100,
            })
            .unwrap();
        let context = egui::Context::default();
        state.poll(&context);
        state.request_check(&context);
        assert!(matches!(
            state.phase,
            UpdatePhase::PreparingWindows {
                downloaded: 25,
                total: 100
            }
        ));
        assert!(state.is_preparing());
        assert!(
            !state.is_launching(),
            "editing must remain available during preparation"
        );
        assert!(!state.take_restart_request());
        assert!(!state.take_close_request());
        assert!(state.receiver.is_some());
    }

    #[cfg(all(windows, feature = "in-place-update-preview"))]
    #[test]
    fn recovery_preparation_failure_never_requests_exit_or_installation() {
        let mut state = AppUpdateState::default();
        state.phase = UpdatePhase::PreparingWindows {
            downloaded: 0,
            total: 0,
        };
        let (sender, receiver) = mpsc::channel();
        state.receiver = Some(receiver);
        sender
            .send(UpdateEvent::PreparedWindows(Box::new(Err(
                "missing trusted recovery".into(),
            ))))
            .unwrap();
        state.poll(&egui::Context::default());
        assert!(
            matches!(&state.phase, UpdatePhase::Failed { explanation } if explanation == "missing trusted recovery")
        );
        assert!(state.receiver.is_none());
        assert!(!state.is_launching());
        assert!(!state.take_restart_request());
        assert!(!state.take_close_request());
    }

    #[test]
    fn background_schedule_is_throttled_and_clock_reversal_is_safe() {
        assert!(background_check_due(None, 10));
        assert!(!background_check_due(Some(100), 50));
        assert!(!background_check_due(Some(100), 100));
        assert!(!background_check_due(Some(100), 100 + 299));
        assert!(background_check_due(Some(100), 100 + 300));
    }

    #[test]
    fn background_failure_does_not_throttle_and_reservation_holds_lock() {
        let root = std::env::temp_dir().join(format!(
            "instplot-background-check-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let reservation = reserve_background_check_at(&root, 100).unwrap().unwrap();
        assert!(reserve_background_check_at(&root, 100).is_err());
        assert!(!root.join("background-check.json").exists());
        reservation.record_failure();
        let diagnostic: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join("background-check-result.json")).unwrap())
                .unwrap();
        assert_eq!(diagnostic["result"], "check-failed");
        assert!(!root.join("background-check.json").exists());
        drop(reservation); // Simulate failed network/signature verification.
        let reservation = reserve_background_check_at(&root, 101).unwrap().unwrap();
        reservation.record_success().unwrap();
        drop(reservation);
        assert!(reserve_background_check_at(&root, 102).unwrap().is_none());
        assert!(reserve_background_check_at(&root, 401).unwrap().is_some());
        fs::write(root.join("background-check.json"), b"invalid").unwrap();
        assert!(reserve_background_check_at(&root, 500).is_err());
        fs::remove_dir_all(root).unwrap();
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
    fn background_available_update_opens_and_focuses_without_starting_installation() {
        let mut state = AppUpdateState::default();
        state.background_started = true;
        state.background_check = true;
        state.phase = UpdatePhase::Checking;
        let (sender, receiver) = mpsc::channel();
        state.receiver = Some(receiver);
        sender
            .send(UpdateEvent::Checked(Ok(CheckOutcome::Available(Box::new(
                AvailableUpdate {
                    version: "0.1.2-rc.3".into(),
                    notes_url: String::new(),
                    package: UpdatePackage {
                        id: "inno-setup".into(),
                        package_type: "exe-installer".into(),
                        file_name: "fixture.exe".into(),
                        minimum_system: None,
                        sha256: "00".repeat(32),
                        size_bytes: 1,
                        url: String::new(),
                    },
                    signed_manifest: Vec::new(),
                    manifest_signature: Vec::new(),
                },
            )))))
            .unwrap();
        state.poll(&egui::Context::default());
        assert!(state.open && state.focus);
        assert!(matches!(state.phase, UpdatePhase::Available(_)));
        assert!(!state.take_restart_request());
        assert!(!state.take_close_request());
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
                    let source = AllowedUpdateRoot::parse(PRODUCTION_PUBLIC_ROOT).unwrap();
                    let _ =
                        enforce_sequence_at(&root, &source, UpdateChannel::Prerelease, sequence);
                })
            })
            .collect::<Vec<_>>();
        for handle in handles {
            handle.join().unwrap();
        }
        let state: SequenceState =
            serde_json::from_slice(&fs::read(root.join("update-sequences.json")).unwrap()).unwrap();
        let source = AllowedUpdateRoot::parse(PRODUCTION_PUBLIC_ROOT).unwrap();
        let key = format!("{}|prerelease", source.state_namespace());
        assert_eq!(state.channels.get(&key), Some(&16));
        assert!(enforce_sequence_at(&root, &source, UpdateChannel::Prerelease, 15).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn sequence_sources_are_isolated_without_resetting_legacy_protection() {
        let directory = std::env::temp_dir().join(format!(
            "studio-sequence-sources-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&directory).unwrap();
        let path = directory.join("update-sequences.json");
        fs::write(&path, br#"{"channels":{"prerelease":20,"stable":9}}"#).unwrap();
        let production = AllowedUpdateRoot::parse(
            "https://instplot-release.oss-cn-beijing.aliyuncs.com/instplot-studio",
        )
        .unwrap();
        let qa = AllowedUpdateRoot::parse("https://instplot-release.oss-cn-beijing.aliyuncs.com/instplot-studio/windows-gui-qa/123-1").unwrap();
        let other_qa = AllowedUpdateRoot::parse("https://instplot-release.oss-cn-beijing.aliyuncs.com/instplot-studio/windows-gui-qa/456-1").unwrap();
        enforce_sequence_at(&directory, &qa, UpdateChannel::Prerelease, 1).unwrap();
        enforce_sequence_at(&directory, &qa, UpdateChannel::Prerelease, 2).unwrap();
        let before_rejection = fs::read(&path).unwrap();
        assert!(enforce_sequence_at(&directory, &qa, UpdateChannel::Prerelease, 1).is_err());
        assert!(
            enforce_sequence_at(&directory, &production, UpdateChannel::Prerelease, 19).is_err()
        );
        assert_eq!(fs::read(&path).unwrap(), before_rejection);
        enforce_sequence_at(&directory, &other_qa, UpdateChannel::Prerelease, 1).unwrap();
        enforce_sequence_at(&directory, &qa, UpdateChannel::Stable, 1).unwrap();
        enforce_sequence_at(&directory, &production, UpdateChannel::Prerelease, 20).unwrap();
        enforce_sequence_at(&directory, &production, UpdateChannel::Prerelease, 21).unwrap();
        let state: SequenceState = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(state.channels.get("prerelease"), Some(&21));
        assert_eq!(state.channels.get("stable"), Some(&9));
        let canonical_qa = AllowedUpdateRoot::parse("https://INSTPLOT-RELEASE.oss-cn-beijing.aliyuncs.com:443/instplot-studio/windows-gui-qa/123-1/").unwrap();
        assert!(
            enforce_sequence_at(&directory, &canonical_qa, UpdateChannel::Prerelease, 1).is_err()
        );
        enforce_sequence_at(&directory, &canonical_qa, UpdateChannel::Prerelease, 2).unwrap();
        fs::remove_dir_all(directory).unwrap();
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
        let source = AllowedUpdateRoot::parse(PRODUCTION_PUBLIC_ROOT).unwrap();
        assert!(enforce_sequence_at(&root, &source, UpdateChannel::Stable, 1).is_err());
        assert_eq!(
            fs::read(root.join("update-sequences.json")).unwrap(),
            b"not json"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
