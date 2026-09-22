use std::error::Error;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Instant;

mod ui_text;
mod workspace;

use eframe::egui;
use instplot_core::{DataSet, DataSetKind};
use instplot_studio::{
    ArtistProperties, ArtistRole, AxisDimension, AxisRanges, AxisRecord, AxisScale, CheckSeverity,
    EditCommand, EditGroup, EditHistory, EguiPreviewAdapter, FigureDocument, FormatterSpec,
    HandoffCleanup, HandoffImport, LabelNode, LocatorSpec, MarkerShape, MoveDirection,
    OpenProjectSource, PRODUCT_NAME, PreviewAdapter, PublicationReport, ReferenceOrientation,
    ResolvedFigure, SeriesCreationStyle, SeriesDescriptor, SeriesKind, StrokeStyle, StudioSession,
    TickDirection, check_publication, import_handoff, product_info, resolve_document,
    save_figure_pdf, save_figure_png, save_fixed_figure_pdf, save_fixed_figure_png, write_handoff,
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
    selected_dataset: Option<String>,
    binding_x: String,
    binding_y: String,
    binding_error: String,
    creation_style: SeriesCreationStyle,
    pending_delete_source: Option<String>,
    x_label_draft: LabelDraft,
    y_label_draft: LabelDraft,
    x_fixed_ticks: String,
    y_fixed_ticks: String,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LabelPartKind {
    Text,
    Variable,
    Upright,
    GreekVariable,
    Number,
    DescriptiveSubscript,
    VariableSubscript,
    Superscript,
    Unit,
    UnitSeparator,
    Operator,
    Emphasis,
    BoldVariable,
}

#[derive(Clone, Debug)]
struct LabelPartDraft {
    kind: LabelPartKind,
    value: String,
}

#[derive(Clone, Debug, Default)]
struct LabelDraft {
    parts: Vec<LabelPartDraft>,
}

impl LabelDraft {
    fn from_nodes(nodes: &[LabelNode]) -> Self {
        Self {
            parts: nodes.iter().map(LabelPartDraft::from_node).collect(),
        }
    }

    fn to_nodes(&self) -> Result<Vec<LabelNode>, String> {
        if self.parts.is_empty() {
            return Err("axis label requires at least one semantic part".to_owned());
        }
        self.parts.iter().map(LabelPartDraft::to_node).collect()
    }
}

impl LabelPartDraft {
    fn from_node(node: &LabelNode) -> Self {
        let (kind, value) = match node {
            LabelNode::Text(value) => (LabelPartKind::Text, value.clone()),
            LabelNode::Variable(value) => (LabelPartKind::Variable, value.clone()),
            LabelNode::Upright(value) => (LabelPartKind::Upright, value.clone()),
            LabelNode::GreekVariable(value) => (LabelPartKind::GreekVariable, value.to_string()),
            LabelNode::Number(value) => (LabelPartKind::Number, value.clone()),
            LabelNode::DescriptiveSubscript(nodes) => {
                (LabelPartKind::DescriptiveSubscript, label_nodes_text(nodes))
            }
            LabelNode::VariableSubscript(nodes) => {
                (LabelPartKind::VariableSubscript, label_nodes_text(nodes))
            }
            LabelNode::Superscript(nodes) => (LabelPartKind::Superscript, label_nodes_text(nodes)),
            LabelNode::Unit(value) => (LabelPartKind::Unit, value.clone()),
            LabelNode::UnitSeparator => (LabelPartKind::UnitSeparator, String::new()),
            LabelNode::Operator(value) => (LabelPartKind::Operator, value.clone()),
            LabelNode::Emphasis(value) => (LabelPartKind::Emphasis, value.clone()),
            LabelNode::BoldVariable(value) => (LabelPartKind::BoldVariable, value.clone()),
        };
        Self { kind, value }
    }

    fn to_node(&self) -> Result<LabelNode, String> {
        if self.kind != LabelPartKind::UnitSeparator && self.value.is_empty() {
            return Err("semantic label parts cannot be empty".to_owned());
        }
        Ok(match self.kind {
            LabelPartKind::Text => LabelNode::Text(self.value.clone()),
            LabelPartKind::Variable => LabelNode::Variable(self.value.clone()),
            LabelPartKind::Upright => LabelNode::Upright(self.value.clone()),
            LabelPartKind::GreekVariable => {
                let mut chars = self.value.chars();
                let value = chars
                    .next()
                    .filter(|_| chars.next().is_none())
                    .ok_or_else(|| {
                        "Greek variable must contain exactly one character".to_owned()
                    })?;
                if !matches!(value as u32, 0x0370..=0x03ff | 0x1f00..=0x1fff) {
                    return Err("Greek variable must use a Greek Unicode character".to_owned());
                }
                LabelNode::GreekVariable(value)
            }
            LabelPartKind::Number => LabelNode::Number(self.value.clone()),
            LabelPartKind::DescriptiveSubscript => {
                LabelNode::DescriptiveSubscript(vec![LabelNode::Text(self.value.clone())])
            }
            LabelPartKind::VariableSubscript => {
                LabelNode::VariableSubscript(vec![LabelNode::Variable(self.value.clone())])
            }
            LabelPartKind::Superscript => {
                LabelNode::Superscript(vec![LabelNode::Number(self.value.clone())])
            }
            LabelPartKind::Unit => LabelNode::Unit(self.value.clone()),
            LabelPartKind::UnitSeparator => LabelNode::UnitSeparator,
            LabelPartKind::Operator => LabelNode::Operator(self.value.clone()),
            LabelPartKind::Emphasis => LabelNode::Emphasis(self.value.clone()),
            LabelPartKind::BoldVariable => LabelNode::BoldVariable(self.value.clone()),
        })
    }
}

fn label_nodes_text(nodes: &[LabelNode]) -> String {
    nodes
        .iter()
        .map(|node| match node {
            LabelNode::Text(value)
            | LabelNode::Variable(value)
            | LabelNode::Upright(value)
            | LabelNode::Number(value)
            | LabelNode::Unit(value)
            | LabelNode::Operator(value)
            | LabelNode::Emphasis(value)
            | LabelNode::BoldVariable(value) => value.clone(),
            LabelNode::GreekVariable(value) => value.to_string(),
            LabelNode::DescriptiveSubscript(nodes)
            | LabelNode::VariableSubscript(nodes)
            | LabelNode::Superscript(nodes) => label_nodes_text(nodes),
            LabelNode::UnitSeparator => " ".to_owned(),
        })
        .collect()
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
        let x_label_draft = LabelDraft::from_nodes(document.axis_label(AxisDimension::X));
        let y_label_draft = LabelDraft::from_nodes(document.axis_label(AxisDimension::Y));
        let x_fixed_ticks = fixed_ticks_text(&document.axis_record(AxisDimension::X));
        let y_fixed_ticks = fixed_ticks_text(&document.axis_record(AxisDimension::Y));
        Self {
            session,
            document,
            resolved,
            publication_report,
            preview: EguiPreviewAdapter,
            selected_series: None,
            selected_dataset: None,
            binding_x: String::new(),
            binding_y: String::new(),
            binding_error: String::new(),
            creation_style: SeriesCreationStyle::Scatter,
            pending_delete_source: None,
            x_label_draft,
            y_label_draft,
            x_fixed_ticks,
            y_fixed_ticks,
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
                if let Err(error) = candidate_document.sync_datasets(candidate_session.datasets()) {
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
                if let Some(data_source_id) = self
                    .session
                    .datasets()
                    .iter()
                    .rev()
                    .find(|dataset| dataset.source == path)
                    .map(|dataset| dataset.plot_id.clone())
                {
                    self.select_dataset(&data_source_id);
                }
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
                    self.selected_dataset = None;
                    self.sync_axis_editors();
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
                    self.selected_dataset = None;
                    self.sync_axis_editors();
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
        self.selected_dataset = None;
        self.sync_axis_editors();
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
            (self.session, _) = StudioSession::from_project(self.document.project());
            self.sync_axis_editors();
            self.set_success(self.language.undo(&description));
        }
    }

    fn redo(&mut self) {
        if let Some(description) = self.edit_history.redo(&mut self.document)
            && self.refresh_document("redo-layout")
        {
            (self.session, _) = StudioSession::from_project(self.document.project());
            self.sync_axis_editors();
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

    fn execute_document_edit(&mut self, command: EditCommand, success: &str) -> bool {
        self.execute_document_edit_with_group(command, success, None)
    }

    fn execute_document_edit_with_group(
        &mut self,
        command: EditCommand,
        success: &str,
        group: Option<EditGroup>,
    ) -> bool {
        match self
            .edit_history
            .execute(&mut self.document, command, group)
        {
            Ok(outcome) if outcome.changed && self.refresh_document("edit-layout") => {
                self.set_success(success.to_owned());
                self.clear_message("edit");
                true
            }
            Ok(_) => false,
            Err(error) => {
                self.push_error("edit", error);
                false
            }
        }
    }

    fn figure_size_editor(&mut self, ui: &mut egui::Ui) {
        ui.label(self.language.text(Text::FigureSize));
        let (mut width, mut height) = self.document.figure_size_mm();
        let mut changed_group = None;
        let mut finish = false;
        ui.horizontal(|ui| {
            let response = ui.add(
                egui::DragValue::new(&mut width)
                    .range(20.0..=500.0)
                    .speed(0.5)
                    .prefix(format!("{}: ", self.language.text(Text::WidthMm))),
            );
            if response.changed() {
                changed_group = Some(EditGroup::FigureWidth);
            }
            finish |= response.drag_stopped() || response.lost_focus();
            let response = ui.add(
                egui::DragValue::new(&mut height)
                    .range(20.0..=500.0)
                    .speed(0.5)
                    .prefix(format!("{}: ", self.language.text(Text::HeightMm))),
            );
            if response.changed() {
                changed_group = Some(EditGroup::FigureHeight);
            }
            finish |= response.drag_stopped() || response.lost_focus();
        });
        ui.horizontal(|ui| {
            if ui.button("85 × 65 mm").clicked() {
                width = 85.0;
                height = 65.0;
                changed_group = Some(EditGroup::FigureWidth);
                finish = true;
            }
            if ui.button("89 × 65 mm").clicked() {
                width = 89.0;
                height = 65.0;
                changed_group = Some(EditGroup::FigureWidth);
                finish = true;
            }
        });
        if let Some(group) = changed_group {
            self.execute_document_edit_with_group(
                EditCommand::SetFigureSize {
                    width_mm: width,
                    height_mm: height,
                },
                self.language.text(Text::ApplySize),
                Some(group),
            );
        }
        if finish {
            self.edit_history.finish_coalescing();
        }
    }

    fn axis_editor(&mut self, ui: &mut egui::Ui, dimension: AxisDimension) {
        let title = match dimension {
            AxisDimension::X => self.language.text(Text::XAxis),
            AxisDimension::Y => self.language.text(Text::YAxis),
        };
        let mut record = self.document.axis_record(dimension);
        let mut changed = false;
        let mut fixed_ticks = match dimension {
            AxisDimension::X => self.x_fixed_ticks.clone(),
            AxisDimension::Y => self.y_fixed_ticks.clone(),
        };
        let mut fixed_tick_error = None;
        ui.collapsing(title, |ui| {
            changed |= ui
                .checkbox(&mut record.autoscale, self.language.text(Text::Autoscale))
                .changed();
            ui.label(self.language.text(Text::Scale));
            egui::ComboBox::from_id_salt(("scale", dimension))
                .selected_text(match record.scale {
                    AxisScale::Linear => self.language.text(Text::Linear),
                    AxisScale::Log10 => self.language.text(Text::Log10),
                })
                .show_ui(ui, |ui| {
                    changed |= ui
                        .selectable_value(
                            &mut record.scale,
                            AxisScale::Linear,
                            self.language.text(Text::Linear),
                        )
                        .changed();
                    changed |= ui
                        .selectable_value(
                            &mut record.scale,
                            AxisScale::Log10,
                            self.language.text(Text::Log10),
                        )
                        .changed();
                });

            let locator_is_fixed = matches!(record.locator, LocatorSpec::Fixed { .. });
            ui.label(self.language.text(Text::Locator));
            egui::ComboBox::from_id_salt(("locator", dimension))
                .selected_text(if locator_is_fixed {
                    self.language.text(Text::Fixed)
                } else {
                    self.language.text(Text::Auto)
                })
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(!locator_is_fixed, self.language.text(Text::Auto))
                        .clicked()
                    {
                        record.locator = LocatorSpec::Auto { target_count: 6 };
                        changed = true;
                    }
                    if ui
                        .selectable_label(locator_is_fixed, self.language.text(Text::Fixed))
                        .clicked()
                    {
                        let values = parse_fixed_ticks(&fixed_ticks)
                            .unwrap_or_else(|_| vec![record.minimum, record.maximum]);
                        record.locator = LocatorSpec::Fixed { values };
                        changed = true;
                    }
                });
            match &mut record.locator {
                LocatorSpec::Auto { target_count } => {
                    changed |= ui
                        .add(
                            egui::DragValue::new(target_count)
                                .range(2..=20)
                                .prefix(format!("{}: ", self.language.text(Text::TargetTickCount))),
                        )
                        .changed();
                }
                LocatorSpec::Fixed { values } => {
                    ui.label(self.language.text(Text::FixedTickValues));
                    ui.text_edit_singleline(&mut fixed_ticks);
                    if ui.button(self.language.text(Text::Apply)).clicked() {
                        match parse_fixed_ticks(&fixed_ticks) {
                            Ok(parsed) => {
                                *values = parsed;
                                changed = true;
                            }
                            Err(error) => fixed_tick_error = Some(error),
                        }
                    }
                }
            }

            let formatter_kind = match record.formatter {
                FormatterSpec::Auto => 0,
                FormatterSpec::Decimal { .. } => 1,
                FormatterSpec::Scientific { .. } => 2,
            };
            ui.label(self.language.text(Text::Formatter));
            egui::ComboBox::from_id_salt(("formatter", dimension))
                .selected_text(match formatter_kind {
                    0 => self.language.text(Text::Auto),
                    1 => self.language.text(Text::Decimal),
                    _ => self.language.text(Text::Scientific),
                })
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(formatter_kind == 0, self.language.text(Text::Auto))
                        .clicked()
                    {
                        record.formatter = FormatterSpec::Auto;
                        changed = true;
                    }
                    if ui
                        .selectable_label(formatter_kind == 1, self.language.text(Text::Decimal))
                        .clicked()
                    {
                        record.formatter = FormatterSpec::Decimal { precision: 2 };
                        changed = true;
                    }
                    if ui
                        .selectable_label(formatter_kind == 2, self.language.text(Text::Scientific))
                        .clicked()
                    {
                        record.formatter = FormatterSpec::Scientific { precision: 2 };
                        changed = true;
                    }
                });
            if let FormatterSpec::Decimal { precision } | FormatterSpec::Scientific { precision } =
                &mut record.formatter
            {
                changed |= ui
                    .add(
                        egui::DragValue::new(precision)
                            .range(0..=15)
                            .prefix(format!("{}: ", self.language.text(Text::Precision))),
                    )
                    .changed();
            }

            ui.separator();
            changed |= ui
                .checkbox(
                    &mut record.appearance.near_spine,
                    self.language.text(Text::NearSpine),
                )
                .changed();
            changed |= ui
                .checkbox(
                    &mut record.appearance.far_spine,
                    self.language.text(Text::FarSpine),
                )
                .changed();
            changed |= ui
                .checkbox(
                    &mut record.appearance.near_ticks,
                    self.language.text(Text::NearTicks),
                )
                .changed();
            changed |= ui
                .checkbox(
                    &mut record.appearance.far_ticks,
                    self.language.text(Text::FarTicks),
                )
                .changed();
            changed |= ui
                .checkbox(
                    &mut record.appearance.near_tick_labels,
                    self.language.text(Text::NearTickLabels),
                )
                .changed();
            changed |= ui
                .checkbox(
                    &mut record.appearance.far_tick_labels,
                    self.language.text(Text::FarTickLabels),
                )
                .changed();
            changed |= ui
                .checkbox(
                    &mut record.appearance.major_ticks,
                    self.language.text(Text::MajorTicks),
                )
                .changed();
            changed |= ui
                .checkbox(
                    &mut record.appearance.minor_ticks,
                    self.language.text(Text::MinorTicks),
                )
                .changed();
            ui.label(self.language.text(Text::TickDirection));
            egui::ComboBox::from_id_salt(("tick-direction", dimension))
                .selected_text(tick_direction_name(
                    self.language,
                    record.appearance.tick_direction,
                ))
                .show_ui(ui, |ui| {
                    for direction in [TickDirection::In, TickDirection::Out, TickDirection::InOut] {
                        changed |= ui
                            .selectable_value(
                                &mut record.appearance.tick_direction,
                                direction,
                                tick_direction_name(self.language, direction),
                            )
                            .changed();
                    }
                });
            changed |= ui
                .checkbox(
                    &mut record.appearance.grid_major,
                    self.language.text(Text::MajorGrid),
                )
                .changed();
            changed |= ui
                .checkbox(
                    &mut record.appearance.grid_minor,
                    self.language.text(Text::MinorGrid),
                )
                .changed();
            for (value, label) in [
                (
                    &mut record.appearance.tick_label_pad_pt,
                    self.language.text(Text::TickLabelPad),
                ),
                (
                    &mut record.appearance.label_edge_pad_pt,
                    self.language.text(Text::LabelEdgePad),
                ),
                (
                    &mut record.appearance.label_tick_pad_pt,
                    self.language.text(Text::LabelTickPad),
                ),
            ] {
                changed |= ui
                    .add(
                        egui::DragValue::new(value)
                            .range(0.0..=72.0)
                            .prefix(format!("{label}: ")),
                    )
                    .changed();
            }
        });
        match dimension {
            AxisDimension::X => self.x_fixed_ticks = fixed_ticks,
            AxisDimension::Y => self.y_fixed_ticks = fixed_ticks,
        }
        if let Some(error) = fixed_tick_error {
            self.push_error("fixed-ticks", error);
        }
        if changed {
            self.execute_document_edit(EditCommand::SetAxisRecord { dimension, record }, title);
        }
        self.axis_label_editor(ui, dimension);
    }

    fn axis_label_editor(&mut self, ui: &mut egui::Ui, dimension: AxisDimension) {
        let draft = match dimension {
            AxisDimension::X => &mut self.x_label_draft,
            AxisDimension::Y => &mut self.y_label_draft,
        };
        let mut remove = None;
        ui.collapsing(self.language.text(Text::AxisLabel), |ui| {
            ui.weak(self.language.text(Text::SemanticPart));
            for (index, part) in draft.parts.iter_mut().enumerate() {
                ui.push_id(("label-part", dimension, index), |ui| {
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt("kind")
                            .selected_text(label_part_kind_name(self.language, part.kind))
                            .show_ui(ui, |ui| {
                                for kind in label_part_kinds() {
                                    ui.selectable_value(
                                        &mut part.kind,
                                        kind,
                                        label_part_kind_name(self.language, kind),
                                    );
                                }
                            });
                        if part.kind != LabelPartKind::UnitSeparator {
                            ui.text_edit_singleline(&mut part.value);
                        }
                        if ui
                            .small_button("−")
                            .on_hover_text(self.language.text(Text::Remove))
                            .clicked()
                        {
                            remove = Some(index);
                        }
                    });
                });
            }
            if ui.button(self.language.text(Text::AddPart)).clicked() {
                draft.parts.push(LabelPartDraft {
                    kind: LabelPartKind::Text,
                    value: String::new(),
                });
            }
        });
        if let Some(index) = remove {
            draft.parts.remove(index);
        }
        let apply = ui.button(self.language.text(Text::ApplyLabel)).clicked();
        if apply {
            match draft.to_nodes() {
                Ok(nodes) => {
                    self.execute_document_edit(
                        EditCommand::SetAxisLabel { dimension, nodes },
                        self.language.text(Text::ApplyLabel),
                    );
                }
                Err(error) => self.push_error("axis-label", error),
            }
        }
    }

    fn sync_axis_editors(&mut self) {
        self.x_label_draft = LabelDraft::from_nodes(self.document.axis_label(AxisDimension::X));
        self.y_label_draft = LabelDraft::from_nodes(self.document.axis_label(AxisDimension::Y));
        self.x_fixed_ticks = fixed_ticks_text(&self.document.axis_record(AxisDimension::X));
        self.y_fixed_ticks = fixed_ticks_text(&self.document.axis_record(AxisDimension::Y));
    }

    fn select_dataset(&mut self, data_source_id: &str) {
        self.selected_dataset = Some(data_source_id.to_owned());
        if let Some(dataset) = self
            .session
            .datasets()
            .iter()
            .find(|dataset| dataset.plot_id == data_source_id)
        {
            self.binding_x = dataset
                .fit_link
                .as_ref()
                .map(|fit| fit.source_x_column.clone())
                .filter(|name| dataset.columns.iter().any(|column| column.name == *name))
                .or_else(|| dataset.columns.first().map(|column| column.name.clone()))
                .unwrap_or_default();
            self.binding_y = dataset
                .fit_link
                .as_ref()
                .map(|fit| fit.source_y_column.clone())
                .filter(|name| dataset.columns.iter().any(|column| column.name == *name))
                .or_else(|| dataset.columns.get(1).map(|column| column.name.clone()))
                .or_else(|| dataset.columns.first().map(|column| column.name.clone()))
                .unwrap_or_default();
            self.binding_error = dataset
                .columns
                .iter()
                .find(|column| column.name != self.binding_x && column.name != self.binding_y)
                .map(|column| column.name.clone())
                .unwrap_or_default();
            self.creation_style = match dataset.kind {
                DataSetKind::Source => SeriesCreationStyle::Scatter,
                DataSetKind::Fit => SeriesCreationStyle::Line,
            };
        }
    }

    fn select_series_for_editing(&mut self, series: &SeriesDescriptor) {
        self.selected_series = Some(series.id.clone());
        if let Some(binding) = &series.binding {
            self.select_dataset(&binding.data_source_id);
            self.binding_x.clone_from(&binding.x_column);
            self.binding_y.clone_from(&binding.y_column);
            if series.kind == SeriesKind::ErrorBar
                && let Some(error_column) = self
                    .document
                    .project()
                    .figure
                    .artists
                    .iter()
                    .find_map(|artist| (artist.id == series.id).then_some(&artist.properties))
                && let instplot_studio::ArtistProperties::ErrorBar { y_error_column, .. } =
                    error_column
            {
                self.binding_error.clone_from(y_error_column);
            }
        }
    }

    fn create_series_from_selection(&mut self) {
        let Some(data_source_id) = self.selected_dataset.clone() else {
            return;
        };
        let before = self
            .document
            .series()
            .into_iter()
            .map(|series| series.id)
            .collect::<std::collections::BTreeSet<_>>();
        let command = EditCommand::CreateSeries {
            data_source_id,
            x_column: self.binding_x.clone(),
            y_column: self.binding_y.clone(),
            style: self.creation_style,
        };
        if self.execute_document_edit(command, self.language.text(Text::AddSeries)) {
            self.selected_series = self
                .document
                .series()
                .into_iter()
                .find(|series| !before.contains(&series.id) && series.kind != SeriesKind::Legend)
                .map(|series| series.id);
        }
    }

    fn delete_data_source_dialog(&mut self, context: &egui::Context) {
        let Some(data_source_id) = self.pending_delete_source.clone() else {
            return;
        };
        let dependencies = self.document.data_source_dependency_count(&data_source_id);
        let label = self
            .session
            .datasets()
            .iter()
            .find(|dataset| dataset.plot_id == data_source_id)
            .map_or_else(|| data_source_id.clone(), |dataset| dataset.display_name());
        egui::Window::new(self.language.text(Text::DeleteDataSourceQuestion))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(context, |ui| {
                ui.label(&label);
                if dependencies > 0 {
                    ui.colored_label(
                        egui::Color32::YELLOW,
                        self.language.dependencies_will_be_deleted(dependencies),
                    );
                }
                ui.horizontal(|ui| {
                    let delete_label = if dependencies > 0 {
                        self.language.text(Text::DeleteWithDependencies)
                    } else {
                        self.language.text(Text::Delete)
                    };
                    if ui.button(delete_label).clicked() {
                        let command = EditCommand::DeleteDataSource {
                            data_source_id: data_source_id.clone(),
                            cascade: dependencies > 0,
                        };
                        if self.execute_document_edit(command, delete_label) {
                            (self.session, _) =
                                StudioSession::from_project(self.document.project());
                            self.selected_dataset = None;
                            self.selected_series = None;
                        }
                        self.pending_delete_source = None;
                    }
                    if ui.button(self.language.text(Text::Cancel)).clicked() {
                        self.pending_delete_source = None;
                    }
                });
            });
    }

    fn series_tree(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.language.text(Text::Series));
        ui.collapsing(self.language.text(Text::FixedFigure), |ui| {
            for series in self.document.series() {
                let selected = self.selected_series.as_deref() == Some(series.id.as_str());
                ui.horizontal(|ui| {
                    let mut visible = series.visible;
                    if ui.checkbox(&mut visible, "").changed() {
                        self.execute_document_edit(
                            EditCommand::SetSeriesVisible {
                                artist_id: series.id.clone(),
                                visible,
                            },
                            self.language.text(Text::Visible),
                        );
                    }
                    let label = format!(
                        "{} · {}",
                        series.label,
                        series_kind_name(self.language, series.kind)
                    );
                    if ui.selectable_label(selected, label).clicked() {
                        self.select_series_for_editing(&series);
                    }
                });
            }
        });
        if let Some(selected_id) = self.selected_series.clone()
            && let Some(series) = self
                .document
                .series()
                .into_iter()
                .find(|series| series.id == selected_id)
            && matches!(
                series.kind,
                SeriesKind::Line | SeriesKind::Scatter | SeriesKind::ErrorBar
            )
        {
            ui.horizontal_wrapped(|ui| {
                if ui.button(self.language.text(Text::Duplicate)).clicked() {
                    let before = self
                        .document
                        .series()
                        .into_iter()
                        .map(|series| series.id)
                        .collect::<std::collections::BTreeSet<_>>();
                    if self.execute_document_edit(
                        EditCommand::DuplicateSeries {
                            artist_id: selected_id.clone(),
                        },
                        self.language.text(Text::Duplicate),
                    ) {
                        self.selected_series = self
                            .document
                            .series()
                            .into_iter()
                            .find(|series| !before.contains(&series.id))
                            .map(|series| series.id);
                    }
                }
                if ui.button(self.language.text(Text::Delete)).clicked()
                    && self.execute_document_edit(
                        EditCommand::DeleteSeries {
                            artist_id: selected_id.clone(),
                        },
                        self.language.text(Text::Delete),
                    )
                {
                    self.selected_series = None;
                }
                if ui.button(self.language.text(Text::MoveEarlier)).clicked() {
                    self.execute_document_edit(
                        EditCommand::MoveSeries {
                            artist_id: selected_id.clone(),
                            direction: MoveDirection::Earlier,
                        },
                        self.language.text(Text::MoveEarlier),
                    );
                }
                if ui.button(self.language.text(Text::MoveLater)).clicked() {
                    self.execute_document_edit(
                        EditCommand::MoveSeries {
                            artist_id: selected_id.clone(),
                            direction: MoveDirection::Later,
                        },
                        self.language.text(Text::MoveLater),
                    );
                }
            });
        }
        ui.separator();
        ui.heading(self.language.text(Text::Data));
        let datasets = self.session.datasets().to_vec();
        if datasets.is_empty() {
            ui.weak(self.language.text(Text::NoData));
        } else {
            for dataset in &datasets {
                let selected = self.selected_dataset.as_deref() == Some(dataset.plot_id.as_str());
                ui.push_id(&dataset.plot_id, |ui| {
                    if ui
                        .selectable_label(selected, dataset.display_name())
                        .clicked()
                    {
                        self.select_dataset(&dataset.plot_id);
                    }
                    let kind = match dataset.kind {
                        DataSetKind::Source => self.language.text(Text::SourceKind),
                        DataSetKind::Fit => self.language.text(Text::FitKind),
                    };
                    let alive = dataset.alive.iter().filter(|alive| **alive).count();
                    ui.weak(format!(
                        "{kind} · {} · {}: {alive}",
                        self.language
                            .rows_columns(dataset.row_count, dataset.columns.len()),
                        self.language.text(Text::AliveRows)
                    ));
                    if let Some(link) = &dataset.fit_link
                        && let Some(parent) = &link.parent_dataset_id
                    {
                        ui.weak(format!(
                            "{}: {parent}",
                            self.language.text(Text::ParentSource)
                        ));
                    }
                    ui.weak(format!(
                        "{}: {}",
                        self.language.text(Text::Columns),
                        dataset
                            .columns
                            .iter()
                            .map(|column| column.name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                });
            }
        }
        if let Some(selected_id) = self.selected_dataset.clone()
            && let Some(dataset) = datasets
                .iter()
                .find(|dataset| dataset.plot_id == selected_id)
        {
            ui.separator();
            ui.label(self.language.text(Text::CreateSeries));
            column_combo(
                ui,
                "create-x-column",
                self.language.text(Text::XColumn),
                &mut self.binding_x,
                dataset,
            );
            column_combo(
                ui,
                "create-y-column",
                self.language.text(Text::YColumn),
                &mut self.binding_y,
                dataset,
            );
            egui::ComboBox::from_label(self.language.text(Text::SeriesStyle))
                .selected_text(series_style_name(self.language, self.creation_style))
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.creation_style,
                        SeriesCreationStyle::Line,
                        self.language.text(Text::Line),
                    );
                    ui.selectable_value(
                        &mut self.creation_style,
                        SeriesCreationStyle::Scatter,
                        self.language.text(Text::Scatter),
                    );
                    ui.selectable_value(
                        &mut self.creation_style,
                        SeriesCreationStyle::LineAndMarker,
                        self.language.text(Text::LineAndMarker),
                    );
                });
            if ui.button(self.language.text(Text::AddSeries)).clicked() {
                self.create_series_from_selection();
            }
            if ui
                .button(self.language.text(Text::DeleteDataSource))
                .clicked()
            {
                self.pending_delete_source = Some(selected_id);
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

        if let Some(series) = &series {
            self.artist_editor(ui, &series.id);
        }

        if let Some(series) = &series
            && let Some(binding) = &series.binding
        {
            ui.separator();
            ui.label(self.language.text(Text::DataBinding));
            let datasets = self.session.datasets().to_vec();
            egui::ComboBox::from_id_salt("binding-data-source")
                .selected_text(
                    datasets
                        .iter()
                        .find(|dataset| dataset.plot_id == binding.data_source_id)
                        .map_or_else(
                            || binding.data_source_id.clone(),
                            |data| data.display_name(),
                        ),
                )
                .show_ui(ui, |ui| {
                    for dataset in &datasets {
                        if ui
                            .selectable_label(
                                self.selected_dataset.as_deref() == Some(dataset.plot_id.as_str()),
                                dataset.display_name(),
                            )
                            .clicked()
                        {
                            self.select_dataset(&dataset.plot_id);
                        }
                    }
                });
            if let Some(dataset_id) = self.selected_dataset.as_deref()
                && let Some(dataset) = datasets
                    .iter()
                    .find(|dataset| dataset.plot_id == dataset_id)
            {
                column_combo(
                    ui,
                    "binding-x-column",
                    self.language.text(Text::XColumn),
                    &mut self.binding_x,
                    dataset,
                );
                column_combo(
                    ui,
                    "binding-y-column",
                    self.language.text(Text::YColumn),
                    &mut self.binding_y,
                    dataset,
                );
                if series.kind == SeriesKind::ErrorBar {
                    column_combo(
                        ui,
                        "binding-error-column",
                        self.language.text(Text::ErrorColumn),
                        &mut self.binding_error,
                        dataset,
                    );
                }
                if ui.button(self.language.text(Text::ApplyBinding)).clicked() {
                    self.execute_document_edit(
                        EditCommand::RebindSeries {
                            artist_id: series.id.clone(),
                            data_source_id: dataset.plot_id.clone(),
                            x_column: self.binding_x.clone(),
                            y_column: self.binding_y.clone(),
                            y_error_column: (series.kind == SeriesKind::ErrorBar)
                                .then(|| self.binding_error.clone()),
                        },
                        self.language.text(Text::ApplyBinding),
                    );
                }
            }
        }

        ui.separator();
        self.figure_size_editor(ui);

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

        self.axis_editor(ui, AxisDimension::X);
        self.axis_editor(ui, AxisDimension::Y);

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

    fn artist_editor(&mut self, ui: &mut egui::Ui, artist_id: &str) {
        let Some(mut record) = self.document.artist_record(artist_id) else {
            return;
        };
        let palette = self.document.palette_colors().to_vec();
        let mut changed = false;
        let mut semantic_labels = Vec::new();
        ui.separator();
        ui.collapsing(self.language.text(Text::ArtistProperties), |ui| {
            if !matches!(record.role, ArtistRole::Annotation | ArtistRole::Legend) {
                changed |= role_editor(ui, self.language, &mut record.role);
            }
            match &mut record.properties {
                ArtistProperties::Line { stroke, .. } => {
                    changed |= stroke_editor(ui, self.language, stroke, &palette);
                }
                ArtistProperties::Scatter { marker, .. } => {
                    changed |= color_editor(ui, self.language, &mut marker.color_id, &palette);
                    ui.label(self.language.text(Text::MarkerShape));
                    egui::ComboBox::from_id_salt(("marker-shape", artist_id))
                        .selected_text(marker_shape_name(self.language, marker.shape))
                        .show_ui(ui, |ui| {
                            for shape in [
                                MarkerShape::Circle,
                                MarkerShape::Square,
                                MarkerShape::Triangle,
                                MarkerShape::Diamond,
                            ] {
                                changed |= ui
                                    .selectable_value(
                                        &mut marker.shape,
                                        shape,
                                        marker_shape_name(self.language, shape),
                                    )
                                    .changed();
                            }
                        });
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut marker.size_pt)
                                .range(0.1..=72.0)
                                .prefix(format!("{}: ", self.language.text(Text::MarkerSize))),
                        )
                        .changed();
                }
                ArtistProperties::ErrorBar {
                    cap_width_pt,
                    stroke,
                    ..
                } => {
                    changed |= ui
                        .add(
                            egui::DragValue::new(cap_width_pt)
                                .range(0.1..=72.0)
                                .prefix(format!("{}: ", self.language.text(Text::CapWidth))),
                        )
                        .changed();
                    changed |= stroke_editor(ui, self.language, stroke, &palette);
                }
                ArtistProperties::ReferenceLine {
                    orientation,
                    value,
                    stroke,
                } => {
                    ui.label(self.language.text(Text::Orientation));
                    egui::ComboBox::from_id_salt(("reference-orientation", artist_id))
                        .selected_text(reference_orientation_name(self.language, *orientation))
                        .show_ui(ui, |ui| {
                            for candidate in [
                                ReferenceOrientation::Horizontal,
                                ReferenceOrientation::Vertical,
                            ] {
                                changed |= ui
                                    .selectable_value(
                                        orientation,
                                        candidate,
                                        reference_orientation_name(self.language, candidate),
                                    )
                                    .changed();
                            }
                        });
                    changed |= ui
                        .add(
                            egui::DragValue::new(value)
                                .prefix(format!("{}: ", self.language.text(Text::Value))),
                        )
                        .changed();
                    changed |= stroke_editor(ui, self.language, stroke, &palette);
                }
                ArtistProperties::Annotation {
                    label_id,
                    x_pt,
                    y_pt,
                } => {
                    changed |= position_editor(ui, self.language, x_pt, y_pt);
                    semantic_labels.push(label_id.clone());
                }
                ArtistProperties::Legend {
                    entries,
                    x_pt,
                    y_pt,
                } => {
                    changed |= position_editor(ui, self.language, x_pt, y_pt);
                    ui.label(self.language.text(Text::LegendEntries));
                    let mut move_entry = None;
                    let entry_count = entries.len();
                    for (index, entry) in entries.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            changed |= ui.checkbox(&mut entry.visible, "").changed();
                            ui.label(&entry.artist_id);
                            if ui.small_button("↑").clicked() && index > 0 {
                                move_entry = Some((index, index - 1));
                            }
                            if ui.small_button("↓").clicked() && index + 1 < entry_count {
                                move_entry = Some((index, index + 1));
                            }
                        });
                        semantic_labels.push(entry.label_id.clone());
                    }
                    if let Some((from, to)) = move_entry {
                        entries.swap(from, to);
                        changed = true;
                    }
                }
            }
        });
        if changed {
            self.execute_document_edit(
                EditCommand::SetArtistRecord(record),
                self.language.text(Text::ArtistProperties),
            );
        }
        for label_id in semantic_labels {
            self.semantic_label_editor(ui, &label_id);
        }
    }

    fn semantic_label_editor(&mut self, ui: &mut egui::Ui, label_id: &str) {
        let Some(nodes) = self.document.semantic_label_nodes(label_id) else {
            return;
        };
        let mut draft = LabelDraft::from_nodes(nodes);
        let mut changed = false;
        let mut remove = None;
        ui.collapsing(
            format!("{} · {label_id}", self.language.text(Text::SemanticLabel)),
            |ui| {
                for (index, part) in draft.parts.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt(("semantic-kind", label_id, index))
                            .selected_text(label_part_kind_name(self.language, part.kind))
                            .show_ui(ui, |ui| {
                                for kind in label_part_kinds() {
                                    changed |= ui
                                        .selectable_value(
                                            &mut part.kind,
                                            kind,
                                            label_part_kind_name(self.language, kind),
                                        )
                                        .changed();
                                }
                            });
                        if part.kind != LabelPartKind::UnitSeparator {
                            changed |= ui.text_edit_singleline(&mut part.value).changed();
                        }
                        if ui.small_button("−").clicked() {
                            remove = Some(index);
                        }
                    });
                }
                if ui.button(self.language.text(Text::AddPart)).clicked() {
                    draft.parts.push(LabelPartDraft {
                        kind: LabelPartKind::Text,
                        value: String::new(),
                    });
                    changed = true;
                }
            },
        );
        if let Some(index) = remove {
            draft.parts.remove(index);
            changed = true;
        }
        if changed {
            match draft.to_nodes() {
                Ok(nodes) => {
                    self.execute_document_edit(
                        EditCommand::SetSemanticLabel {
                            label_id: label_id.to_owned(),
                            nodes,
                        },
                        self.language.text(Text::ApplyLabel),
                    );
                }
                Err(error) => self.push_error("semantic-label", error),
            }
        }
    }
}

fn role_editor(ui: &mut egui::Ui, language: UiLanguage, role: &mut ArtistRole) -> bool {
    let mut changed = false;
    ui.label(language.text(Text::Role));
    egui::ComboBox::from_id_salt("artist-role")
        .selected_text(artist_role_name(language, *role))
        .show_ui(ui, |ui| {
            for candidate in [
                ArtistRole::Data,
                ArtistRole::Fit,
                ArtistRole::Theory,
                ArtistRole::Reference,
                ArtistRole::Baseline,
            ] {
                changed |= ui
                    .selectable_value(role, candidate, artist_role_name(language, candidate))
                    .changed();
            }
        });
    changed
}

fn color_editor(
    ui: &mut egui::Ui,
    language: UiLanguage,
    color_id: &mut String,
    palette: &[instplot_studio::PaletteColor],
) -> bool {
    let mut changed = false;
    ui.label(language.text(Text::Color));
    egui::ComboBox::from_id_salt("artist-color")
        .selected_text(color_id.as_str())
        .show_ui(ui, |ui| {
            for color in palette {
                ui.horizontal(|ui| {
                    let [r, g, b, a] = color.rgba;
                    let swatch = egui::Color32::from_rgba_unmultiplied(r, g, b, a);
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 2.0, swatch);
                    changed |= ui
                        .selectable_value(color_id, color.id.clone(), &color.id)
                        .changed();
                });
            }
        });
    changed
}

fn stroke_editor(
    ui: &mut egui::Ui,
    language: UiLanguage,
    stroke: &mut StrokeStyle,
    palette: &[instplot_studio::PaletteColor],
) -> bool {
    let mut changed = color_editor(ui, language, &mut stroke.color_id, palette);
    changed |= ui
        .add(
            egui::DragValue::new(&mut stroke.width_pt)
                .range(0.1..=72.0)
                .prefix(format!("{}: ", language.text(Text::LineWidth))),
        )
        .changed();
    let selected = dash_name(language, &stroke.dash_pt);
    ui.label(language.text(Text::Dash));
    egui::ComboBox::from_id_salt("artist-dash")
        .selected_text(selected)
        .show_ui(ui, |ui| {
            for (name, pattern) in [
                (Text::Solid, Vec::new()),
                (Text::Dashed, vec![4.0, 2.4]),
                (Text::Dotted, vec![0.8, 1.8]),
                (Text::DashDot, vec![4.0, 2.0, 0.8, 2.0]),
            ] {
                changed |= ui
                    .selectable_value(&mut stroke.dash_pt, pattern, language.text(name))
                    .changed();
            }
        });
    changed
}

fn position_editor(
    ui: &mut egui::Ui,
    language: UiLanguage,
    x_pt: &mut f64,
    y_pt: &mut f64,
) -> bool {
    let mut changed = ui
        .add(egui::DragValue::new(x_pt).prefix(format!("{}: ", language.text(Text::XPosition))))
        .changed();
    changed |= ui
        .add(egui::DragValue::new(y_pt).prefix(format!("{}: ", language.text(Text::YPosition))))
        .changed();
    changed
}

fn artist_role_name(language: UiLanguage, role: ArtistRole) -> &'static str {
    language.text(match role {
        ArtistRole::Data => Text::DataRole,
        ArtistRole::Fit => Text::FitRole,
        ArtistRole::Theory => Text::TheoryRole,
        ArtistRole::Reference => Text::ReferenceRole,
        ArtistRole::Baseline => Text::BaselineRole,
        ArtistRole::Annotation => Text::Annotation,
        ArtistRole::Legend => Text::Legend,
    })
}

fn marker_shape_name(language: UiLanguage, shape: MarkerShape) -> &'static str {
    language.text(match shape {
        MarkerShape::Circle => Text::Circle,
        MarkerShape::Square => Text::Square,
        MarkerShape::Triangle => Text::Triangle,
        MarkerShape::Diamond => Text::Diamond,
    })
}

fn reference_orientation_name(
    language: UiLanguage,
    orientation: ReferenceOrientation,
) -> &'static str {
    language.text(match orientation {
        ReferenceOrientation::Horizontal => Text::Horizontal,
        ReferenceOrientation::Vertical => Text::Vertical,
    })
}

fn dash_name(language: UiLanguage, pattern: &[f64]) -> &'static str {
    if pattern.is_empty() {
        language.text(Text::Solid)
    } else if pattern == [4.0, 2.4] {
        language.text(Text::Dashed)
    } else if pattern == [0.8, 1.8] {
        language.text(Text::Dotted)
    } else {
        language.text(Text::DashDot)
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
        self.delete_data_source_dialog(&context);
    }
}

fn resolved_preview(
    document: &FigureDocument,
) -> Result<ResolvedFigure, instplot_studio::DocumentLayoutError> {
    resolve_document(document)
}

fn series_style_name(language: UiLanguage, style: SeriesCreationStyle) -> &'static str {
    match style {
        SeriesCreationStyle::Line => language.text(Text::Line),
        SeriesCreationStyle::Scatter => language.text(Text::Scatter),
        SeriesCreationStyle::LineAndMarker => language.text(Text::LineAndMarker),
    }
}

fn series_kind_name(language: UiLanguage, kind: SeriesKind) -> &'static str {
    match kind {
        SeriesKind::Line => language.text(Text::Line),
        SeriesKind::Scatter => language.text(Text::Scatter),
        SeriesKind::ErrorBar => language.text(Text::ErrorBar),
        SeriesKind::ReferenceLine => language.text(Text::ReferenceLine),
        SeriesKind::Annotation => language.text(Text::Annotation),
        SeriesKind::Legend => language.text(Text::Legend),
    }
}

fn fixed_ticks_text(record: &AxisRecord) -> String {
    match &record.locator {
        LocatorSpec::Fixed { values } => values
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", "),
        LocatorSpec::Auto { .. } => String::new(),
    }
}

fn parse_fixed_ticks(value: &str) -> Result<Vec<f64>, String> {
    let parsed = value
        .split([',', ';', ' ', '\t'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.parse::<f64>()
                .map_err(|_| format!("invalid fixed tick value: {part}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if parsed.is_empty() || parsed.iter().any(|value| !value.is_finite()) {
        return Err("fixed ticks require one or more finite values".to_owned());
    }
    Ok(parsed)
}

fn tick_direction_name(language: UiLanguage, direction: TickDirection) -> &'static str {
    match direction {
        TickDirection::In => language.text(Text::Inward),
        TickDirection::Out => language.text(Text::Outward),
        TickDirection::InOut => language.text(Text::InAndOut),
    }
}

fn label_part_kinds() -> [LabelPartKind; 13] {
    [
        LabelPartKind::Text,
        LabelPartKind::Variable,
        LabelPartKind::Upright,
        LabelPartKind::GreekVariable,
        LabelPartKind::Number,
        LabelPartKind::DescriptiveSubscript,
        LabelPartKind::VariableSubscript,
        LabelPartKind::Superscript,
        LabelPartKind::Unit,
        LabelPartKind::UnitSeparator,
        LabelPartKind::Operator,
        LabelPartKind::Emphasis,
        LabelPartKind::BoldVariable,
    ]
}

fn label_part_kind_name(language: UiLanguage, kind: LabelPartKind) -> &'static str {
    let key = match kind {
        LabelPartKind::Text => "text",
        LabelPartKind::Variable => "variable",
        LabelPartKind::Upright => "upright",
        LabelPartKind::GreekVariable => "greek_variable",
        LabelPartKind::Number => "number",
        LabelPartKind::DescriptiveSubscript => "descriptive_subscript",
        LabelPartKind::VariableSubscript => "variable_subscript",
        LabelPartKind::Superscript => "superscript",
        LabelPartKind::Unit => "unit",
        LabelPartKind::UnitSeparator => "unit_separator",
        LabelPartKind::Operator => "operator",
        LabelPartKind::Emphasis => "emphasis",
        LabelPartKind::BoldVariable => "bold_variable",
    };
    language.semantic_part_name(key)
}

fn column_combo(
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    selected: &mut String,
    dataset: &DataSet,
) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(format!("{label}: {selected}"))
        .show_ui(ui, |ui| {
            for column in &dataset.columns {
                ui.selectable_value(selected, column.name.clone(), &column.name);
            }
        });
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
