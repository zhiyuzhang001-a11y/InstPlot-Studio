use super::*;
use instplot_studio::{
    DataBinding, lite_edit,
    lite_edit_protocol::{self, Transfer},
};
use std::{
    fs,
    process::{Child, Command, Stdio},
    time::Duration,
};

pub(super) struct PendingLiteEdit {
    root: PathBuf,
    request: Transfer,
    artist_id: String,
    binding_before: DataBinding,
    result: Option<Transfer>,
    error: Option<String>,
    keep_manual_axes: bool,
    last_poll: Instant,
    child: Option<Child>,
}

impl Drop for PendingLiteEdit {
    fn drop(&mut self) {
        // Never kill a Lite session: it may contain additional user work.
        if let Some(mut child) = self.child.take() {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
    }
}

fn lite_executable() -> Result<PathBuf, String> {
    let mut candidates = Vec::new();
    #[cfg(target_os = "macos")]
    {
        if let Ok(exe) = std::env::current_exe()
            && let Some(apps) = exe.ancestors().nth(4)
        {
            candidates.push(apps.join("InstPlot Lite.app/Contents/MacOS/instplot-lite"));
        }
        candidates.push(PathBuf::from(
            "/Applications/InstPlot Lite.app/Contents/MacOS/instplot-lite",
        ));
    }
    #[cfg(windows)]
    {
        for key in ["LOCALAPPDATA", "ProgramFiles"] {
            if let Some(root) = std::env::var_os(key) {
                let root = PathBuf::from(root);
                candidates.push(root.join("Programs/InstPlot Lite/instplot-lite.exe"));
                candidates.push(root.join("InstPlot Lite/instplot-lite.exe"));
            }
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        for root in std::env::split_paths(&path) {
            candidates.push(root.join(if cfg!(windows) {
                "instplot-lite.exe"
            } else {
                "instplot-lite"
            }));
        }
    }
    for path in candidates {
        let Ok(metadata) = fs::metadata(&path) else {
            continue;
        };
        if !metadata.is_file() || metadata.len() > 128 * 1024 * 1024 {
            continue;
        }
        // Old Lite ignores unknown CLI options. Detect compatibility without
        // starting an unsupported old version or a duplicate normal window.
        if fs::read(&path).is_ok_and(|bytes| {
            bytes
                .windows(lite_edit_protocol::CAPABILITY.len())
                .any(|v| v == lite_edit_protocol::CAPABILITY.as_bytes())
        }) {
            return Ok(path);
        }
    }
    Err("未找到支持 Studio 联动的 Lite，请先更新 Lite（不会启动不兼容的旧版本）".to_owned())
}

impl StudioApp {
    pub(crate) fn begin_lite_edit(&mut self, artist_id: &str) {
        let result = (|| {
            if self.lite_edit.is_some() {
                return Err("已有 Lite 编辑会话，请先完成或取消该会话".to_owned());
            }
            let executable = lite_executable()?;
            let mut random = [0u8; 32];
            getrandom::fill(&mut random).map_err(|e| e.to_string())?;
            let nonce = random
                .iter()
                .map(|v| format!("{v:02x}"))
                .collect::<String>();
            let (request, binding_before) =
                lite_edit::make_request(&self.document, artist_id, nonce.clone())?;
            let root = std::env::temp_dir()
                .canonicalize()
                .map_err(|e| e.to_string())?
                .join(format!("instplot-edit-{nonce}"));
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            builder.create(&root).map_err(|e| e.to_string())?;
            lite_edit_protocol::write_transfer(&root.join("request.json"), &request)?;
            let child = Command::new(executable)
                .arg("--studio-edit")
                .arg(root.join("request.json"))
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| e.to_string())?;
            Ok(PendingLiteEdit {
                root,
                request,
                artist_id: artist_id.to_owned(),
                binding_before,
                result: None,
                error: None,
                keep_manual_axes: true,
                last_poll: Instant::now(),
                child: Some(child),
            })
        })();
        match result {
            Ok(pending) => {
                self.lite_edit = Some(pending);
                self.set_success(
                    "已在 Lite 打开这一组数据；请保持 Studio 打开，处理后点击“发回 Studio”"
                        .to_owned(),
                );
            }
            Err(error) => self.push_error("lite-edit", error),
        }
    }

    pub(crate) fn lite_edit_window(&mut self, context: &egui::Context) {
        let Some(pending) = &mut self.lite_edit else {
            return;
        };
        if pending.result.is_none() && pending.error.is_none() {
            context.request_repaint_after(Duration::from_secs(1));
            if pending.last_poll.elapsed() >= Duration::from_secs(1) {
                pending.last_poll = Instant::now();
                let result_path = pending.root.join("result.json");
                if result_path.symlink_metadata().is_ok() {
                    match lite_edit_protocol::read_transfer(&result_path) {
                        Ok(result) if result.matches_session(&pending.request) => {
                            pending.result = Some(result);
                            context.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                            context.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                            context.send_viewport_cmd(egui::ViewportCommand::Focus);
                        }
                        Ok(_) => {
                            pending.error = Some("回传身份与本次编辑不符，未更新原图".to_owned())
                        }
                        Err(error) => {
                            pending.error = Some(format!("回传数据无效，未更新原图：{error}"))
                        }
                    }
                } else if let Some(child) = &mut pending.child
                    && child.try_wait().ok().flatten().is_some()
                    && result_path.symlink_metadata().is_err()
                {
                    pending.error = Some("Lite 已退出，尚未收到结果；原图未变化".to_owned());
                }
            }
        }
        let mut apply = false;
        let mut cancel = false;
        egui::Window::new("Lite 数据编辑")
            .id(egui::Id::new("lite-edit-return"))
            .resizable(false).collapsible(false).default_width(380.0)
            .show(context, |ui| {
                ui.label(&pending.request.label);
                if let Some(error) = &pending.error { ui.colored_label(egui::Color32::YELLOW, error); }
                else if let Some(result) = &pending.result {
                    ui.label(format!("已收到 {} 行数据（{} 行保留），只更新这一组。", result.alive.len(), result.alive.iter().filter(|v| **v).count()));
                    ui.label(format!("X：{}\nY：{}", result.x_column, result.y_column));
                    ui.checkbox(&mut pending.keep_manual_axes, "保留手动范围与刻度（自动轴仍刷新）");
                    ui.weak("颜色、线型、点样式、图例与排版不变；已有拟合不会自动重算。更新可一次撤销。");
                    apply = ui.button("更新这组数据").clicked();
                } else { ui.label("在 Lite 完成处理后，点击“发回 Studio”。请保持 Studio 打开。"); }
                cancel = ui.button("取消本次联动（不关闭 Lite）").clicked();
            });
        if cancel {
            self.lite_edit = None;
        } else if apply {
            let pending = self.lite_edit.as_ref().unwrap();
            let command = EditCommand::ApplyLiteData {
                update: Box::new(lite_edit::LiteSourceUpdate {
                    artist_id: pending.artist_id.clone(),
                    binding_before: pending.binding_before.clone(),
                    result: pending.result.clone().unwrap(),
                }),
                keep_manual_axes: pending.keep_manual_axes,
            };
            let state = AppTransactionState {
                document: &self.document,
                session: &self.session,
                workspace: &self.workspace,
                edit_history: &self.edit_history,
            };
            match ApplicationController::execute(
                state,
                AppAction::EditDocument {
                    command: Box::new(command),
                    group: None,
                },
            ) {
                Ok(outcome) => {
                    self.commit_app_outcome(outcome);
                    self.lite_edit = None;
                    self.sync_axis_editors();
                    self.set_success(
                        "Lite 数据已确认；Studio 排版已保留，数据变化可一次撤销".to_owned(),
                    );
                }
                Err(error) => {
                    let reason = error.diagnostics.join("；");
                    self.lite_edit.as_mut().unwrap().error = Some(reason.clone());
                    self.push_error("lite-edit-apply", reason);
                }
            }
        }
    }
}
