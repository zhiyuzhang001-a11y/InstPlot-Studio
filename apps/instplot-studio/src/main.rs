use std::error::Error;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Instant;

mod ui_text;
mod workspace;

use eframe::egui;
use instplot_studio::{
    AxisRanges, CheckSeverity, EditCommand, EditGroup, EditHistory, EguiPreviewAdapter,
    FigureDocument, HandoffCleanup, HandoffImport, OpenProjectSource, PRODUCT_NAME, PreviewAdapter,
    PublicationReport, ResolvedFigure, SeriesDescriptor, StudioSession, check_publication,
    import_handoff, product_info, resolve_document, save_figure_pdf, save_figure_png,
    save_fixed_figure_pdf, save_fixed_figure_png, write_handoff,
};
use ui_text::{Text, UiLanguage};
use workspace::WorkspaceState;

fn main() {
    if let Err(error) = run(std::env::args_os().skip(1)) {
        eprintln!("InstPlot Studio: {error}");
        std::process::exit(2);
    }
}

fn run(arguments: impl IntoIterator<Item = OsString>) -> Result<(), Box<dyn Error>> {
    match StartupCommand::parse(arguments)? {
        StartupCommand::ProductInfo => {
            println!("{}", product_info());
            Ok(())
        }
        StartupCommand::ExportFixedPdf(path) => {
            let size = save_fixed_figure_pdf(&path)?;
            println!("exported_pdf={} bytes={size}", path.display());
            Ok(())
        }
        StartupCommand::ExportFixedPng(path) => {
            let size = save_fixed_figure_png(&path, 300)?;
            println!("exported_png={} dpi=300 bytes={size}", path.display());
            Ok(())
        }
        StartupCommand::CreateProject(path) => {
            let document = FigureDocument::fixed();
            document.save(&path)?;
            println!(
                "created_project={} schema={}",
                path.display(),
                document.project().schema_version
            );
            Ok(())
        }
        StartupCommand::CheckProject(path) => {
            let (document, report) = FigureDocument::open(&path)?;
            document.layout_figure()?;
            println!(
                "checked_project={} schema={} source={:?} warnings={}",
                path.display(),
                document.project().schema_version,
                report.source,
                report.warnings.len()
            );
            for warning in report.warnings {
                println!("warning={warning}");
            }
            Ok(())
        }
        StartupCommand::PublicationCheck(path) => {
            let (document, _) = FigureDocument::open(&path)?;
            let resolved = resolve_document(&document)?;
            let report = check_publication(&document, &resolved, 300);
            println!("{}", serde_json::to_string_pretty(&report)?);
            if report.error_count() > 0 {
                Err(format!("publication check found {} error(s)", report.error_count()).into())
            } else {
                Ok(())
            }
        }
        StartupCommand::CreateHandoff { source, output } => {
            let mut session = StudioSession::default();
            let outcome = session.import_data_file(&source)?;
            let size = write_handoff(
                &output,
                session.datasets(),
                "InstPlot Lite compatible producer",
                env!("CARGO_PKG_VERSION"),
            )?;
            println!(
                "created_handoff={} datasets={} bytes={size}",
                output.display(),
                outcome.read
            );
            Ok(())
        }
        StartupCommand::ImportHandoff { source, project } => {
            let imported = import_handoff(&source, HandoffCleanup::Keep)?;
            imported.document.save(&project)?;
            println!(
                "imported_handoff={} project={} datasets={} source_removed=false",
                source.display(),
                project.display(),
                imported.datasets.len()
            );
            Ok(())
        }
        StartupCommand::OpenHandoff(path) => {
            let imported = import_handoff(&path, HandoffCleanup::DeleteAfterImport)?;
            launch_gui(Some(imported)).map_err(Into::into)
        }
        StartupCommand::Gui => launch_gui(None).map_err(Into::into),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum StartupCommand {
    Gui,
    ProductInfo,
    ExportFixedPdf(PathBuf),
    ExportFixedPng(PathBuf),
    CreateProject(PathBuf),
    CheckProject(PathBuf),
    PublicationCheck(PathBuf),
    CreateHandoff { source: PathBuf, output: PathBuf },
    ImportHandoff { source: PathBuf, project: PathBuf },
    OpenHandoff(PathBuf),
}

impl StartupCommand {
    fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Self, &'static str> {
        let mut arguments = arguments.into_iter();
        match (arguments.next(), arguments.next(), arguments.next()) {
            (None, None, None) => Ok(Self::Gui),
            (Some(flag), None, None) if flag == "--product-info" => Ok(Self::ProductInfo),
            (Some(flag), Some(path), None) if flag == "--export-fixed-pdf" => {
                Ok(Self::ExportFixedPdf(path.into()))
            }
            (Some(flag), Some(path), None) if flag == "--export-fixed-png" => {
                Ok(Self::ExportFixedPng(path.into()))
            }
            (Some(flag), Some(path), None) if flag == "--create-project" => {
                Ok(Self::CreateProject(path.into()))
            }
            (Some(flag), Some(path), None) if flag == "--check-project" => {
                Ok(Self::CheckProject(path.into()))
            }
            (Some(flag), Some(path), None) if flag == "--publication-check" => {
                Ok(Self::PublicationCheck(path.into()))
            }
            (Some(flag), Some(path), None) if flag == "--open-handoff" => {
                Ok(Self::OpenHandoff(path.into()))
            }
            (Some(flag), Some(source), Some(output)) if flag == "--create-handoff" => {
                Ok(Self::CreateHandoff {
                    source: source.into(),
                    output: output.into(),
                })
            }
            (Some(flag), Some(source), Some(project)) if flag == "--import-handoff" => {
                Ok(Self::ImportHandoff {
                    source: source.into(),
                    project: project.into(),
                })
            }
            _ => Err(
                "usage: instplot-studio [--product-info | --export-fixed-pdf PATH | --export-fixed-png PATH | --create-project PATH | --check-project PATH | --publication-check PATH | --create-handoff SOURCE PACKAGE | --import-handoff PACKAGE PROJECT | --open-handoff PACKAGE]",
            ),
        }
    }
}

fn launch_gui(startup: Option<HandoffImport>) -> eframe::Result {
    let started = Instant::now();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(PRODUCT_NAME)
            .with_inner_size([1080.0, 720.0])
            .with_min_inner_size([760.0, 520.0]),
        ..Default::default()
    };

    eframe::run_native(
        "instplot-studio",
        options,
        Box::new(move |creation| Ok(Box::new(StudioApp::new(creation, started, startup)))),
    )
}

struct StudioApp {
    session: StudioSession,
    document: FigureDocument,
    resolved: ResolvedFigure,
    publication_report: PublicationReport,
    preview: EguiPreviewAdapter,
    selected_series: Option<String>,
    workspace: WorkspaceState,
    edit_history: EditHistory,
    pending_action: Option<PendingAction>,
    allow_close: bool,
    canvas_zoom: f32,
    messages: Vec<AppMessage>,
    status: Option<(String, Instant)>,
    language: UiLanguage,
    first_frame: bool,
    started: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PendingAction {
    NewProject,
    OpenLiteHandoff,
    OpenProject,
    Exit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MessageLevel {
    Warning,
    Error,
}

#[derive(Clone, Debug)]
struct AppMessage {
    code: String,
    level: MessageLevel,
    text: String,
}

impl StudioApp {
    fn new(
        creation: &eframe::CreationContext<'_>,
        started: Instant,
        startup: Option<HandoffImport>,
    ) -> Self {
        creation.egui_ctx.options_mut(|options| {
            options.zoom_with_keyboard = true;
        });
        ui_shell_spike::install_publication_fonts(&creation.egui_ctx);
        let startup_unsaved = startup.is_some();
        let language = UiLanguage::default();
        let (document, datasets, status, workspace) = startup.map_or_else(
            || {
                let document = FigureDocument::fixed();
                let (session, _) = StudioSession::from_project(document.project());
                (
                    document,
                    session.datasets().to_vec(),
                    language.text(Text::Ready).to_owned(),
                    WorkspaceState::new(language.text(Text::Untitled)),
                )
            },
            |imported| {
                (
                    imported.document,
                    imported.datasets,
                    language.opened_lite(&imported.producer_name, &imported.producer_version),
                    WorkspaceState::from_lite(language.text(Text::Untitled)),
                )
            },
        );
        let resolved = resolved_preview(&document)
            .expect("the validated Figure Document resolves through formal layout");
        let publication_report = check_publication(&document, &resolved, 300);
        let mut session = StudioSession::default();
        session.replace_datasets(datasets);
        let edit_history = EditHistory::new(&document, !startup_unsaved);
        Self {
            session,
            document,
            resolved,
            publication_report,
            preview: EguiPreviewAdapter,
            selected_series: None,
            workspace,
            edit_history,
            pending_action: None,
            allow_close: false,
            canvas_zoom: 1.5,
            messages: Vec::new(),
            status: Some((status, Instant::now())),
            language,
            first_frame: true,
            started,
        }
    }

    fn open_data(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title(self.language.text(Text::OpenDataDialog))
            .add_filter(
                self.language.text(Text::SupportedData),
                &["txt", "csv", "dat", "tsv", "xlsx", "xls"],
            )
            .pick_file()
        else {
            return;
        };
        let mut candidate_session = self.session.clone();
        match candidate_session.import_data_file(&path) {
            Ok(outcome) => {
                let mut candidate_document = self.document.clone();
                if let Err(error) =
                    candidate_document.sync_external_datasets(candidate_session.datasets())
                {
                    self.push_error(
                        "data-sync",
                        self.language.operation_failed(
                            self.language.text(Text::DataSourceUpdateOperation),
                            &error,
                        ),
                    );
                    return;
                }
                let resolved = match resolved_preview(&candidate_document) {
                    Ok(resolved) => resolved,
                    Err(error) => {
                        self.push_error(
                            "layout",
                            self.language.operation_failed(
                                self.language.text(Text::ProjectLayoutOperation),
                                &error,
                            ),
                        );
                        return;
                    }
                };
                self.document = candidate_document;
                self.session = candidate_session;
                self.edit_history.rebase_after_external_change();
                self.publication_report = check_publication(&self.document, &resolved, 300);
                self.resolved = resolved;
                self.workspace.note_data_import(&path);
                self.set_success(self.language.imported(
                    outcome.read,
                    outcome.added,
                    outcome.replaced,
                ));
                self.clear_message("import");
                self.clear_message("data-sync");
            }
            Err(error) => self.push_error(
                "import",
                self.language
                    .operation_failed(self.language.text(Text::ImportDataOperation), &error),
            ),
        }
    }

    fn open_lite_handoff(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title(self.language.text(Text::OpenLiteDialog))
            .add_filter(self.language.text(Text::HandoffFile), &["instplot-handoff"])
            .pick_file()
        else {
            return;
        };
        match import_handoff(&path, HandoffCleanup::DeleteAfterImport) {
            Ok(imported) => match resolved_preview(&imported.document) {
                Ok(resolved) => {
                    self.publication_report = check_publication(&imported.document, &resolved, 300);
                    self.session.replace_datasets(imported.datasets);
                    self.document = imported.document;
                    self.edit_history.reset(&self.document, false);
                    self.resolved = resolved;
                    self.workspace = WorkspaceState::from_lite(self.language.text(Text::Untitled));
                    self.selected_series = None;
                    self.messages.clear();
                    self.set_success(
                        self.language
                            .opened_lite(&imported.producer_name, &imported.producer_version),
                    );
                }
                Err(error) => self.push_error(
                    "handoff-layout",
                    self.language
                        .operation_failed(self.language.text(Text::HandoffLayoutOperation), &error),
                ),
            },
            Err(error) => self.push_error(
                "handoff",
                self.language
                    .operation_failed(self.language.text(Text::OpenHandoffOperation), &error),
            ),
        }
    }

    fn open_project(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title(self.language.text(Text::OpenProjectDialog))
            .add_filter(self.language.text(Text::ProjectFile), &["instplot"])
            .pick_file()
        else {
            return;
        };
        match FigureDocument::open(&path) {
            Ok((document, report)) => match resolved_preview(&document) {
                Ok(resolved) => {
                    let opened_primary = report.source == OpenProjectSource::Primary;
                    let (session, session_warnings) =
                        StudioSession::from_project(document.project());
                    self.publication_report = check_publication(&document, &resolved, 300);
                    self.document = document;
                    self.session = session;
                    self.edit_history.reset(&self.document, opened_primary);
                    self.resolved = resolved;
                    self.selected_series = None;
                    self.workspace = WorkspaceState::from_project(&path, !opened_primary);
                    self.messages.clear();
                    for (index, warning) in report
                        .warnings
                        .into_iter()
                        .filter(|warning| !warning.starts_with("External data source"))
                        .chain(session_warnings)
                        .enumerate()
                    {
                        self.push_warning(format!("project-source-{index}"), warning);
                    }
                    let status = match report.source {
                        OpenProjectSource::Primary => self.language.opened_project(&path),
                        OpenProjectSource::Backup => self.language.recovered_project(&path),
                    };
                    self.set_success(status);
                }
                Err(error) => self.push_error(
                    "layout",
                    self.language
                        .operation_failed(self.language.text(Text::ProjectLayoutOperation), &error),
                ),
            },
            Err(error) => self.push_error(
                "open-project",
                self.language
                    .operation_failed(self.language.text(Text::OpenProjectOperation), &error),
            ),
        }
    }

    fn new_project(&mut self) {
        let document = FigureDocument::fixed();
        let resolved = resolved_preview(&document)
            .expect("the built-in new project must always resolve through formal layout");
        self.publication_report = check_publication(&document, &resolved, 300);
        self.document = document;
        self.resolved = resolved;
        (self.session, _) = StudioSession::from_project(self.document.project());
        self.selected_series = None;
        self.workspace = WorkspaceState::new(self.language.text(Text::Untitled));
        self.edit_history.reset(&self.document, true);
        self.messages.clear();
        self.set_success(self.language.text(Text::Ready).to_owned());
    }

    fn save_project(&mut self, save_as: bool) -> bool {
        let path = if !save_as {
            self.workspace.project_path().map(Path::to_path_buf)
        } else {
            None
        };
        let path = path.or_else(|| {
            rfd::FileDialog::new()
                .set_title(self.language.text(Text::SaveProjectDialog))
                .add_filter(self.language.text(Text::ProjectFile), &["instplot"])
                .set_file_name("figure.instplot")
                .save_file()
        });
        let Some(path) = path else {
            return false;
        };
        match self.document.save(&path) {
            Ok(()) => {
                self.workspace.note_saved(path.clone());
                self.edit_history.mark_saved(&self.document);
                self.set_success(self.language.saved_project(&path));
                self.clear_message("save-project");
                true
            }
            Err(error) => {
                self.push_error(
                    "save-project",
                    self.language
                        .operation_failed(self.language.text(Text::SaveProjectOperation), &error),
                );
                false
            }
        }
    }

    fn export_pdf(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title(self.language.text(Text::ExportFigureDialog))
            .add_filter("PDF", &["pdf"])
            .set_file_name("instplot-studio-figure.pdf")
            .save_file()
        else {
            return;
        };
        self.export_pdf_to(&path);
    }

    fn export_pdf_to(&mut self, path: &Path) {
        match save_figure_pdf(&self.document, path) {
            Ok(size) => {
                self.set_success(self.language.exported(size, path));
                self.clear_message("export");
            }
            Err(error) => self.push_error(
                "export",
                self.language
                    .operation_failed(self.language.text(Text::ExportOperation), &error),
            ),
        }
    }

    fn export_png(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title(self.language.text(Text::ExportFigureDialog))
            .add_filter("PNG", &["png"])
            .set_file_name("instplot-studio-figure.png")
            .save_file()
        else {
            return;
        };
        match save_figure_png(&self.document, &path, 300) {
            Ok(size) => {
                self.set_success(self.language.exported(size, &path));
                self.clear_message("export");
            }
            Err(error) => self.push_error(
                "export",
                self.language
                    .operation_failed(self.language.text(Text::ExportOperation), &error),
            ),
        }
    }

    fn apply_ranges(&mut self, ranges: AxisRanges, group: EditGroup) {
        match self.edit_history.execute(
            &mut self.document,
            EditCommand::SetAxisRanges(ranges),
            Some(group),
        ) {
            Ok(outcome) if outcome.changed && self.refresh_document("layout") => {
                self.set_success(outcome.description);
                self.clear_message("invalid-axes");
            }
            Ok(_) => {}
            Err(error) => self.push_error(
                "invalid-axes",
                self.language
                    .operation_failed(self.language.text(Text::AxesOperation), &error),
            ),
        }
    }

    fn refresh_document(&mut self, message_code: &'static str) -> bool {
        match resolved_preview(&self.document) {
            Ok(resolved) => {
                self.publication_report = check_publication(&self.document, &resolved, 300);
                self.resolved = resolved;
                self.clear_message(message_code);
                true
            }
            Err(error) => {
                self.push_error(
                    message_code,
                    self.language
                        .operation_failed(self.language.text(Text::LayoutOperation), &error),
                );
                false
            }
        }
    }

    fn undo(&mut self) {
        if let Some(description) = self.edit_history.undo(&mut self.document)
            && self.refresh_document("undo-layout")
        {
            self.set_success(self.language.undo(&description));
        }
    }

    fn redo(&mut self) {
        if let Some(description) = self.edit_history.redo(&mut self.document)
            && self.refresh_document("redo-layout")
        {
            self.set_success(self.language.redo(&description));
        }
    }

    fn request_replacement(&mut self, action: PendingAction) {
        if self.edit_history.is_dirty(&self.document) {
            self.pending_action = Some(action);
        } else {
            self.perform_action(action);
        }
    }

    fn perform_action(&mut self, action: PendingAction) {
        self.pending_action = None;
        match action {
            PendingAction::NewProject => self.new_project(),
            PendingAction::OpenLiteHandoff => self.open_lite_handoff(),
            PendingAction::OpenProject => self.open_project(),
            PendingAction::Exit => {
                // The close command is issued by the confirmation dialog, which has the context.
            }
        }
    }

    fn unsaved_dialog(&mut self, context: &egui::Context) {
        let Some(action) = self.pending_action else {
            return;
        };
        egui::Window::new(self.language.text(Text::UnsavedChanges))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(context, |ui| {
                ui.label(self.language.text(Text::UnsavedExplanation));
                ui.horizontal(|ui| {
                    if ui.button(self.language.text(Text::Save)).clicked()
                        && self.save_project(false)
                    {
                        if action == PendingAction::Exit {
                            self.allow_close = true;
                            context.send_viewport_cmd(egui::ViewportCommand::Close);
                            self.pending_action = None;
                        } else {
                            self.perform_action(action);
                        }
                    }
                    if ui.button(self.language.text(Text::Discard)).clicked() {
                        if action == PendingAction::Exit {
                            self.allow_close = true;
                            context.send_viewport_cmd(egui::ViewportCommand::Close);
                            self.pending_action = None;
                        } else {
                            self.perform_action(action);
                        }
                    }
                    if ui.button(self.language.text(Text::Cancel)).clicked() {
                        self.pending_action = None;
                    }
                });
            });
    }

    fn set_success(&mut self, text: String) {
        self.status = Some((text, Instant::now()));
    }

    fn push_warning(&mut self, code: impl Into<String>, text: String) {
        self.push_message(code, MessageLevel::Warning, text);
    }

    fn push_error(&mut self, code: impl Into<String>, text: String) {
        self.push_message(code, MessageLevel::Error, text);
    }

    fn push_message(&mut self, code: impl Into<String>, level: MessageLevel, text: String) {
        let code = code.into();
        if let Some(existing) = self
            .messages
            .iter_mut()
            .find(|message| message.code == code)
        {
            existing.level = level;
            existing.text = text.clone();
        } else {
            self.messages.push(AppMessage {
                code,
                level,
                text: text.clone(),
            });
        }
        self.status = Some((text, Instant::now()));
    }

    fn clear_message(&mut self, code: &str) {
        self.messages.retain(|message| message.code != code);
    }

    fn series_tree(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.language.text(Text::Series));
        ui.collapsing(self.language.text(Text::FixedFigure), |ui| {
            for series in self.document.series() {
                let selected = self.selected_series.as_deref() == Some(series.id.as_str());
                if ui.selectable_label(selected, &series.label).clicked() {
                    self.selected_series = Some(series.id);
                }
            }
        });
        ui.separator();
        ui.heading(self.language.text(Text::Data));
        if self.session.datasets().is_empty() {
            ui.weak(self.language.text(Text::NoData));
        } else {
            for dataset in self.session.datasets() {
                ui.label(dataset.display_name());
                ui.weak(
                    self.language
                        .rows_columns(dataset.row_count, dataset.columns.len()),
                );
            }
        }
    }

    fn inspector(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.language.text(Text::Inspector));
        let series = self.selected_series.as_deref().and_then(|id| {
            self.document
                .series()
                .into_iter()
                .find(|series| series.id == id)
        });
        describe_selection(ui, series.as_ref(), self.language);

        ui.separator();
        ui.label(self.language.text(Text::AxesRanges));
        let mut ranges = self.document.axis_ranges();
        let mut changed_group = None;
        let mut finish_coalescing = false;
        egui::Grid::new("axis_ranges")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label(self.language.text(Text::XMin));
                let response = ui.add(egui::DragValue::new(&mut ranges.x_min));
                if response.changed() {
                    changed_group = Some(EditGroup::AxisXMinimum);
                }
                finish_coalescing |= response.drag_stopped() || response.lost_focus();
                ui.end_row();
                ui.label(self.language.text(Text::XMax));
                let response = ui.add(egui::DragValue::new(&mut ranges.x_max));
                if response.changed() {
                    changed_group = Some(EditGroup::AxisXMaximum);
                }
                finish_coalescing |= response.drag_stopped() || response.lost_focus();
                ui.end_row();
                ui.label(self.language.text(Text::YMin));
                let response = ui.add(egui::DragValue::new(&mut ranges.y_min));
                if response.changed() {
                    changed_group = Some(EditGroup::AxisYMinimum);
                }
                finish_coalescing |= response.drag_stopped() || response.lost_focus();
                ui.end_row();
                ui.label(self.language.text(Text::YMax));
                let response = ui.add(egui::DragValue::new(&mut ranges.y_max));
                if response.changed() {
                    changed_group = Some(EditGroup::AxisYMaximum);
                }
                finish_coalescing |= response.drag_stopped() || response.lost_focus();
                ui.end_row();
            });
        if let Some(group) = changed_group {
            self.apply_ranges(ranges, group);
        }
        if finish_coalescing {
            self.edit_history.finish_coalescing();
        }

        ui.separator();
        ui.label(self.language.text(Text::ViewOnly));
        ui.add(
            egui::Slider::new(&mut self.canvas_zoom, 0.5..=3.0)
                .text(self.language.text(Text::CanvasZoom)),
        );
        ui.weak(self.language.text(Text::ZoomNotSaved));

        ui.separator();
        ui.heading(self.language.text(Text::PublicationCheck));
        ui.label(self.language.publication_summary(
            self.publication_report.error_count(),
            self.publication_report.warning_count(),
            self.publication_report.information_count(),
        ));
        ui.weak(format!(
            "{}: {} · {}: {} dpi",
            self.language.text(Text::Rules),
            self.publication_report.rules_version,
            self.language.text(Text::Raster),
            self.publication_report.raster_dpi
        ));
        for finding in &self.publication_report.findings {
            let text = if let Some(node) = &finding.node_id {
                format!("{} · {} — {}", finding.rule_id, node, finding.message)
            } else {
                format!("{} — {}", finding.rule_id, finding.message)
            };
            match finding.severity {
                CheckSeverity::Error => {
                    ui.colored_label(egui::Color32::LIGHT_RED, text);
                }
                CheckSeverity::Warning => {
                    ui.colored_label(egui::Color32::YELLOW, text);
                }
                CheckSeverity::Information => {
                    ui.weak(text);
                }
            }
        }
    }
}

impl eframe::App for StudioApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let context = ui.ctx().clone();
        let dirty = self.edit_history.is_dirty(&self.document);
        let dirty_mark = if dirty { " *" } else { "" };
        context.send_viewport_cmd(egui::ViewportCommand::Title(format!(
            "{PRODUCT_NAME} — {}{dirty_mark}",
            self.workspace.display_name()
        )));
        if self
            .status
            .as_ref()
            .is_some_and(|(_, created)| created.elapsed().as_secs() >= 8)
        {
            self.status = None;
        }
        if context.input(|input| input.viewport().close_requested()) && dirty && !self.allow_close {
            context.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending_action = Some(PendingAction::Exit);
        }
        if context.input_mut(|input| {
            input.consume_shortcut(&egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::O,
            ))
        }) {
            self.open_data();
        }
        if context.input_mut(|input| {
            input.consume_shortcut(&egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND,
                egui::Key::O,
            ))
        }) {
            self.request_replacement(PendingAction::OpenProject);
        }
        if context.input_mut(|input| {
            input.consume_shortcut(&egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND,
                egui::Key::N,
            ))
        }) {
            self.request_replacement(PendingAction::NewProject);
        }
        if context.input_mut(|input| {
            input.consume_shortcut(&egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::S,
            ))
        }) {
            self.save_project(true);
        } else if context.input_mut(|input| {
            input.consume_shortcut(&egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND,
                egui::Key::S,
            ))
        }) {
            self.save_project(false);
        }
        let redo_requested = context.input_mut(|input| {
            input.consume_shortcut(&egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::Z,
            ))
        }) || context.input_mut(|input| {
            input.consume_shortcut(&egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND,
                egui::Key::Y,
            ))
        });
        if redo_requested {
            self.redo();
        } else if context.input_mut(|input| {
            input.consume_shortcut(&egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND,
                egui::Key::Z,
            ))
        }) {
            self.undo();
        }

        egui::Panel::top("product_header").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button(self.language.text(Text::File), |ui| {
                    if ui.button(self.language.text(Text::NewProject)).clicked() {
                        ui.close();
                        self.request_replacement(PendingAction::NewProject);
                    }
                    ui.separator();
                    if ui.button(self.language.text(Text::OpenData)).clicked() {
                        ui.close();
                        self.open_data();
                    }
                    if ui.button(self.language.text(Text::OpenLite)).clicked() {
                        ui.close();
                        self.request_replacement(PendingAction::OpenLiteHandoff);
                    }
                    if ui.button(self.language.text(Text::OpenProject)).clicked() {
                        ui.close();
                        self.request_replacement(PendingAction::OpenProject);
                    }
                    ui.separator();
                    if ui.button(self.language.text(Text::Save)).clicked() {
                        ui.close();
                        self.save_project(false);
                    }
                    if ui.button(self.language.text(Text::SaveAs)).clicked() {
                        ui.close();
                        self.save_project(true);
                    }
                    ui.separator();
                    if ui.button(self.language.text(Text::Exit)).clicked() {
                        ui.close();
                        if dirty {
                            self.pending_action = Some(PendingAction::Exit);
                        } else {
                            self.allow_close = true;
                            context.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    }
                });
                ui.menu_button(self.language.text(Text::Edit), |ui| {
                    let undo_text = self.edit_history.undo_description().map_or_else(
                        || self.language.text(Text::Undo).to_owned(),
                        |value| format!("{} {value}", self.language.text(Text::Undo)),
                    );
                    if ui
                        .add_enabled(
                            self.edit_history.undo_description().is_some(),
                            egui::Button::new(undo_text),
                        )
                        .clicked()
                    {
                        ui.close();
                        self.undo();
                    }
                    let redo_text = self.edit_history.redo_description().map_or_else(
                        || self.language.text(Text::Redo).to_owned(),
                        |value| format!("{} {value}", self.language.text(Text::Redo)),
                    );
                    if ui
                        .add_enabled(
                            self.edit_history.redo_description().is_some(),
                            egui::Button::new(redo_text),
                        )
                        .clicked()
                    {
                        ui.close();
                        self.redo();
                    }
                });
                ui.menu_button(self.language.text(Text::View), |ui| {
                    ui.menu_button(self.language.text(Text::Language), |ui| {
                        if ui
                            .selectable_label(
                                self.language == UiLanguage::Chinese,
                                self.language.text(Text::Chinese),
                            )
                            .clicked()
                        {
                            self.language = UiLanguage::Chinese;
                            ui.close();
                        }
                        if ui
                            .selectable_label(
                                self.language == UiLanguage::English,
                                self.language.text(Text::English),
                            )
                            .clicked()
                        {
                            self.language = UiLanguage::English;
                            ui.close();
                        }
                    });
                });
                ui.menu_button(self.language.text(Text::Export), |ui| {
                    if ui.button(self.language.text(Text::ExportPdf)).clicked() {
                        ui.close();
                        self.export_pdf();
                    }
                    if ui.button(self.language.text(Text::ExportPng)).clicked() {
                        ui.close();
                        self.export_png();
                    }
                });
                ui.separator();
                ui.strong(format!("{}{dirty_mark}", self.workspace.display_name()));
                if let Some((status, _)) = &self.status {
                    ui.separator();
                    ui.label(status);
                }
            });
        });

        egui::Panel::left("series_tree")
            .default_size(230.0)
            .show(ui, |ui| self.series_tree(ui));

        egui::Panel::right("inspector")
            .default_size(260.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.inspector(ui));
            });

        egui::Panel::bottom("warning_panel")
            .default_size(90.0)
            .show(ui, |ui| {
                ui.heading(self.language.text(Text::WarningsAndErrors));
                if self.messages.is_empty() {
                    ui.weak(self.language.text(Text::NoWarnings));
                } else {
                    for message in &self.messages {
                        let (label, color) = match message.level {
                            MessageLevel::Warning => {
                                (self.language.text(Text::Warning), egui::Color32::YELLOW)
                            }
                            MessageLevel::Error => {
                                (self.language.text(Text::Error), egui::Color32::LIGHT_RED)
                            }
                        };
                        ui.colored_label(color, format!("{label}: {}", message.text));
                    }
                }
            });

        egui::CentralPanel::default().show(ui, |ui| {
            let available = ui.available_size();
            let figure_size = egui::vec2(self.resolved.display.width, self.resolved.display.height)
                * self.canvas_zoom;
            let origin = ui.min_rect().min
                + egui::vec2(
                    ((available.x - figure_size.x) * 0.5).max(16.0),
                    ((available.y - figure_size.y) * 0.5).max(16.0),
                );
            let figure_rect = egui::Rect::from_min_size(origin, figure_size);
            ui.painter()
                .rect_filled(figure_rect, 0.0, egui::Color32::WHITE);
            ui.painter().rect_stroke(
                figure_rect,
                0.0,
                egui::Stroke::new(1.0, egui::Color32::GRAY),
                egui::StrokeKind::Outside,
            );
            let metrics = self.preview.paint(
                ui.painter(),
                &self.resolved.display,
                origin,
                self.canvas_zoom,
                context.pixels_per_point(),
            );
            ui.painter().text(
                figure_rect.left_bottom() + egui::vec2(0.0, 18.0),
                egui::Align2::LEFT_TOP,
                format!(
                    "{}: {} × {} px",
                    self.language.text(Text::PreviewFramebuffer),
                    metrics.framebuffer_width,
                    metrics.framebuffer_height
                ),
                egui::FontId::monospace(12.0),
                ui.visuals().text_color(),
            );
        });

        if self.first_frame {
            self.first_frame = false;
            eprintln!(
                "FIRST_CANVAS product=instplot-studio elapsed_ms={} pixels_per_point={:.3}",
                self.started.elapsed().as_millis(),
                context.pixels_per_point()
            );
        }
        self.unsaved_dialog(&context);
    }
}

fn resolved_preview(
    document: &FigureDocument,
) -> Result<ResolvedFigure, instplot_studio::DocumentLayoutError> {
    resolve_document(document)
}

fn describe_selection(ui: &mut egui::Ui, series: Option<&SeriesDescriptor>, language: UiLanguage) {
    if let Some(series) = series {
        ui.label(&series.label);
        ui.weak(format!("{}: {}", language.text(Text::Kind), series.kind));
        ui.weak(format!(
            "{}: {}",
            language.text(Text::StableNode),
            series.id
        ));
    } else {
        ui.weak(language.text(Text::SelectSeries));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use export_backend_spike::ResolvedItem;
    use studio_render_spike::{Color, DisplayItem, NodeId};

    #[test]
    fn startup_commands_keep_headless_work_before_gui_creation() {
        assert_eq!(StartupCommand::parse([]).unwrap(), StartupCommand::Gui);
        assert_eq!(
            StartupCommand::parse([OsString::from("--product-info")]).unwrap(),
            StartupCommand::ProductInfo
        );
        assert_eq!(
            StartupCommand::parse([
                OsString::from("--export-fixed-pdf"),
                OsString::from("figure.pdf")
            ])
            .unwrap(),
            StartupCommand::ExportFixedPdf(PathBuf::from("figure.pdf"))
        );
        assert_eq!(
            StartupCommand::parse([
                OsString::from("--export-fixed-png"),
                OsString::from("figure.png")
            ])
            .unwrap(),
            StartupCommand::ExportFixedPng(PathBuf::from("figure.png"))
        );
        assert_eq!(
            StartupCommand::parse([
                OsString::from("--create-project"),
                OsString::from("figure.instplot")
            ])
            .unwrap(),
            StartupCommand::CreateProject(PathBuf::from("figure.instplot"))
        );
        assert_eq!(
            StartupCommand::parse([
                OsString::from("--check-project"),
                OsString::from("figure.instplot")
            ])
            .unwrap(),
            StartupCommand::CheckProject(PathBuf::from("figure.instplot"))
        );
        assert_eq!(
            StartupCommand::parse([
                OsString::from("--publication-check"),
                OsString::from("figure.instplot")
            ])
            .unwrap(),
            StartupCommand::PublicationCheck(PathBuf::from("figure.instplot"))
        );
        assert_eq!(
            StartupCommand::parse([
                OsString::from("--open-handoff"),
                OsString::from("transfer.instplot-handoff")
            ])
            .unwrap(),
            StartupCommand::OpenHandoff(PathBuf::from("transfer.instplot-handoff"))
        );
        assert_eq!(
            StartupCommand::parse([
                OsString::from("--create-handoff"),
                OsString::from("lite.txt"),
                OsString::from("transfer.instplot-handoff")
            ])
            .unwrap(),
            StartupCommand::CreateHandoff {
                source: PathBuf::from("lite.txt"),
                output: PathBuf::from("transfer.instplot-handoff")
            }
        );
        assert_eq!(
            StartupCommand::parse([
                OsString::from("--import-handoff"),
                OsString::from("transfer.instplot-handoff"),
                OsString::from("figure.instplot")
            ])
            .unwrap(),
            StartupCommand::ImportHandoff {
                source: PathBuf::from("transfer.instplot-handoff"),
                project: PathBuf::from("figure.instplot")
            }
        );
        assert!(StartupCommand::parse([OsString::from("--unknown")]).is_err());
    }

    #[test]
    fn preview_uses_formal_layout_and_resolved_rotated_text() {
        let resolved = resolved_preview(&FigureDocument::fixed()).unwrap();
        assert!(resolved.display.items.iter().any(|item| matches!(
            item,
            ResolvedItem::Graphics(DisplayItem::Path {
                stroke: Some(stroke),
                ..
            }) if stroke.color == Color(218, 221, 224, 255)
        )));
        assert!(resolved.display.items.iter().any(|item| matches!(
            item,
            ResolvedItem::Text(text)
                if text.source == NodeId(4) && text.rotation_degrees == -90.0
        )));
        assert!(resolved.display.items.iter().any(|item| matches!(
            item,
            ResolvedItem::Graphics(DisplayItem::Path { source, .. })
                if *source == NodeId(11)
        )));
        assert!(resolved.display.items.iter().any(|item| matches!(
            item,
            ResolvedItem::Graphics(DisplayItem::Path { source, .. })
                if *source == NodeId(13)
        )));
        assert!(
            !resolved
                .display
                .items
                .iter()
                .any(|item| matches!(item, ResolvedItem::Graphics(DisplayItem::GlyphRun(_))))
        );
    }
}
