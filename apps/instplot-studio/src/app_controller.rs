use super::*;

impl StudioApp {
    pub(super) fn new(
        creation: &eframe::CreationContext<'_>,
        started: Instant,
        startup: Option<HandoffImport>,
    ) -> Self {
        creation.egui_ctx.options_mut(|options| {
            options.zoom_with_keyboard = true;
        });
        instplot_ui::install_publication_fonts(&creation.egui_ctx);
        configure_interface_style(&creation.egui_ctx);
        let startup_unsaved = startup.is_some();
        let language = UiLanguage::default();
        let (document, datasets, status, workspace) = startup.map_or_else(
            || {
                let document = FigureDocument::showcase();
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
        let publication_report = check_publication(
            &document,
            &resolved,
            document.export_preferences().selected_raster_dpi,
        );
        let mut session = StudioSession::default();
        session.replace_datasets(datasets);
        let edit_history = EditHistory::new(&document, !startup_unsaved);
        let x_fixed_ticks = fixed_ticks_text(&document.axis_record(AxisDimension::X));
        let y_fixed_ticks = fixed_ticks_text(&document.axis_record(AxisDimension::Y));
        Self {
            branding: instplot_ui::Branding::new(PRODUCT_NAME, env!("CARGO_PKG_VERSION"))
                .with_build_id(BUILD_ID),
            features: instplot_ui::FeatureSet::STUDIO,
            session,
            document,
            resolved,
            publication_report,
            preview: EguiPreviewAdapter::default(),
            selected_series: None,
            selected_canvas_node: None,
            selected_canvas_role: None,
            selection_candidates: Vec::new(),
            selected_dataset: None,
            binding_x: String::new(),
            binding_y: String::new(),
            binding_error: String::new(),
            pending_data_removal: None,
            pending_managed_save_conflict: None,
            label_inputs: BTreeMap::new(),
            numeric_inputs: BTreeMap::new(),
            x_fixed_ticks,
            y_fixed_ticks,
            workspace,
            edit_history,
            pending_action: None,
            pending_export: None,
            allow_close: false,
            canvas_zoom: 1.5,
            canvas_scroll: egui::Vec2::ZERO,
            hover_data_coordinates: None,
            trackpad_scroll_active: false,
            show_layers: false,
            show_inspector: false,
            show_palette: false,
            show_messages: false,
            focus_inspector: false,
            focus_palette: false,
            focus_manual_data: false,
            context_editor_focus_target: None,
            context_editor_targets: Vec::new(),
            active_artist_drag: None,
            drawing_tool: DrawingTool::Select,
            tool_draft: None,
            messages: Vec::new(),
            status: Some((status, Instant::now())),
            manual_data: ManualDataState::default(),
            marker_size_for_all: false,
            marker_interval_for_all: false,
            marker_fill_for_all: false,
            language,
            #[cfg(target_os = "macos")]
            macos_open_files: None,
            first_frame: true,
            started,
        }
    }

    pub(super) fn open_data(&mut self) {
        let supported = DATA_FORMAT_CAPABILITIES
            .iter()
            .flat_map(|format| format.extensions.iter().copied())
            .collect::<Vec<_>>();
        let mut dialog = rfd::FileDialog::new()
            .set_title(self.language.text(Text::OpenDataDialog))
            .add_filter(self.language.text(Text::SupportedData), &supported);
        for format in DATA_FORMAT_CAPABILITIES {
            dialog = dialog.add_filter(format.name, format.extensions);
        }
        let Some(paths) = dialog.pick_files() else {
            return;
        };
        self.load_data_paths(paths);
    }

    pub(super) fn load_data_paths(&mut self, paths: Vec<PathBuf>) {
        let state = AppTransactionState {
            document: &self.document,
            session: &self.session,
            workspace: &self.workspace,
            edit_history: &self.edit_history,
        };
        let preferred_columns = (!self.binding_x.is_empty() && !self.binding_y.is_empty())
            .then(|| (self.binding_x.clone(), self.binding_y.clone()));
        let outcome = match ApplicationController::execute(
            state,
            AppAction::ImportFiles {
                paths,
                preferred_columns,
            },
        ) {
            Ok(outcome) => outcome,
            Err(error) => {
                self.push_error(
                    error.code,
                    self.language.operation_failed(
                        self.language.text(Text::ImportDataOperation),
                        &error.diagnostics.join("；"),
                    ),
                );
                self.show_messages = true;
                return;
            }
        };
        let effect = self.commit_app_outcome(outcome);
        let AppEffect::Imported {
            paths: imported_paths,
            imported_ids,
            read,
            added,
            replaced,
            diagnostics,
            replaced_showcase,
        } = effect
        else {
            unreachable!("import action must produce an import effect");
        };
        if replaced_showcase {
            self.selected_series = None;
            self.selected_canvas_node = None;
            self.selected_canvas_role = None;
            self.selection_candidates.clear();
            self.context_editor_targets.clear();
            self.active_artist_drag = None;
            self.label_inputs.clear();
        }
        self.sync_axis_editors();
        if let Some(data_source_id) = self
            .session
            .datasets()
            .iter()
            .find(|dataset| {
                imported_ids.contains(&dataset.plot_id) && dataset.kind == DataSetKind::Source
            })
            .or_else(|| {
                self.session
                    .datasets()
                    .iter()
                    .find(|dataset| imported_ids.contains(&dataset.plot_id))
            })
            .map(|dataset| dataset.plot_id.clone())
        {
            self.select_dataset(&data_source_id);
        }
        self.show_layers = true;
        self.first_frame = true;
        self.canvas_scroll = egui::Vec2::ZERO;
        self.set_success(self.language.imported(read, added, replaced));
        self.clear_message("import");
        self.clear_message("data-sync");
        if !diagnostics.is_empty() {
            self.push_warning("import-partial", diagnostics.join("；"));
            self.show_messages = true;
        } else {
            self.clear_message("import-partial");
        }
        debug_assert!(!imported_paths.is_empty());
    }

    pub(super) fn open_paths(&mut self, paths: Vec<PathBuf>) {
        let mut project_paths = paths
            .iter()
            .filter(|path| {
                path.extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("instplot"))
            })
            .cloned();
        if let Some(project_path) = project_paths.next() {
            if project_paths.next().is_some() {
                self.push_warning(
                    "open-project-multiple",
                    "一次只能打开一个 InstPlot 项目，已打开第一个项目。".to_owned(),
                );
                self.show_messages = true;
            }
            self.request_replacement(PendingAction::OpenProjectPath(project_path));
            return;
        }
        self.load_data_paths(paths);
    }
    pub(super) fn open_lite_handoff(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title(self.language.text(Text::OpenLiteDialog))
            .add_filter(self.language.text(Text::HandoffFile), &["instplot-handoff"])
            .pick_file()
        else {
            return;
        };
        let state = AppTransactionState {
            document: &self.document,
            session: &self.session,
            workspace: &self.workspace,
            edit_history: &self.edit_history,
        };
        match ApplicationController::execute(
            state,
            AppAction::OpenHandoff {
                path,
                untitled: self.language.text(Text::Untitled).to_owned(),
            },
        ) {
            Ok(outcome) => {
                let effect = self.commit_app_outcome(outcome);
                let AppEffect::OpenedHandoff {
                    producer_name,
                    producer_version,
                } = effect
                else {
                    unreachable!("handoff action must produce a handoff effect");
                };
                self.selected_series = None;
                self.selected_canvas_node = None;
                self.selected_canvas_role = None;
                self.context_editor_targets.clear();
                self.selected_dataset = None;
                self.sync_axis_editors();
                self.messages.clear();
                self.set_success(self.language.opened_lite(&producer_name, &producer_version));
            }
            Err(error) => {
                let operation = if error.code == "handoff-layout" {
                    self.language.text(Text::HandoffLayoutOperation)
                } else {
                    self.language.text(Text::OpenHandoffOperation)
                };
                self.push_error(
                    error.code,
                    self.language
                        .operation_failed(operation, &error.diagnostics.join("；")),
                );
            }
        }
    }

    pub(super) fn open_project(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title(self.language.text(Text::OpenProjectDialog))
            .add_filter(self.language.text(Text::ProjectFile), &["instplot"])
            .pick_file()
        else {
            return;
        };
        self.open_project_path(path);
    }

    pub(super) fn open_project_path(&mut self, path: PathBuf) {
        let state = AppTransactionState {
            document: &self.document,
            session: &self.session,
            workspace: &self.workspace,
            edit_history: &self.edit_history,
        };
        match ApplicationController::execute(state, AppAction::OpenProject(path)) {
            Ok(outcome) => {
                let effect = self.commit_app_outcome(outcome);
                let AppEffect::OpenedProject {
                    path,
                    source,
                    warnings,
                } = effect
                else {
                    unreachable!("open action must produce an open effect");
                };
                let status = match source {
                    OpenProjectSource::Primary => self.language.opened_project(&path),
                    OpenProjectSource::Backup => self.language.recovered_project(&path),
                };
                self.selected_series = None;
                self.selected_canvas_node = None;
                self.selected_canvas_role = None;
                self.context_editor_targets.clear();
                self.selected_dataset = None;
                self.show_layers = !self.session.datasets().is_empty();
                self.first_frame = true;
                self.sync_axis_editors();
                self.messages.clear();
                for (index, warning) in warnings.into_iter().enumerate() {
                    self.push_warning(format!("project-source-{index}"), warning);
                }
                self.set_success(status);
            }
            Err(error) => {
                let operation = if error.code == "project-layout" {
                    self.language.text(Text::ProjectLayoutOperation)
                } else {
                    self.language.text(Text::OpenProjectOperation)
                };
                self.push_error(
                    error.code,
                    self.language
                        .operation_failed(operation, &error.diagnostics.join("；")),
                );
            }
        }
    }

    pub(super) fn new_project(&mut self) {
        let state = AppTransactionState {
            document: &self.document,
            session: &self.session,
            workspace: &self.workspace,
            edit_history: &self.edit_history,
        };
        let outcome = match ApplicationController::execute(
            state,
            AppAction::NewProject {
                untitled: self.language.text(Text::Untitled).to_owned(),
            },
        ) {
            Ok(outcome) => outcome,
            Err(error) => {
                self.push_error(
                    error.code,
                    self.language.operation_failed(
                        self.language.text(Text::ProjectLayoutOperation),
                        &error.diagnostics.join("；"),
                    ),
                );
                return;
            }
        };
        let effect = self.commit_app_outcome(outcome);
        debug_assert!(matches!(effect, AppEffect::NewProject));
        self.selected_series = None;
        self.selected_canvas_node = None;
        self.selected_canvas_role = None;
        self.context_editor_targets.clear();
        self.selected_dataset = None;
        self.sync_axis_editors();
        self.messages.clear();
        self.set_success(self.language.text(Text::Ready).to_owned());
    }

    pub(super) fn save_project(&mut self, save_as: bool) -> bool {
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
        let has_manual_data = self
            .document
            .project()
            .data_sources
            .iter()
            .any(|source| source.origin == instplot_studio::DataSourceOrigin::Manual);
        if has_manual_data && (save_as || self.document.has_unmanaged_manual_sources()) {
            let selection = rfd::FileDialog::new()
                .set_title("选择手动数据格式与保存目录")
                .add_filter("CSV", &["csv"])
                .add_filter("TSV", &["tsv"])
                .add_filter("文本数据", &["txt"])
                .add_filter("DAT 数据", &["dat"])
                .add_filter("Excel 工作簿", &["xlsx"])
                .set_file_name("Data.csv")
                .save_file();
            let Some(selection) = selection else {
                return false;
            };
            let Some(format) = managed_format_from_path(&selection) else {
                self.push_error(
                    "save-manual-data",
                    "请选择 CSV、TSV、TXT、DAT 或 XLSX 格式".to_owned(),
                );
                return false;
            };
            let directory = selection.parent().unwrap_or_else(|| Path::new("."));
            if let Err(error) = self.document.configure_manual_data_files(directory, format) {
                self.push_error("save-manual-data", error);
                return false;
            }
        }
        self.execute_project_save(path, false)
    }

    pub(super) fn execute_project_save(&mut self, path: PathBuf, overwrite_managed: bool) -> bool {
        let state = AppTransactionState {
            document: &self.document,
            session: &self.session,
            workspace: &self.workspace,
            edit_history: &self.edit_history,
        };
        let action = if overwrite_managed {
            AppAction::SaveProjectOverwriteManaged(path.clone())
        } else {
            AppAction::SaveProject(path.clone())
        };
        match ApplicationController::execute(state, action) {
            Ok(outcome) => {
                let effect = self.commit_app_outcome(outcome);
                let AppEffect::SavedProject { path } = effect else {
                    unreachable!("save action must produce a save effect");
                };
                self.set_success(self.language.saved_project(&path));
                self.clear_message("save-project");
                true
            }
            Err(error) => {
                if let Some(explanation) = managed_save_conflict_explanation(&error.diagnostics) {
                    self.pending_managed_save_conflict = Some(ManagedSaveConflict {
                        project_path: path,
                        explanation,
                    });
                    return false;
                }
                self.push_error(
                    error.code,
                    self.language.operation_failed(
                        self.language.text(Text::SaveProjectOperation),
                        &error.diagnostics.join("；"),
                    ),
                );
                false
            }
        }
    }

    pub(super) fn managed_save_conflict_dialog(&mut self, context: &egui::Context) {
        let Some(conflict) = self.pending_managed_save_conflict.clone() else {
            return;
        };
        let mut overwrite = false;
        let mut save_as = false;
        let mut reopen = false;
        let mut cancel = false;
        egui::Window::new("手动数据文件冲突")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .frame(studio_card_frame(context.theme() == egui::Theme::Dark))
            .show(context, |ui| {
                studio_dialog_heading(ui, "手动数据文件冲突");
                ui.label(&conflict.explanation);
                ui.label("请选择如何处理；Studio 不会静默覆盖外部更改。");
                ui.horizontal_wrapped(|ui| {
                    overwrite = ui.button("覆盖外部文件").clicked();
                    save_as = ui.button("另存为…").clicked();
                    reopen = ui.button("重新打开已保存项目").clicked();
                    cancel = ui.button(self.language.text(Text::Cancel)).clicked();
                });
            });
        if overwrite {
            if self.execute_project_save(conflict.project_path, true) {
                self.pending_managed_save_conflict = None;
            }
        } else if save_as {
            self.pending_managed_save_conflict = None;
            self.save_project(true);
        } else if reopen {
            self.pending_managed_save_conflict = None;
            self.open_project_path(conflict.project_path);
        } else if cancel {
            self.pending_managed_save_conflict = None;
        }
    }

    pub(super) fn export_manual_data(&mut self) {
        let selection = rfd::FileDialog::new()
            .set_title("导出手动数据：选择格式与保存目录")
            .add_filter("CSV", &["csv"])
            .add_filter("TSV", &["tsv"])
            .add_filter("文本数据", &["txt"])
            .add_filter("DAT 数据", &["dat"])
            .add_filter("Excel 工作簿", &["xlsx"])
            .set_file_name("Data.csv")
            .save_file();
        let Some(selection) = selection else {
            return;
        };
        let Some(format) = managed_format_from_path(&selection) else {
            self.push_error(
                "export-manual-data",
                "请选择 CSV、TSV、TXT、DAT 或 XLSX 格式".to_owned(),
            );
            return;
        };
        let directory = selection.parent().unwrap_or_else(|| Path::new("."));
        match self.document.export_manual_data_files(directory, format) {
            Ok(0) => self.push_error("export-manual-data", "当前没有可导出的手动数据".to_owned()),
            Ok(count) => {
                self.clear_message("export-manual-data");
                self.set_success(format!("已导出 {count} 个手动数据文件"));
            }
            Err(error) => self.push_error("export-manual-data", error.to_string()),
        }
    }

    pub(super) fn export_pdf(&mut self) {
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

    pub(super) fn export_pdf_to(&mut self, path: &Path) {
        let state = AppTransactionState {
            document: &self.document,
            session: &self.session,
            workspace: &self.workspace,
            edit_history: &self.edit_history,
        };
        match ApplicationController::execute(
            state,
            AppAction::ExportFigure {
                path: path.to_path_buf(),
                format: FigureExport::Pdf,
            },
        ) {
            Ok(outcome) => {
                let effect = self.commit_app_outcome(outcome);
                let AppEffect::Exported { path, bytes } = effect else {
                    unreachable!("export action must produce an export effect");
                };
                self.set_success(self.language.exported(bytes, &path));
                self.clear_message("export");
            }
            Err(error) => self.push_error(
                error.code,
                self.language.operation_failed(
                    self.language.text(Text::ExportOperation),
                    &error.diagnostics.join("；"),
                ),
            ),
        }
    }

    pub(super) fn export_png(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title(self.language.text(Text::ExportFigureDialog))
            .add_filter("PNG", &["png"])
            .set_file_name("instplot-studio-figure.png")
            .save_file()
        else {
            return;
        };
        let preferences = self.document.export_preferences();
        let state = AppTransactionState {
            document: &self.document,
            session: &self.session,
            workspace: &self.workspace,
            edit_history: &self.edit_history,
        };
        match ApplicationController::execute(
            state,
            AppAction::ExportFigure {
                path,
                format: FigureExport::Png {
                    dpi: preferences.selected_raster_dpi,
                    transparent_background: preferences.transparent_background,
                },
            },
        ) {
            Ok(outcome) => {
                let effect = self.commit_app_outcome(outcome);
                let AppEffect::Exported { path, bytes } = effect else {
                    unreachable!("export action must produce an export effect");
                };
                self.set_success(self.language.exported(bytes, &path));
                self.clear_message("export");
            }
            Err(error) => self.push_error(
                error.code,
                self.language.operation_failed(
                    self.language.text(Text::ExportOperation),
                    &error.diagnostics.join("；"),
                ),
            ),
        }
    }

    pub(super) fn export_svg(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title(self.language.text(Text::ExportFigureDialog))
            .add_filter("SVG", &["svg"])
            .set_file_name("instplot-studio-figure.svg")
            .save_file()
        else {
            return;
        };
        let state = AppTransactionState {
            document: &self.document,
            session: &self.session,
            workspace: &self.workspace,
            edit_history: &self.edit_history,
        };
        match ApplicationController::execute(
            state,
            AppAction::ExportFigure {
                path,
                format: FigureExport::Svg,
            },
        ) {
            Ok(outcome) => {
                let effect = self.commit_app_outcome(outcome);
                let AppEffect::Exported { path, bytes } = effect else {
                    unreachable!("export action must produce an export effect");
                };
                self.set_success(self.language.exported(bytes, &path));
                self.clear_message("export");
            }
            Err(error) => self.push_error(
                error.code,
                self.language.operation_failed(
                    self.language.text(Text::ExportOperation),
                    &error.diagnostics.join("；"),
                ),
            ),
        }
    }

    pub(super) fn export_dialog(&mut self, context: &egui::Context) {
        let Some(kind) = self.pending_export else {
            return;
        };
        let mut open = true;
        let mut close_requested = false;
        let mut confirm = false;
        let mut preferences = self.document.export_preferences().clone();
        let before = preferences.clone();
        let width_mm = f64::from(self.resolved.display.width) * 25.4 / 72.0;
        let height_mm = f64::from(self.resolved.display.height) * 25.4 / 72.0;
        let title = self.language.text(Text::ExportSettings);
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .min_width(340.0)
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .frame(studio_card_frame(context.theme() == egui::Theme::Dark))
            .show(context, |ui| {
                ui.horizontal(|ui| {
                    ui.strong(title);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        close_requested = ui.button("×").clicked();
                    });
                });
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);
                ui.label(format!(
                    "{}: {}",
                    self.language.text(Text::Format),
                    match kind {
                        ExportKind::Pdf => "PDF",
                        ExportKind::Png => "PNG",
                        ExportKind::Svg => "SVG",
                    }
                ));
                ui.label(format!(
                    "{}: {width_mm:.2} × {height_mm:.2} mm",
                    self.language.text(Text::PhysicalSize)
                ));
                if kind == ExportKind::Png {
                    egui::ComboBox::from_label(self.language.text(Text::RasterDpi))
                        .selected_text(preferences.selected_raster_dpi.to_string())
                        .show_ui(ui, |ui| {
                            for dpi in preferences.raster_dpi.clone() {
                                ui.selectable_value(
                                    &mut preferences.selected_raster_dpi,
                                    dpi,
                                    dpi.to_string(),
                                );
                            }
                        });
                    let pixel_width = (width_mm / 25.4 * f64::from(preferences.selected_raster_dpi))
                        .round() as u32;
                    let pixel_height = (height_mm / 25.4
                        * f64::from(preferences.selected_raster_dpi))
                    .round() as u32;
                    ui.label(format!(
                        "{}: {pixel_width} × {pixel_height} px",
                        self.language.text(Text::PixelDimensions)
                    ));
                    ui.checkbox(
                        &mut preferences.transparent_background,
                        self.language.text(Text::TransparentBackground),
                    );
                } else if kind == ExportKind::Pdf {
                    ui.weak(self.language.text(Text::PdfFontNote));
                }
                ui.separator();
                ui.label(self.language.publication_summary(
                    self.publication_report.error_count(),
                    self.publication_report.warning_count(),
                ));
                if self.publication_report.error_count() > 0 {
                    ui.colored_label(
                        egui::Color32::LIGHT_RED,
                        self.language.text(Text::FixErrorsBeforeExport),
                    );
                }
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            self.publication_report.error_count() == 0,
                            egui::Button::new(self.language.text(Text::ExportNow)),
                        )
                        .clicked()
                    {
                        confirm = true;
                    }
                    if ui.button(self.language.text(Text::Cancel)).clicked() {
                        self.pending_export = None;
                    }
                });
            });
        if preferences != before {
            self.execute_document_edit(
                EditCommand::SetExportPreferences(preferences),
                self.language.text(Text::ExportSettings),
            );
        }
        if confirm {
            self.pending_export = None;
            match kind {
                ExportKind::Pdf => self.export_pdf(),
                ExportKind::Png => self.export_png(),
                ExportKind::Svg => self.export_svg(),
            }
        } else if !open || close_requested {
            self.pending_export = None;
        }
    }

    pub(super) fn apply_ranges(&mut self, ranges: AxisRanges, group: EditGroup) {
        let state = AppTransactionState {
            document: &self.document,
            session: &self.session,
            workspace: &self.workspace,
            edit_history: &self.edit_history,
        };
        match ApplicationController::execute(
            state,
            AppAction::EditDocument {
                command: Box::new(EditCommand::SetAxisRanges(ranges)),
                group: Some(group),
            },
        ) {
            Ok(outcome) => {
                let effect = self.commit_app_outcome(outcome);
                let AppEffect::DocumentEdited { changed } = effect else {
                    unreachable!("axis edit must produce an edit effect");
                };
                if changed {
                    self.set_success(self.language.text(Text::AxesOperation).to_owned());
                    self.clear_message("invalid-axes");
                }
            }
            Err(error) => self.push_error(
                "invalid-axes",
                self.language.operation_failed(
                    self.language.text(Text::AxesOperation),
                    &error.diagnostics.join("；"),
                ),
            ),
        }
    }

    pub(super) fn undo(&mut self) {
        let state = AppTransactionState {
            document: &self.document,
            session: &self.session,
            workspace: &self.workspace,
            edit_history: &self.edit_history,
        };
        if let Ok(outcome) = ApplicationController::execute(state, AppAction::Undo) {
            let effect = self.commit_app_outcome(outcome);
            let AppEffect::HistoryMoved {
                description,
                redo: false,
            } = effect
            else {
                unreachable!("undo action must produce an undo effect");
            };
            self.sync_axis_editors();
            self.set_success(self.language.undo(&description));
        }
    }

    pub(super) fn redo(&mut self) {
        let state = AppTransactionState {
            document: &self.document,
            session: &self.session,
            workspace: &self.workspace,
            edit_history: &self.edit_history,
        };
        if let Ok(outcome) = ApplicationController::execute(state, AppAction::Redo) {
            let effect = self.commit_app_outcome(outcome);
            let AppEffect::HistoryMoved {
                description,
                redo: true,
            } = effect
            else {
                unreachable!("redo action must produce a redo effect");
            };
            self.sync_axis_editors();
            self.set_success(self.language.redo(&description));
        }
    }

    pub(super) fn request_replacement(&mut self, action: PendingAction) {
        if self.edit_history.is_dirty(&self.document) {
            self.pending_action = Some(action);
        } else {
            self.perform_action(action);
        }
    }

    pub(super) fn perform_action(&mut self, action: PendingAction) {
        self.pending_action = None;
        match action {
            PendingAction::NewProject => self.new_project(),
            PendingAction::OpenLiteHandoff => self.open_lite_handoff(),
            PendingAction::OpenProject => self.open_project(),
            PendingAction::OpenProjectPath(path) => self.open_project_path(path),
            PendingAction::Exit => {
                // The close command is issued by the confirmation dialog, which has the context.
            }
        }
    }

    pub(super) fn unsaved_dialog(&mut self, context: &egui::Context) {
        let Some(action) = self.pending_action.clone() else {
            return;
        };
        egui::Window::new(self.language.text(Text::UnsavedChanges))
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .min_width(340.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .frame(studio_card_frame(context.theme() == egui::Theme::Dark))
            .show(context, |ui| {
                studio_dialog_heading(ui, self.language.text(Text::UnsavedChanges));
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
                            self.perform_action(action.clone());
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

    pub(super) fn set_success(&mut self, text: String) {
        self.status = Some((text, Instant::now()));
    }

    pub(super) fn push_warning(&mut self, code: impl Into<String>, text: String) {
        self.push_message(code, MessageLevel::Warning, text);
    }

    pub(super) fn push_error(&mut self, code: impl Into<String>, text: String) {
        self.push_message(code, MessageLevel::Error, text);
    }

    pub(super) fn push_message(
        &mut self,
        code: impl Into<String>,
        level: MessageLevel,
        text: String,
    ) {
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

    pub(super) fn clear_message(&mut self, code: &str) {
        self.messages.retain(|message| message.code != code);
    }

    pub(super) fn commit_app_outcome(&mut self, outcome: AppOutcome) -> AppEffect {
        self.document = outcome.document;
        self.session = outcome.session;
        self.workspace = outcome.workspace;
        self.edit_history = outcome.edit_history;
        self.resolved = outcome.resolved;
        self.publication_report = outcome.publication_report;
        outcome.effect
    }

    pub(super) fn execute_document_edit(&mut self, command: EditCommand, success: &str) -> bool {
        self.execute_document_edit_with_group(command, success, None)
    }

    pub(super) fn execute_document_edit_with_group(
        &mut self,
        command: EditCommand,
        success: &str,
        group: Option<EditGroup>,
    ) -> bool {
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
                group,
            },
        ) {
            Ok(outcome) => {
                let effect = self.commit_app_outcome(outcome);
                let AppEffect::DocumentEdited { changed } = effect else {
                    unreachable!("edit action must produce an edit effect");
                };
                if changed {
                    self.set_success(success.to_owned());
                    self.clear_message("edit");
                    true
                } else {
                    false
                }
            }
            Err(error) => {
                self.push_error(error.code, error.diagnostics.join("；"));
                false
            }
        }
    }

    pub(super) fn figure_size_editor(&mut self, ui: &mut egui::Ui) {
        ui.label(self.language.text(Text::FigureSize));
        let (mut width, mut height) = self.document.figure_size_mm();
        let mut changed_group = None;
        let mut finish = false;
        ui.horizontal_wrapped(|ui| {
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
        let output_width = f64::from(self.resolved.display.width) * 25.4 / 72.0;
        let output_height = f64::from(self.resolved.display.height) * 25.4 / 72.0;
        if (output_width - width).abs() > 0.01 || (output_height - height).abs() > 0.01 {
            ui.weak(format!(
                "{}: {output_width:.2} × {output_height:.2} mm",
                self.language.text(Text::OutputCanvasSize)
            ));
        }
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

    pub(super) fn fit_canvas(&mut self, available: egui::Vec2) {
        let padding = egui::vec2(64.0, 96.0);
        let available = (available - padding).max(egui::vec2(1.0, 1.0));
        self.canvas_zoom = (available.x / self.resolved.display.width)
            .min(available.y / self.resolved.display.height)
            .clamp(0.1, 3.0);
        self.canvas_scroll = egui::Vec2::ZERO;
    }

    pub(super) fn axis_ranges_editor(&mut self, ui: &mut egui::Ui) {
        ui.label(self.language.text(Text::AxesRanges));
        let mut ranges = self.document.axis_ranges();
        let mut changed_group = None;
        let mut finish_coalescing = false;
        egui::Grid::new("axis_ranges")
            .num_columns(4)
            .show(ui, |ui| {
                ui.label(self.language.text(Text::XMin));
                let edit = deferred_f64_editor(
                    ui,
                    &mut self.numeric_inputs,
                    "axis-range-x-min",
                    &mut ranges.x_min,
                    f64::NEG_INFINITY..=f64::INFINITY,
                    78.0,
                );
                if edit.changed {
                    changed_group = Some(EditGroup::AxisXMinimum);
                }
                finish_coalescing |= edit.finish;
                ui.label(self.language.text(Text::XMax));
                let edit = deferred_f64_editor(
                    ui,
                    &mut self.numeric_inputs,
                    "axis-range-x-max",
                    &mut ranges.x_max,
                    f64::NEG_INFINITY..=f64::INFINITY,
                    78.0,
                );
                if edit.changed {
                    changed_group = Some(EditGroup::AxisXMaximum);
                }
                finish_coalescing |= edit.finish;
                ui.end_row();
                ui.label(self.language.text(Text::YMin));
                let edit = deferred_f64_editor(
                    ui,
                    &mut self.numeric_inputs,
                    "axis-range-y-min",
                    &mut ranges.y_min,
                    f64::NEG_INFINITY..=f64::INFINITY,
                    78.0,
                );
                if edit.changed {
                    changed_group = Some(EditGroup::AxisYMinimum);
                }
                finish_coalescing |= edit.finish;
                ui.label(self.language.text(Text::YMax));
                let edit = deferred_f64_editor(
                    ui,
                    &mut self.numeric_inputs,
                    "axis-range-y-max",
                    &mut ranges.y_max,
                    f64::NEG_INFINITY..=f64::INFINITY,
                    78.0,
                );
                if edit.changed {
                    changed_group = Some(EditGroup::AxisYMaximum);
                }
                finish_coalescing |= edit.finish;
                ui.end_row();
            });
        if let Some(group) = changed_group {
            self.apply_ranges(ranges, group);
        }
        if finish_coalescing {
            self.edit_history.finish_coalescing();
        }
    }

    #[allow(dead_code)]
    pub(super) fn axis_editor(&mut self, ui: &mut egui::Ui, dimension: AxisDimension) {
        let title = match dimension {
            AxisDimension::X => self.language.text(Text::XAxis),
            AxisDimension::Y => self.language.text(Text::YAxis),
        };
        let mut record = self.document.axis_record(dimension);
        let mut changed = false;
        let mut continuous_change = false;
        let mut finish_coalescing = false;
        let mut fixed_ticks = match dimension {
            AxisDimension::X => self.x_fixed_ticks.clone(),
            AxisDimension::Y => self.y_fixed_ticks.clone(),
        };
        let mut fixed_tick_error = None;
        egui::CollapsingHeader::new(title)
            .default_open(true)
            .show(ui, |ui| {
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
                if record.scale == AxisScale::Log10
                    && matches!(record.locator, LocatorSpec::Interval { .. })
                {
                    record.locator = LocatorSpec::Auto { target_count: 6 };
                    changed = true;
                }
                if record.scale == AxisScale::Log10 && record.minor_interval.take().is_some() {
                    changed = true;
                }

                let locator_mode = match record.locator {
                    LocatorSpec::Auto { .. } => 0,
                    LocatorSpec::Interval { .. } => 1,
                    LocatorSpec::Fixed { .. } => 2,
                };
                ui.label(self.language.text(Text::Locator));
                egui::ComboBox::from_id_salt(("locator", dimension))
                    .selected_text(match locator_mode {
                        0 => self.language.text(Text::Auto),
                        1 => self.language.text(Text::Interval),
                        _ => self.language.text(Text::Fixed),
                    })
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(locator_mode == 0, self.language.text(Text::Auto))
                            .clicked()
                        {
                            record.locator = LocatorSpec::Auto { target_count: 6 };
                            changed = true;
                        }
                        if record.scale == AxisScale::Linear
                            && ui
                                .selectable_label(
                                    locator_mode == 1,
                                    self.language.text(Text::Interval),
                                )
                                .clicked()
                        {
                            record.locator = LocatorSpec::Interval {
                                step: (record.maximum - record.minimum) / 5.0,
                            };
                            changed = true;
                        }
                        if ui
                            .selectable_label(locator_mode == 2, self.language.text(Text::Fixed))
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
                        let response =
                            ui.add(egui::DragValue::new(target_count).range(2..=20).prefix(
                                format!("{}: ", self.language.text(Text::TargetTickCount)),
                            ));
                        changed |= response.changed();
                        continuous_change |= response.changed();
                        finish_coalescing |= response.drag_stopped() || response.lost_focus();
                    }
                    LocatorSpec::Interval { step } => {
                        ui.label(self.language.text(Text::MajorTickInterval));
                        let edit = deferred_f64_editor(
                            ui,
                            &mut self.numeric_inputs,
                            format!("axis-{dimension:?}-major-interval"),
                            step,
                            f64::MIN_POSITIVE..=f64::INFINITY,
                            132.0,
                        );
                        changed |= edit.changed;
                        finish_coalescing |= edit.finish;
                    }
                    LocatorSpec::Fixed { values } => {
                        ui.label(self.language.text(Text::FixedTickValues));
                        let before = fixed_ticks.clone();
                        let mut output = egui::TextEdit::singleline(&mut fixed_ticks)
                            .id_salt(("fixed-ticks", dimension))
                            .show(ui);
                        text_input::auto_pair_brackets(
                            ui,
                            &before,
                            &mut fixed_ticks,
                            &mut output,
                            text_input::BracketMode::Literal,
                        );
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
                let minor_edit = minor_interval_editor(
                    ui,
                    self.language,
                    match dimension {
                        AxisDimension::X => AxisIdentity::X1,
                        AxisDimension::Y => AxisIdentity::Y1,
                    },
                    &mut record,
                    &mut self.numeric_inputs,
                );
                changed |= minor_edit.changed;
                continuous_change |= minor_edit.continuous;
                finish_coalescing |= minor_edit.finish;

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
                            .selectable_label(
                                formatter_kind == 1,
                                self.language.text(Text::Decimal),
                            )
                            .clicked()
                        {
                            record.formatter = FormatterSpec::Decimal { precision: 2 };
                            changed = true;
                        }
                        if ui
                            .selectable_label(
                                formatter_kind == 2,
                                self.language.text(Text::Scientific),
                            )
                            .clicked()
                        {
                            record.formatter = FormatterSpec::Scientific { precision: 2 };
                            changed = true;
                        }
                    });
                if let FormatterSpec::Decimal { precision }
                | FormatterSpec::Scientific { precision } = &mut record.formatter
                {
                    let response = ui.add(
                        egui::DragValue::new(precision)
                            .range(0..=15)
                            .prefix(format!("{}: ", self.language.text(Text::Precision))),
                    );
                    changed |= response.changed();
                    continuous_change |= response.changed();
                    finish_coalescing |= response.drag_stopped() || response.lost_focus();
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
            });
        match dimension {
            AxisDimension::X => self.x_fixed_ticks = fixed_ticks,
            AxisDimension::Y => self.y_fixed_ticks = fixed_ticks,
        }
        if let Some(error) = fixed_tick_error {
            self.push_error("fixed-ticks", error);
        }
        if changed {
            let group = continuous_change.then_some(match dimension {
                AxisDimension::X => EditGroup::AxisXSettings,
                AxisDimension::Y => EditGroup::AxisYSettings,
            });
            self.execute_document_edit_with_group(
                EditCommand::SetAxisRecord { dimension, record },
                title,
                group,
            );
        }
        if finish_coalescing {
            self.edit_history.finish_coalescing();
        }
        self.axis_label_editor(ui, dimension);
    }

    pub(super) fn axis_quick_editor_by_identity(
        &mut self,
        ui: &mut egui::Ui,
        identity: AxisIdentity,
    ) {
        let Some(mut record) = self.document.axis_record_by_identity(identity) else {
            return;
        };
        let dimension = axis_dimension(identity);
        let title = axis_title(self.language, identity);
        ui.strong(self.language.text(Text::AxesRanges));
        ui.add_space(2.0);
        let mut changed = ui
            .checkbox(&mut record.autoscale, self.language.text(Text::Autoscale))
            .changed();
        let mut continuous = false;
        let mut finish = false;
        ui.add_enabled_ui(!record.autoscale, |ui| {
            ui.horizontal(|ui| {
                ui.label(self.language.text(Text::Minimum));
                let minimum = deferred_f64_editor(
                    ui,
                    &mut self.numeric_inputs,
                    format!("quick-axis-{identity:?}-minimum"),
                    &mut record.minimum,
                    f64::NEG_INFINITY..=f64::INFINITY,
                    92.0,
                );
                changed |= minimum.changed;
                finish |= minimum.finish;
                ui.label(self.language.text(Text::Maximum));
                let maximum = deferred_f64_editor(
                    ui,
                    &mut self.numeric_inputs,
                    format!("quick-axis-{identity:?}-maximum"),
                    &mut record.maximum,
                    f64::NEG_INFINITY..=f64::INFINITY,
                    92.0,
                );
                changed |= maximum.changed;
                finish |= maximum.finish;
            });
        });
        if record.scale == AxisScale::Linear {
            ui.add_space(6.0);
            ui.separator();
            ui.add_space(4.0);
            let mode = match record.locator {
                LocatorSpec::Auto { .. } => 0,
                LocatorSpec::Interval { .. } => 1,
                LocatorSpec::Fixed { .. } => 2,
            };
            ui.horizontal_wrapped(|ui| {
                ui.strong(self.language.text(Text::MajorTicks));
                egui::ComboBox::from_id_salt(("quick-locator", identity))
                    .selected_text(match mode {
                        0 => self.language.text(Text::Auto),
                        1 => self.language.text(Text::Interval),
                        _ => self.language.text(Text::Fixed),
                    })
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(mode == 0, self.language.text(Text::Auto))
                            .clicked()
                        {
                            record.locator = LocatorSpec::Auto { target_count: 6 };
                            changed = true;
                        }
                        if ui
                            .selectable_label(mode == 1, self.language.text(Text::Interval))
                            .clicked()
                        {
                            record.locator = LocatorSpec::Interval {
                                step: (record.maximum - record.minimum) / 5.0,
                            };
                            changed = true;
                        }
                    });
                if let LocatorSpec::Interval { step } = &mut record.locator {
                    ui.label(self.language.text(Text::MajorTickInterval));
                    let edit = deferred_f64_editor(
                        ui,
                        &mut self.numeric_inputs,
                        format!("quick-axis-{identity:?}-major-interval"),
                        step,
                        f64::MIN_POSITIVE..=f64::INFINITY,
                        112.0,
                    );
                    changed |= edit.changed;
                    finish |= edit.finish;
                }
            });
            ui.add_space(5.0);
            let minor_edit = minor_interval_editor(
                ui,
                self.language,
                identity,
                &mut record,
                &mut self.numeric_inputs,
            );
            changed |= minor_edit.changed;
            continuous |= minor_edit.continuous;
            finish |= minor_edit.finish;
        }
        let mode = self.document.axis_mode();
        let spine_color_visible = matches!(
            (mode, identity),
            (AxisMode::DualX, AxisIdentity::X1 | AxisIdentity::X2)
                | (AxisMode::DualY, AxisIdentity::Y1 | AxisIdentity::Y2)
        );
        if spine_color_visible {
            ui.add_space(6.0);
            ui.separator();
            ui.strong(match identity {
                AxisIdentity::X1 => "下方 X1 spine",
                AxisIdentity::X2 => "上方 X2 spine",
                AxisIdentity::Y1 => "左侧 Y1 spine",
                AxisIdentity::Y2 => "右侧 Y2 spine",
            });
            let palette = self.document.palette_colors().to_vec();
            changed |= color_editor(
                ui,
                self.language,
                &format!("axis-spine-{identity:?}"),
                &mut record.appearance.spine_color_id,
                &palette,
            );
        }
        if changed {
            self.execute_document_edit_with_group(
                EditCommand::SetAxisRecordByIdentity { identity, record },
                &title,
                continuous.then_some(match dimension {
                    AxisDimension::X => EditGroup::AxisXSettings,
                    AxisDimension::Y => EditGroup::AxisYSettings,
                }),
            );
        }
        if finish {
            self.edit_history.finish_coalescing();
        }
    }

    pub(super) fn axis_label_editor(&mut self, ui: &mut egui::Ui, dimension: AxisDimension) {
        let identity = match dimension {
            AxisDimension::X => AxisIdentity::X1,
            AxisDimension::Y => AxisIdentity::Y1,
        };
        self.axis_label_editor_by_identity(ui, identity);
    }

    pub(super) fn axis_label_editor_by_identity(
        &mut self,
        ui: &mut egui::Ui,
        identity: AxisIdentity,
    ) {
        let current = self.document.axis_label_by_identity(identity).to_vec();
        let key = match identity {
            AxisIdentity::X1 => "axis-label-x1",
            AxisIdentity::X2 => "axis-label-x2",
            AxisIdentity::Y1 => "axis-label-y1",
            AxisIdentity::Y2 => "axis-label-y2",
        };
        let initial = label_input::format(&current)
            .or_else(|| {
                self.label_inputs
                    .get(key)
                    .filter(|state| state.source_nodes == current)
                    .map(|state| state.text.clone())
            })
            .unwrap_or_else(|| label_input::display_text(&current));
        self.label_input_editor(ui, key, current, initial, LabelInputTarget::Axis(identity));
    }

    pub(super) fn sync_axis_editors(&mut self) {
        self.x_fixed_ticks = fixed_ticks_text(&self.document.axis_record(AxisDimension::X));
        self.y_fixed_ticks = fixed_ticks_text(&self.document.axis_record(AxisDimension::Y));
    }

    pub(super) fn select_dataset(&mut self, data_source_id: &str) {
        self.selected_dataset = Some(data_source_id.to_owned());
        let previous_x = self.binding_x.clone();
        let previous_y = self.binding_y.clone();
        if let Some(dataset) = self
            .session
            .datasets()
            .iter()
            .find(|dataset| dataset.plot_id == data_source_id)
        {
            let preferred = if previous_x.is_empty() && previous_y.is_empty() {
                dataset
                    .fit_link
                    .as_ref()
                    .map(|fit| (fit.source_x_column.as_str(), fit.source_y_column.as_str()))
            } else {
                Some((previous_x.as_str(), previous_y.as_str()))
            };
            let (x, y) = preferred_dataset_columns(dataset, preferred).unwrap_or_default();
            self.binding_x = x;
            self.binding_y = y;
            self.binding_error = dataset
                .columns
                .iter()
                .find(|column| column.name != self.binding_x && column.name != self.binding_y)
                .map(|column| column.name.clone())
                .unwrap_or_default();
        }
    }

    pub(super) fn select_series_for_editing(&mut self, series: &SeriesDescriptor) {
        self.selected_series = Some(series.id.clone());
        self.selected_canvas_node = Some(series.id.clone());
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

    pub(super) fn open_context_editor(&mut self, target: CanvasHit) {
        let target = self.canonical_context_editor_target(target);
        if !self.context_editor_targets.contains(&target) {
            self.context_editor_targets.push(target.clone());
        }
        self.context_editor_focus_target = Some(target);
    }

    fn canonical_context_editor_target(&self, mut target: CanvasHit) -> CanvasHit {
        if let Some(record) = self.document.artist_record(&target.project_id) {
            target.data_index = None;
            target.role = match record.properties {
                ArtistProperties::MeasurementArrow { .. } => SelectableRole::MeasurementArrow,
                ArtistProperties::Annotation { .. } => SelectableRole::Annotation,
                ArtistProperties::ReferenceLine { .. } => SelectableRole::ReferenceLine,
                ArtistProperties::Legend { .. } => SelectableRole::Legend,
                _ => target.role,
            };
        }
        target
    }

    pub(super) fn request_palette_window(&mut self) {
        self.show_palette = true;
        self.focus_palette = true;
    }

    pub(super) fn request_publication_window(&mut self) {
        self.show_inspector = true;
        self.focus_inspector = true;
    }

    pub(super) fn annotation_artist_for_label(&self, label_id: &str) -> Option<String> {
        self.document
            .project()
            .figure
            .artists
            .iter()
            .find_map(|artist| match &artist.properties {
                ArtistProperties::Annotation {
                    label_id: candidate,
                    ..
                } if candidate == label_id => Some(artist.id.clone()),
                _ => None,
            })
    }

    pub(super) fn measurement_artist_for_label(&self, label_id: &str) -> Option<String> {
        self.document
            .project()
            .figure
            .artists
            .iter()
            .find_map(|artist| match &artist.properties {
                ArtistProperties::MeasurementArrow {
                    label_id: Some(candidate),
                    ..
                } if candidate == label_id => Some(artist.id.clone()),
                _ => None,
            })
    }

    pub(super) fn delete_empty_annotation_on_close(&mut self, artist_id: &str) {
        let Some(record) = self.document.artist_record(artist_id) else {
            return;
        };
        let ArtistProperties::Annotation { label_id, .. } = record.properties else {
            return;
        };
        if !self
            .label_inputs
            .get(&label_id)
            .is_some_and(|state| state.text.trim().is_empty())
        {
            return;
        }
        if self.execute_document_edit(
            EditCommand::DeleteSeries {
                artist_id: artist_id.to_owned(),
            },
            self.language.text(Text::Delete),
        ) {
            self.label_inputs.remove(&label_id);
            self.selected_series = None;
            self.selected_canvas_node = None;
            self.selected_canvas_role = None;
            self.context_editor_targets
                .retain(|target| target.project_id != artist_id);
        }
    }

    pub(super) fn add_text_annotation(&mut self) {
        let before = self
            .document
            .series()
            .into_iter()
            .map(|series| series.id)
            .collect::<std::collections::BTreeSet<_>>();
        if self.execute_document_edit(
            EditCommand::AddAnnotation {
                nodes: vec![LabelNode::Text("Text".to_owned())],
            },
            self.language.text(Text::InsertAnnotation),
        ) && let Some(created) =
            self.document.series().into_iter().find(|series| {
                series.kind == SeriesKind::Annotation && !before.contains(&series.id)
            })
        {
            self.selected_series = Some(created.id.clone());
            self.selected_canvas_node = Some(created.id.clone());
            self.selected_canvas_role = Some(SelectableRole::Annotation);
            self.open_context_editor(CanvasHit {
                project_id: created.id,
                role: SelectableRole::Annotation,
                data_index: None,
            });
        }
    }

    pub(super) fn manual_data_window(&mut self, context: &egui::Context) {
        if !self.manual_data.open {
            return;
        }
        let mut submit = false;
        let mut cancel = false;
        let title = self.language.text(Text::EnterData);
        let embedded_id = egui::Id::new("manual-data-window");
        let viewport_id = egui::ViewportId::from_hash_of("manual-data-viewport");
        if std::mem::take(&mut self.focus_manual_data) {
            instplot_ui::ToolWindowPolicy::raise(context, viewport_id, embedded_id);
        }
        let window_spec = instplot_ui::ToolWindowSpec::new([760.0, 640.0], [560.0, 420.0]);
        if instplot_ui::ToolWindowPolicy::mode(context) == instplot_ui::ToolWindowMode::Embedded {
            let mut open = true;
            window_spec
                .embedded(title, embedded_id)
                .open(&mut open)
                .frame(studio_card_frame(context.theme() == egui::Theme::Dark))
                .show(context, |ui| {
                    self.manual_data_fields(ui, &mut submit, &mut cancel)
                });
            if !open {
                cancel = true;
            }
        } else {
            let builder = window_spec.viewport(title);
            let close_requested =
                context.show_viewport_immediate(viewport_id, builder, |ui, _class| {
                    let child_context = ui.ctx().clone();
                    let close_requested = instplot_ui::viewport_close_requested(&child_context);
                    egui::CentralPanel::default()
                        .frame(
                            egui::Frame::new()
                                .fill(studio_surface(child_context.theme() == egui::Theme::Dark))
                                .inner_margin(egui::Margin::same(16)),
                        )
                        .show(ui, |ui| {
                            self.manual_data_fields(ui, &mut submit, &mut cancel)
                        });
                    close_requested
                });
            cancel |= close_requested;
        }
        if cancel {
            self.manual_data.open = false;
        }
        if submit {
            match self.insert_manual_data() {
                Ok(()) => {
                    self.manual_data.open = false;
                    self.manual_data.error = None;
                    self.manual_data.input = ManualDataInput::default();
                }
                Err(error) => self.manual_data.error = Some(error),
            }
        }
    }

    pub(super) fn prepare_manual_data_window(&mut self) {
        if self.manual_data.open {
            self.focus_manual_data = true;
            return;
        }
        let groups = self
            .document
            .project()
            .data_sources
            .iter()
            .filter_map(|source| source.manual_recipe.as_ref())
            .map(ManualDataGroupInput::from_recipe)
            .collect::<Vec<_>>();
        self.manual_data.input = if groups.is_empty() {
            ManualDataInput::default()
        } else {
            ManualDataInput { groups }
        };
        self.manual_data.editing_group_id = None;
        self.manual_data.error = None;
        self.manual_data.open = true;
        self.focus_manual_data = true;
    }

    pub(super) fn manual_data_fields(
        &mut self,
        ui: &mut egui::Ui,
        submit: &mut bool,
        cancel: &mut bool,
    ) {
        ui.weak("每个框只粘贴一列数值；支持换行、Tab、空格、逗号和分号，不要包含表头。增加重复测量后会自动计算均值和误差棒。");
        ui.separator();

        let data_area_height = (ui.available_height() - 86.0).max(180.0);
        let saved_ids = self
            .document
            .project()
            .data_sources
            .iter()
            .filter(|source| source.manual_recipe.is_some())
            .map(|source| source.id.clone())
            .collect::<BTreeSet<_>>();
        let mut edit_group = None;
        egui::ScrollArea::vertical()
            .max_height(data_area_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let group_count = self.manual_data.input.groups.len();
                let mut remove_group = None;
                for (index, group) in self.manual_data.input.groups.iter_mut().enumerate() {
                    let saved = saved_ids.contains(&group.group_id);
                    let editable = !saved
                        || self.manual_data.editing_group_id.as_deref()
                            == Some(group.group_id.as_str());
                    let (edit, remove) =
                        manual_group_input_card(ui, index, group, saved, editable, group_count > 1);
                    if edit {
                        edit_group = Some(group.group_id.clone());
                    }
                    if remove {
                        remove_group = Some((
                            index,
                            group.group_id.clone(),
                            group.source_name.clone(),
                            saved,
                        ));
                    }
                }
                if let Some((index, group_id, label, saved)) = remove_group {
                    if saved {
                        self.pending_data_removal = Some(DataRemovalRequest::File {
                            label,
                            ids: vec![group_id],
                        });
                    } else {
                        self.manual_data.input.groups.remove(index);
                    }
                }
                if ui.button("＋ 新增数据组").clicked() {
                    let next = self
                        .document
                        .project()
                        .data_sources
                        .iter()
                        .filter(|source| {
                            source.origin != instplot_studio::DataSourceOrigin::Imported
                        })
                        .count()
                        + self.manual_data.input.groups.len()
                        + 1;
                    self.manual_data
                        .input
                        .groups
                        .push(ManualDataGroupInput::new(next));
                }
            });
        if let Some(group_id) = edit_group {
            self.prepare_manual_data_window();
            self.manual_data.editing_group_id = Some(group_id);
        }

        if let Some(error) = &self.manual_data.error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }
        ui.separator();
        ui.horizontal(|ui| {
            let action = if self.manual_data.editing_group_id.is_some() {
                "应用修改"
            } else {
                "绘图"
            };
            *submit |= ui.button(action).clicked();
            *cancel |= ui.button("取消").clicked();
        });
    }

    pub(super) fn insert_manual_data(&mut self) -> Result<(), String> {
        let parsed = DataImporter::import_manual(&self.manual_data.input)
            .map_err(|diagnostic| diagnostic.to_string())?;
        let existing_ids = self
            .document
            .project()
            .data_sources
            .iter()
            .map(|source| source.id.clone())
            .collect::<BTreeSet<_>>();
        let parsed_groups = parsed
            .groups
            .into_iter()
            .filter(|group| {
                self.manual_data.editing_group_id.as_deref() == Some(group.dataset.plot_id.as_str())
                    || !existing_ids.contains(&group.dataset.plot_id)
            })
            .collect::<Vec<_>>();
        if parsed_groups.is_empty() {
            return Err("没有新增或正在编辑的数据组".to_owned());
        }
        let replace_showcase = self.workspace.should_replace_showcase_on_import();
        let mut datasets = if replace_showcase {
            Vec::new()
        } else {
            self.session.datasets().to_vec()
        };
        for group in &parsed_groups {
            if let Some(position) = datasets
                .iter()
                .position(|item| item.plot_id == group.dataset.plot_id)
            {
                if self.manual_data.editing_group_id.as_deref()
                    != Some(group.dataset.plot_id.as_str())
                {
                    return Err(format!("数据组“{}”已经存在于当前图中", group.recipe.name));
                }
                datasets[position] = group.dataset.clone();
            } else {
                datasets.push(group.dataset.clone());
            }
        }

        let mut document = if replace_showcase {
            let mut document =
                FigureDocument::from_datasets(&datasets).map_err(|error| error.to_string())?;
            if USER_PALETTE_IDS.contains(&self.document.palette_id()) {
                document.set_palette(self.document.palette_id())?;
            }
            let automatic = document
                .series()
                .into_iter()
                .filter(|series| {
                    matches!(
                        series.kind,
                        SeriesKind::Line | SeriesKind::Scatter | SeriesKind::ErrorBar
                    )
                })
                .map(|series| series.id)
                .collect::<Vec<_>>();
            for id in automatic {
                document.delete_series(&id)?;
            }
            document
        } else {
            let mut document = self.document.clone();
            document
                .sync_datasets_without_autoscale(&datasets)
                .map_err(|error| error.to_string())?;
            document
        };
        let series_offset = document
            .series()
            .into_iter()
            .filter(|series| matches!(series.kind, SeriesKind::Line | SeriesKind::Scatter))
            .filter_map(|series| {
                series
                    .binding
                    .map(|binding| (binding.data_source_id, binding.x_column, binding.y_column))
            })
            .collect::<BTreeSet<_>>()
            .len();
        for (series_index, group) in parsed_groups.iter().enumerate() {
            let dataset = &group.dataset;
            let specification = &group.series;
            let style = match group.recipe.plot_style {
                ManualPlotStyle::Line => SeriesCreationStyle::Line,
                ManualPlotStyle::Scatter => SeriesCreationStyle::Scatter,
                ManualPlotStyle::LineAndMarker => SeriesCreationStyle::LineAndMarker,
            };
            let marker_size = match (style, dataset.row_count) {
                (SeriesCreationStyle::Scatter, 0..=10) => 5.0,
                (SeriesCreationStyle::Scatter, 11..=250) => 4.0,
                (SeriesCreationStyle::Scatter, _) => 3.0,
                (SeriesCreationStyle::LineAndMarker, 0..=10) => 4.5,
                (SeriesCreationStyle::LineAndMarker, 11..=1_000) => 3.5,
                (SeriesCreationStyle::LineAndMarker, _) => 2.5,
                (SeriesCreationStyle::Line, _) => 0.0,
            };
            let style_index = series_offset + series_index;
            let existing_series = document
                .series()
                .into_iter()
                .filter(|series| {
                    series.binding.as_ref().is_some_and(|binding| {
                        binding.data_source_id == dataset.plot_id
                            && matches!(series.kind, SeriesKind::Line | SeriesKind::Scatter)
                    })
                })
                .collect::<Vec<_>>();
            if let Some(primary) = existing_series.first() {
                for series in &existing_series {
                    document.rebind_series(
                        &series.id,
                        &dataset.plot_id,
                        &specification.x_column,
                        &specification.y_column,
                        None,
                    )?;
                }
                document.set_series_style(&primary.id, style)?;
                document.set_series_legend_label(
                    &primary.id,
                    vec![LabelNode::Text(specification.label.clone())],
                )?;
                document.set_series_error_columns(
                    &primary.id,
                    specification.x_error_column.as_deref(),
                    specification.y_error_column.as_deref(),
                )?;
                document.set_manual_source_metadata(&dataset.plot_id, group.recipe.clone())?;
                continue;
            }
            let created = document.create_series(
                &dataset.plot_id,
                &specification.x_column,
                &specification.y_column,
                style,
            )?;
            for id in &created {
                let Some(mut record) = document.artist_record(id) else {
                    continue;
                };
                match &mut record.properties {
                    ArtistProperties::Scatter { marker, .. } => {
                        marker.size_pt = marker_size;
                        marker.shape =
                            PRODUCT_MARKER_SHAPES[style_index % PRODUCT_MARKER_SHAPES.len()];
                    }
                    ArtistProperties::Line { .. } => {}
                    _ => {}
                }
                document.set_artist_record(record)?;
            }
            document.set_series_legend_label(
                &created[0],
                vec![LabelNode::Text(specification.label.clone())],
            )?;
            if let Some(error_column) = specification.y_error_column.as_deref() {
                document.create_error_bars(
                    &dataset.plot_id,
                    &specification.x_column,
                    &specification.y_column,
                    error_column,
                    specification.x_error_column.as_deref(),
                )?;
            }
            document.set_manual_source_metadata(&dataset.plot_id, group.recipe.clone())?;
        }
        let first = parsed_groups
            .first()
            .ok_or_else(|| "请至少录入一组 XY 数据".to_owned())?;
        document.set_axis_label(
            AxisDimension::X,
            vec![LabelNode::Text(first.series.x_column.clone())],
        )?;
        document.set_axis_label(
            AxisDimension::Y,
            vec![LabelNode::Text(first.series.y_column.clone())],
        )?;
        document.refresh_autoscale()?;
        let state = AppTransactionState {
            document: &self.document,
            session: &self.session,
            workspace: &self.workspace,
            edit_history: &self.edit_history,
        };
        let outcome = ApplicationController::execute(
            state,
            AppAction::CommitPreparedData {
                document: Box::new(document),
                datasets,
                source_path: first.dataset.source.clone(),
                selected_dataset: first.dataset.plot_id.clone(),
            },
        )
        .map_err(|error| error.diagnostics.join("；"))?;
        let effect = self.commit_app_outcome(outcome);
        let AppEffect::PreparedDataCommitted { selected_dataset } = effect else {
            unreachable!("prepared data action must produce a prepared-data effect");
        };
        self.sync_axis_editors();
        self.select_dataset(&selected_dataset);
        self.show_layers = true;
        self.canvas_scroll = egui::Vec2::ZERO;
        self.first_frame = true;
        self.set_success(format!(
            "已绘制 {} 条曲线，最多 {} 个数据点",
            parsed_groups.len(),
            parsed_groups
                .iter()
                .map(|group| group.dataset.row_count)
                .max()
                .unwrap_or(0)
        ));
        self.manual_data.editing_group_id = None;
        Ok(())
    }

    pub(super) fn data_removal_dialog(&mut self, context: &egui::Context) {
        let Some(request) = self.pending_data_removal.clone() else {
            return;
        };
        let ids = request.ids();
        let Ok((source_count, artist_count)) = self.document.data_source_removal_impact(&ids)
        else {
            self.pending_data_removal = None;
            return;
        };
        let (title, label) = match &request {
            DataRemovalRequest::File { label, .. } => {
                (self.language.text(Text::RemoveFileQuestion), label.clone())
            }
            DataRemovalRequest::All(_) => (
                self.language.text(Text::ClearAllDataQuestion),
                self.language.text(Text::ClearAllData).to_owned(),
            ),
        };
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .min_width(340.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .frame(studio_card_frame(context.theme() == egui::Theme::Dark))
            .show(context, |ui| {
                studio_dialog_heading(ui, title);
                ui.label(&label);
                ui.colored_label(
                    egui::Color32::YELLOW,
                    self.language
                        .data_removal_impact(source_count, artist_count),
                );
                ui.horizontal(|ui| {
                    let delete_label = self.language.text(Text::Delete);
                    if ui.button(delete_label).clicked() {
                        let state = AppTransactionState {
                            document: &self.document,
                            session: &self.session,
                            workspace: &self.workspace,
                            edit_history: &self.edit_history,
                        };
                        match ApplicationController::execute(
                            state,
                            AppAction::RemoveDataSources(ids.clone()),
                        ) {
                            Ok(outcome) => {
                                let effect = self.commit_app_outcome(outcome);
                                debug_assert!(matches!(effect, AppEffect::RemovedData));
                                self.selected_dataset = None;
                                self.selected_series = None;
                                self.selected_canvas_node = None;
                                self.selected_canvas_role = None;
                                self.context_editor_targets.clear();
                                self.sync_axis_editors();
                                if self.manual_data.open {
                                    self.prepare_manual_data_window();
                                }
                                self.set_success(title.to_owned());
                                self.clear_message("edit");
                            }
                            Err(error) => self.push_error(
                                error.code,
                                self.language
                                    .operation_failed(title, &error.diagnostics.join("；")),
                            ),
                        }
                        self.pending_data_removal = None;
                    }
                    if ui.button(self.language.text(Text::Cancel)).clicked() {
                        self.pending_data_removal = None;
                    }
                });
            });
    }

    pub(super) fn series_tree(&mut self, ui: &mut egui::Ui) {
        self.data_source_controls(ui);
    }

    pub(super) fn data_source_controls(&mut self, ui: &mut egui::Ui) {
        let mut clear_all = false;
        let can_clear = !self.document.project().data_sources.is_empty()
            && !self.workspace.should_replace_showcase_on_import();
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            if can_clear
                && studio_close_button_sized(
                    ui,
                    self.language.text(Text::ClearAllData),
                    SIDEBAR_FILE_CLOSE_BUTTON_SIZE,
                )
                .clicked()
            {
                clear_all = true;
            }
            let title_width = ui.available_width().max(48.0);
            ui.add_sized(
                [title_width, 34.0],
                egui::Label::new(
                    egui::RichText::new(self.language.text(Text::DataFiles))
                        .size(18.0)
                        .strong(),
                )
                .halign(egui::Align::LEFT),
            );
        });
        if clear_all {
            self.pending_data_removal = Some(DataRemovalRequest::All(
                self.document
                    .project()
                    .data_sources
                    .iter()
                    .map(|source| source.id.clone())
                    .collect(),
            ));
        }
        if self.workspace.should_replace_showcase_on_import() {
            ui.weak(self.language.text(Text::DemoDataHint));
            return;
        }
        let groups = data_navigation_groups(self.session.datasets(), self.language);
        if groups.is_empty() {
            ui.weak(self.language.text(Text::NoData));
            return;
        }
        let mut remove_file = None;
        for group in &groups {
            let ids = group
                .items
                .iter()
                .map(|item| item.id.clone())
                .collect::<Vec<_>>();
            let rows = group.items.iter().map(|item| item.rows).max().unwrap_or(0);
            let columns = group
                .items
                .iter()
                .map(|item| item.columns)
                .max()
                .unwrap_or(0);
            studio_file_card_frame(ui.ctx().theme() == egui::Theme::Dark).show(ui, |ui| {
                // Size every card for the default 260 px sidebar, leaving enough
                // room for panel margins and borders so a horizontal scrollbar is
                // needed only after the user deliberately narrows the sidebar.
                ui.set_width(SIDEBAR_FILE_CARD_INNER_WIDTH);
                ui.horizontal(|ui| {
                    if studio_close_button_sized(
                        ui,
                        self.language.text(Text::RemoveFile),
                        SIDEBAR_FILE_CLOSE_BUTTON_SIZE,
                    )
                    .clicked()
                    {
                        remove_file = Some((group.title.clone(), ids.clone()));
                    }
                    let label_width = ui.available_width().max(48.0);
                    let (label_rect, label_response) =
                        ui.allocate_exact_size(egui::vec2(label_width, 28.0), egui::Sense::hover());
                    ui.put(
                        label_rect,
                        egui::Label::new(egui::RichText::new(&group.title).strong())
                            .truncate()
                            .halign(egui::Align::LEFT),
                    );
                    label_response.on_hover_text(format!(
                        "{} · {}",
                        group.path,
                        self.language.rows_columns(rows, columns)
                    ));
                });
                ui.add_space(3.0);
                self.column_controls_for_group(ui, group);
            });
            ui.add_space(9.0);
        }
        if let Some((label, ids)) = remove_file {
            self.pending_data_removal = Some(DataRemovalRequest::File { label, ids });
        }
    }

    pub(super) fn column_controls_for_group(
        &mut self,
        ui: &mut egui::Ui,
        group: &DataNavigationGroup,
    ) {
        let Some(dataset_id) = group
            .items
            .iter()
            .find(|item| item.kind == DataSetKind::Source)
            .or_else(|| group.items.first())
            .map(|item| item.id.as_str())
        else {
            return;
        };
        let mut series_for_dataset = self
            .document
            .logical_series()
            .into_iter()
            .filter(|series| {
                series
                    .binding
                    .as_ref()
                    .is_some_and(|binding| binding.data_source_id == dataset_id)
            })
            .collect::<Vec<_>>();
        let Some(dataset) = self
            .session
            .datasets()
            .iter()
            .find(|dataset| dataset.plot_id == dataset_id)
            .cloned()
        else {
            return;
        };
        if series_for_dataset.is_empty() {
            return;
        }
        let selected_index = self
            .selected_series
            .as_ref()
            .and_then(|selected| {
                series_for_dataset
                    .iter()
                    .position(|series| &series.id == selected)
            })
            .unwrap_or(0);
        let mut selected_id = series_for_dataset[selected_index].id.clone();
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt(("file-series", &group.key))
                .width(142.0)
                .selected_text(
                    series_for_dataset
                        .iter()
                        .position(|series| series.id == selected_id)
                        .map_or_else(|| "曲线".to_owned(), |index| format!("曲线 {}", index + 1)),
                )
                .show_ui(ui, |ui| {
                    for (index, series) in series_for_dataset.iter().enumerate() {
                        ui.selectable_value(
                            &mut selected_id,
                            series.id.clone(),
                            format!(
                                "曲线 {} · {}",
                                index + 1,
                                canvas_logical_series_title(self.language, &self.document, series,)
                            ),
                        );
                    }
                });
            if ui.button("＋").on_hover_text("从此文件新增曲线").clicked() {
                let used_y = series_for_dataset
                    .iter()
                    .filter_map(|series| {
                        series
                            .binding
                            .as_ref()
                            .map(|binding| binding.y_column.as_str())
                    })
                    .collect::<BTreeSet<_>>();
                let x = series_for_dataset[0]
                    .binding
                    .as_ref()
                    .map(|binding| binding.x_column.clone())
                    .unwrap_or_else(|| dataset.columns[0].name.clone());
                if let Some(y) = dataset
                    .columns
                    .iter()
                    .map(|column| column.name.as_str())
                    .find(|column| *column != x && !used_y.contains(column))
                    .or_else(|| {
                        dataset
                            .columns
                            .iter()
                            .map(|column| column.name.as_str())
                            .find(|column| *column != x)
                    })
                {
                    self.execute_document_edit(
                        EditCommand::CreateSeries {
                            data_source_id: dataset.plot_id.clone(),
                            x_column: x,
                            y_column: y.to_owned(),
                            style: SeriesCreationStyle::Scatter,
                        },
                        "新增曲线",
                    );
                    series_for_dataset = self
                        .document
                        .logical_series()
                        .into_iter()
                        .filter(|series| {
                            series
                                .binding
                                .as_ref()
                                .is_some_and(|binding| binding.data_source_id == dataset_id)
                        })
                        .collect();
                    if let Some(created) = series_for_dataset.last() {
                        selected_id.clone_from(&created.id);
                    }
                }
            }
        });
        self.selected_series = Some(selected_id.clone());
        let Some(series) = series_for_dataset
            .into_iter()
            .find(|series| series.id == selected_id)
        else {
            return;
        };
        let binding = series.binding.as_ref().expect("filtered bound series");
        let mut x_column = binding.x_column.clone();
        let mut y_column = binding.y_column.clone();
        let (mut x_error_column, mut y_error_column) = self
            .document
            .project()
            .figure
            .artists
            .iter()
            .find_map(|artist| match &artist.properties {
                ArtistProperties::ErrorBar {
                    binding: error_binding,
                    x_error_column,
                    y_error_column,
                    ..
                } if error_binding == binding => {
                    Some((x_error_column.clone(), Some(y_error_column.clone())))
                }
                _ => None,
            })
            .unwrap_or((None, None));
        let error_before = (x_error_column.clone(), y_error_column.clone());
        let before = (x_column.clone(), y_column.clone());
        ui.horizontal(|ui| {
            ui.add_sized(
                [22.0, 36.0],
                egui::Label::new("X").halign(egui::Align::LEFT),
            );
            column_combo(
                ui,
                &format!("column-sidebar-x-{}", group.key),
                "",
                &mut x_column,
                &dataset,
            );
        });
        ui.horizontal(|ui| {
            ui.add_sized(
                [22.0, 36.0],
                egui::Label::new("Y").halign(egui::Align::LEFT),
            );
            column_combo(
                ui,
                &format!("column-sidebar-y-{}", group.key),
                "",
                &mut y_column,
                &dataset,
            );
        });
        if before != (x_column.clone(), y_column.clone()) && x_column != y_column {
            self.selected_dataset = Some(binding.data_source_id.clone());
            self.binding_x.clone_from(&x_column);
            self.binding_y.clone_from(&y_column);
            self.execute_document_edit(
                EditCommand::RebindSeries {
                    artist_id: series.id.clone(),
                    data_source_id: binding.data_source_id.clone(),
                    x_column: x_column.clone(),
                    y_column: y_column.clone(),
                    y_error_column: (series.kind == SeriesKind::ErrorBar)
                        .then(|| self.binding_error.clone()),
                },
                self.language.text(Text::ApplyBinding),
            );
        }
        let mut axes = series.axes.unwrap_or_default();
        if axis_binding_editor(
            ui,
            ("sidebar-series-axis-binding", &group.key, &series.id),
            self.document.axis_mode(),
            &mut axes,
        ) {
            self.execute_document_edit(
                EditCommand::SetSeriesAxisBinding {
                    artist_id: series.id.clone(),
                    axes,
                },
                "更改曲线坐标轴",
            );
        }
        egui::CollapsingHeader::new("误差棒")
            .id_salt(("error-columns", &group.key))
            .default_open(y_error_column.is_some())
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add_sized([32.0, 36.0], egui::Label::new("Y ±"));
                    optional_column_combo(
                        ui,
                        &format!("column-sidebar-y-error-{}", group.key),
                        &mut y_error_column,
                        &dataset,
                        [&x_column, &y_column],
                    );
                });
                if y_error_column.is_none() {
                    x_error_column = None;
                }
                ui.add_enabled_ui(y_error_column.is_some(), |ui| {
                    ui.horizontal(|ui| {
                        ui.add_sized([32.0, 36.0], egui::Label::new("X ±"));
                        optional_column_combo(
                            ui,
                            &format!("column-sidebar-x-error-{}", group.key),
                            &mut x_error_column,
                            &dataset,
                            [&x_column, &y_column],
                        );
                    });
                });
            });
        if error_before != (x_error_column.clone(), y_error_column.clone()) {
            self.execute_document_edit(
                EditCommand::SetSeriesErrorColumns {
                    artist_id: series.id.clone(),
                    x_error_column,
                    y_error_column,
                },
                "更改误差列",
            );
        }
    }

    pub(super) fn inspector(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.language.text(Text::PublicationCheck));
        let error_count = self.publication_report.error_count();
        let warning_count = self.publication_report.warning_count();
        if error_count == 0 && warning_count == 0 {
            ui.label("未发现出版规范问题");
            return;
        }
        ui.label(
            self.language
                .publication_summary(error_count, warning_count),
        );
        let findings = self.publication_report.findings.clone();
        for (severity, label, color) in [
            (
                CheckSeverity::Error,
                self.language.text(Text::Error),
                egui::Color32::LIGHT_RED,
            ),
            (
                CheckSeverity::Warning,
                self.language.text(Text::Warning),
                egui::Color32::YELLOW,
            ),
        ] {
            let matching = findings
                .iter()
                .filter(|finding| finding.severity == severity)
                .collect::<Vec<_>>();
            ui.collapsing(format!("{label} ({})", matching.len()), |ui| {
                for finding in matching {
                    ui.group(|ui| {
                        let (rule, reason, impact, remediation) =
                            localized_publication_finding(self.language, finding);
                        let title = finding.node_id.as_ref().map_or_else(
                            || rule.to_owned(),
                            |node| {
                                format!(
                                    "{rule} · {}",
                                    publication_object_name(&self.document, node, self.language)
                                )
                            },
                        );
                        if let Some(node) = &finding.node_id {
                            if ui.button(title).clicked() {
                                self.selected_canvas_node = Some(node.clone());
                                self.selected_canvas_role = None;
                                let series = self
                                    .document
                                    .series()
                                    .into_iter()
                                    .find(|series| series.id == *node);
                                if let Some(series) = series {
                                    self.select_series_for_editing(&series);
                                } else {
                                    self.selected_series = None;
                                }
                            }
                        } else {
                            ui.colored_label(color, title);
                        }
                        ui.colored_label(
                            color,
                            format!("{}: {reason}", self.language.text(Text::Why)),
                        );
                        ui.weak(format!("{}: {}", self.language.text(Text::Impact), impact));
                        ui.weak(format!(
                            "{}: {}",
                            self.language.text(Text::HowToFix),
                            remediation
                        ));
                    });
                }
            });
        }
    }

    pub(super) fn publication_check_window(&mut self, context: &egui::Context) {
        if !self.show_inspector {
            return;
        }
        let title = self.language.text(Text::PublicationCheck);
        let compact_height = if self.publication_report.error_count() == 0
            && self.publication_report.warning_count() == 0
        {
            150.0
        } else {
            360.0
        };
        let embedded_id = egui::Id::new("publication-check-window");
        let viewport_id = egui::ViewportId::from_hash_of("publication-check-viewport");
        if std::mem::take(&mut self.focus_inspector) {
            instplot_ui::ToolWindowPolicy::raise(context, viewport_id, embedded_id);
        }
        let window_spec = instplot_ui::ToolWindowSpec::new([460.0, compact_height], [360.0, 140.0]);
        if instplot_ui::ToolWindowPolicy::mode(context) == instplot_ui::ToolWindowMode::Embedded {
            let mut open = self.show_inspector;
            window_spec
                .embedded(title, embedded_id)
                .open(&mut open)
                .frame(studio_card_frame(context.theme() == egui::Theme::Dark))
                .show(context, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| self.inspector(ui));
                });
            self.show_inspector = open;
            return;
        }
        let builder = window_spec.viewport(title);
        let close_requested =
            context.show_viewport_immediate(viewport_id, builder, |ui, _class| {
                let child_context = ui.ctx().clone();
                let close_requested = instplot_ui::viewport_close_requested(&child_context);
                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::new()
                            .fill(studio_surface(child_context.theme() == egui::Theme::Dark))
                            .inner_margin(egui::Margin::same(16)),
                    )
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| self.inspector(ui));
                    });
                close_requested
            });
        if close_requested {
            self.show_inspector = false;
        }
    }

    pub(super) fn palette_window(&mut self, context: &egui::Context) {
        if !self.show_palette {
            return;
        }
        let title = self.language.text(Text::ColorScheme);
        let embedded_id = egui::Id::new("palette-window");
        let viewport_id = egui::ViewportId::from_hash_of("palette-viewport");
        if std::mem::take(&mut self.focus_palette) {
            instplot_ui::ToolWindowPolicy::raise(context, viewport_id, embedded_id);
        }
        let window_spec = instplot_ui::ToolWindowSpec::new([510.0, 430.0], [420.0, 300.0]);
        let mut requested_palette = None;
        if instplot_ui::ToolWindowPolicy::mode(context) == instplot_ui::ToolWindowMode::Embedded {
            let mut open = self.show_palette;
            window_spec
                .embedded(title, embedded_id)
                .open(&mut open)
                .frame(studio_card_frame(context.theme() == egui::Theme::Dark))
                .show(context, |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, true])
                        .show(ui, |ui| self.palette_fields(ui, &mut requested_palette));
                });
            self.show_palette = open;
        } else {
            let builder = window_spec.viewport(title);
            let close_requested =
                context.show_viewport_immediate(viewport_id, builder, |ui, _class| {
                    let child_context = ui.ctx().clone();
                    let close_requested = instplot_ui::viewport_close_requested(&child_context);
                    egui::CentralPanel::default()
                        .frame(
                            egui::Frame::new()
                                .fill(studio_surface(child_context.theme() == egui::Theme::Dark))
                                .inner_margin(egui::Margin::same(16)),
                        )
                        .show(ui, |ui| {
                            egui::ScrollArea::vertical()
                                .auto_shrink([false, true])
                                .show(ui, |ui| self.palette_fields(ui, &mut requested_palette));
                        });
                    close_requested
                });
            if close_requested {
                self.show_palette = false;
            }
        }
        if let Some(palette_id) = requested_palette {
            let success = match self.language {
                UiLanguage::Chinese => format!(
                    "已应用配色：{}",
                    palette_scheme_name(self.language, palette_id)
                ),
                UiLanguage::English => format!(
                    "Applied palette: {}",
                    palette_scheme_name(self.language, palette_id)
                ),
            };
            self.execute_document_edit(
                EditCommand::SetPalette {
                    palette_id: palette_id.to_owned(),
                },
                &success,
            );
        }
    }

    fn palette_fields(&self, ui: &mut egui::Ui, requested_palette: &mut Option<&'static str>) {
        let active = match self.document.palette_id() {
            "publication-default-v1" | "studio-showcase-v1" => "tol-bright-v1",
            id => id,
        };
        for (group_index, kind) in [
            PaletteKind::Qualitative,
            PaletteKind::Sequential,
            PaletteKind::Diverging,
            PaletteKind::Neutral,
        ]
        .into_iter()
        .enumerate()
        {
            if group_index > 0 {
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);
            }
            ui.label(
                egui::RichText::new(palette_group_name(self.language, kind))
                    .strong()
                    .color(ui.visuals().weak_text_color()),
            );
            ui.add_space(3.0);
            for palette_id in USER_PALETTE_IDS {
                if builtin_palette(palette_id).map(|palette| palette.kind) != Some(kind) {
                    continue;
                }
                ui.horizontal(|ui| {
                    let selected = ui
                        .selectable_label(
                            active == palette_id,
                            palette_scheme_name(self.language, palette_id),
                        )
                        .clicked();
                    if selected {
                        *requested_palette = Some(palette_id);
                    }
                    if let Some(registry) = builtin_palette_registry(palette_id) {
                        for color_id in palette_series_color_ids(palette_id).iter().take(7) {
                            if let Some(color) =
                                registry.colors.iter().find(|color| color.id == *color_id)
                            {
                                let [red, green, blue, _] = color.rgba;
                                ui.colored_label(egui::Color32::from_rgb(red, green, blue), "●");
                            }
                        }
                    }
                });
            }
        }
    }

    pub(super) fn context_editor(&mut self, context: &egui::Context) {
        if self.selection_candidates.len() > 1 {
            let candidates = self.selection_candidates.clone();
            let mut chosen = None;
            let mut open = true;
            egui::Window::new(self.language.text(Text::ChooseObject))
                .id(egui::Id::new("context-object-chooser"))
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .min_width(300.0)
                .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-24.0, 72.0))
                .frame(studio_card_frame(context.theme() == egui::Theme::Dark))
                .show(context, |ui| {
                    for candidate in &candidates {
                        let label = self
                            .document
                            .series()
                            .into_iter()
                            .find(|series| series.id == candidate.project_id)
                            .map_or_else(
                                || {
                                    if let Some(identity) = axis_identity_for_project_id(
                                        &self.document,
                                        &candidate.project_id,
                                    ) {
                                        axis_title(self.language, identity)
                                    } else {
                                        self.language.text(Text::FixedFigure).to_owned()
                                    }
                                },
                                |series| {
                                    canvas_logical_series_title(
                                        self.language,
                                        &self.document,
                                        &series,
                                    )
                                },
                            );
                        if ui.button(label).clicked() {
                            chosen = Some(candidate.clone());
                        }
                    }
                });
            if let Some(chosen) = chosen {
                self.selected_series = self
                    .document
                    .series()
                    .into_iter()
                    .find(|series| series.id == chosen.project_id)
                    .map(|series| series.id);
                if let Some(series) = self
                    .document
                    .series()
                    .into_iter()
                    .find(|series| Some(&series.id) == self.selected_series.as_ref())
                {
                    self.select_series_for_editing(&series);
                }
                self.selected_canvas_node = Some(chosen.project_id.clone());
                self.selected_canvas_role = Some(chosen.role);
                self.open_context_editor(chosen);
                self.selection_candidates.clear();
            } else if !open {
                self.selection_candidates.clear();
            }
        }

        let targets = self.context_editor_targets.clone();
        let mut remaining = Vec::with_capacity(targets.len());
        for (index, target) in targets.into_iter().enumerate() {
            let focus_requested = self.context_editor_focus_target.as_ref() == Some(&target);
            if self.context_editor_window(context, &target, index, focus_requested) {
                remaining.push(target);
            }
        }
        self.context_editor_focus_target = None;
        self.context_editor_targets = remaining;
    }

    pub(super) fn context_editor_window(
        &mut self,
        context: &egui::Context,
        target: &CanvasHit,
        index: usize,
        focus_requested: bool,
    ) -> bool {
        let selected_id = target.project_id.clone();
        let selected_role = Some(target.role);
        let series = self
            .document
            .series()
            .into_iter()
            .find(|series| series.id == selected_id);
        let axes = &self.document.project().figure.axes[0];
        let axes_id = axes.id.clone();
        let selected_axis_identity = axis_identity_for_project_id(&self.document, &selected_id);
        let title = if let Some(series) = &series {
            canvas_logical_series_title(self.language, &self.document, series)
        } else if let Some(identity) = selected_axis_identity {
            format!(
                "{} · {}",
                axis_title(self.language, identity),
                self.language
                    .text(if selected_role == Some(SelectableRole::AxisLabel) {
                        Text::AxisLabel
                    } else {
                        Text::MajorTicks
                    })
            )
        } else if selected_id == axes_id {
            self.language.text(Text::FigureSize).to_owned()
        } else {
            self.language.text(Text::EditSelected).to_owned()
        };
        let window_key = format!("context-object-editor-{selected_id}-{:?}", target.role);
        let viewport_id = egui::ViewportId::from_hash_of(&window_key);
        let embedded_id = egui::Id::new(("fullscreen-context-object-editor", &window_key));
        if focus_requested {
            instplot_ui::ToolWindowPolicy::raise(context, viewport_id, embedded_id);
        }
        let selected_record = self.document.artist_record(&selected_id);
        let editor_width = match &selected_record {
            Some(record) if matches!(record.properties, ArtistProperties::Legend { .. }) => 400.0,
            _ => 470.0,
        };
        let editor_height = if matches!(selected_role, Some(SelectableRole::AxisLabel)) {
            310.0
        } else {
            match selected_record {
                Some(record) => match record.properties {
                    ArtistProperties::Line { .. } | ArtistProperties::Scatter { .. } => 390.0,
                    ArtistProperties::Legend { ref entries, .. } => {
                        (250.0 + entries.len() as f32 * 84.0).min(620.0)
                    }
                    ArtistProperties::Annotation { .. } => 350.0,
                    ArtistProperties::MeasurementArrow { .. } => 520.0,
                    _ => 380.0,
                },
                None => 390.0,
            }
        };
        let draw_fields = |this: &mut Self, ui: &mut egui::Ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    if let Some(series) = &series {
                        this.artist_editor(ui, &series.id, true);
                    } else if let Some(identity) = selected_axis_identity {
                        if selected_role == Some(SelectableRole::AxisLabel) {
                            this.axis_label_editor_by_identity(ui, identity);
                        } else {
                            this.axis_quick_editor_by_identity(ui, identity);
                        }
                    } else if selected_id == axes_id {
                        this.figure_size_editor(ui);
                        ui.separator();
                        this.axis_ranges_editor(ui);
                    } else if this.document.semantic_label_nodes(&selected_id).is_some() {
                        this.semantic_label_editor(ui, &selected_id);
                    } else if this.document.artist_record(&selected_id).is_some() {
                        this.artist_editor(ui, &selected_id, true);
                    } else {
                        ui.weak(this.language.text(Text::ClickToEdit));
                    }
                });
        };
        let window_spec =
            instplot_ui::ToolWindowSpec::new([editor_width, editor_height], [380.0, 260.0]);
        if instplot_ui::ToolWindowPolicy::mode(context) == instplot_ui::ToolWindowMode::Embedded {
            let mut open = true;
            let available = context.content_rect();
            window_spec
                .embedded(title.clone(), embedded_id)
                .open(&mut open)
                .default_pos(egui::pos2(
                    (available.right() - editor_width - 30.0).max(24.0),
                    72.0 + index as f32 * 28.0,
                ))
                .frame(studio_card_frame(context.theme() == egui::Theme::Dark))
                .show(context, |ui| draw_fields(self, ui));
            if !open {
                self.delete_empty_annotation_on_close(&selected_id);
            }
            return open;
        }
        let builder = window_spec.viewport(title.clone());
        let close_requested =
            context.show_viewport_immediate(viewport_id, builder, |ui, _class| {
                let child_context = ui.ctx().clone();
                let close_requested = instplot_ui::viewport_close_requested(&child_context);
                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::new()
                            .fill(studio_surface(child_context.theme() == egui::Theme::Dark))
                            .inner_margin(egui::Margin::same(16)),
                    )
                    .show(ui, |ui| {
                        draw_fields(self, ui);
                    });
                close_requested
            });
        if close_requested {
            self.delete_empty_annotation_on_close(&selected_id);
        }
        !close_requested
    }

    pub(super) fn handle_drawing_tool_event(&mut self, event: CanvasToolEvent) {
        match self.drawing_tool {
            DrawingTool::Select => {}
            DrawingTool::Reference { orientation, axes } if event.clicked => {
                let Some((x, y)) = event
                    .current
                    .and_then(|point| coordinates_for_binding(point, axes))
                else {
                    return;
                };
                let value = match orientation {
                    ReferenceOrientation::Vertical => x,
                    ReferenceOrientation::Horizontal => y,
                };
                self.execute_document_edit(
                    EditCommand::AddReferenceLine {
                        orientation,
                        value,
                        axes,
                    },
                    "添加参考线",
                );
            }
            DrawingTool::Reference { .. } => {}
            DrawingTool::Measurement {
                axes,
                constraint,
                start_arrow,
                end_arrow,
            } => {
                if event.started {
                    let Some(start) = event
                        .press
                        .or(event.current)
                        .and_then(|point| coordinates_for_binding(point, axes))
                    else {
                        return;
                    };
                    self.tool_draft = Some(ToolDraft {
                        start,
                        end: start,
                        axes,
                        constraint,
                        start_arrow,
                        end_arrow,
                    });
                }
                if let Some(end) = event
                    .current
                    .and_then(|point| coordinates_for_binding(point, axes))
                    && let Some(draft) = &mut self.tool_draft
                {
                    draft.end = match constraint {
                        MeasurementConstraint::Free => end,
                        MeasurementConstraint::Horizontal => (end.0, draft.start.1),
                        MeasurementConstraint::Vertical => (draft.start.0, end.1),
                    };
                }
                if event.stopped
                    && let Some(draft) = self.tool_draft.take()
                {
                    let length = (draft.end.0 - draft.start.0).hypot(draft.end.1 - draft.start.1);
                    if length > f64::EPSILON {
                        self.execute_document_edit(
                            EditCommand::AddMeasurementArrow {
                                start: draft.start,
                                end: draft.end,
                                axes: draft.axes,
                                constraint: draft.constraint,
                                start_arrow: draft.start_arrow,
                                end_arrow: draft.end_arrow,
                                label_nodes: None,
                            },
                            "添加测量箭头",
                        );
                    }
                    self.drawing_tool = DrawingTool::Select;
                }
                let _ = event.shift;
            }
        }
    }

    pub(super) fn handle_artist_drag(&mut self, event: CanvasDragEvent, zoom: f32) {
        let (base_width_mm, base_height_mm) = self.document.figure_size_mm();
        let base_width = base_width_mm * 72.0 / 25.4;
        let base_height = base_height_mm * 72.0 / 25.4;
        let canvas_top = self
            .resolved
            .layout
            .result
            .legend
            .filter(|legend| legend.bottom() < self.resolved.layout.result.axes.y)
            .map(|_| (f64::from(self.resolved.display.height) - base_height).max(0.0))
            .unwrap_or(0.0);
        if event.started {
            let Some(record) = self.document.artist_record(&event.id) else {
                return;
            };
            let mut bounds = event.bounds;
            let axes = self.resolved.layout.result.axes;
            let mut legend_drag = None;
            let connector_index = (event.role == SelectableRole::AnnotationConnector)
                .then_some(event.data_index)
                .flatten();
            let connector_text_bounds = connector_index.and_then(|_| {
                layout_hit_bounds_for_role(&self.resolved, &event.id, SelectableRole::Annotation)
            });
            let mut measurement_drag = None;
            let (start_x, start_y) = match &record.properties {
                ArtistProperties::Annotation { x_pt, y_pt, .. }
                    if event.role == SelectableRole::Annotation =>
                {
                    bounds.1 -= canvas_top;
                    bounds.3 -= canvas_top;
                    (*x_pt, *y_pt)
                }
                ArtistProperties::Annotation { connectors, .. }
                    if event.role == SelectableRole::AnnotationConnector =>
                {
                    let Some(connector) = connector_index.and_then(|index| connectors.get(index))
                    else {
                        return;
                    };
                    (connector.target_x, connector.target_y)
                }
                ArtistProperties::Legend { entries, .. }
                    if event.role == SelectableRole::Legend =>
                {
                    let visible = entries
                        .iter()
                        .filter(|entry| {
                            entry.visible
                                && self.document.project().figure.artists.iter().any(|artist| {
                                    artist.id == entry.artist_id
                                        && self
                                            .document
                                            .project()
                                            .artist_effectively_visible(artist)
                                })
                        })
                        .count()
                        .max(1);
                    let rows = ((bounds.3 - bounds.1 - 8.0) / 12.0).round().max(1.0) as usize;
                    let columns = visible.div_ceil(rows).max(1);
                    legend_drag = Some(LegendDragContext {
                        axes_left: axes.x,
                        axes_right: axes.right(),
                        axes_top: axes.y,
                        axes_bottom: axes.bottom(),
                        canvas_top,
                        entries: visible,
                        cell_width: (bounds.2 - bounds.0) / columns as f64,
                    });
                    (bounds.0, bounds.1)
                }
                ArtistProperties::ReferenceLine { value, .. }
                    if event.role == SelectableRole::ReferenceLine =>
                {
                    (*value, *value)
                }
                ArtistProperties::MeasurementArrow {
                    start_x,
                    start_y,
                    end_x,
                    end_y,
                    axes,
                    label_offset_x_pt,
                    label_offset_y_pt,
                    ..
                } if matches!(
                    event.role,
                    SelectableRole::MeasurementArrow
                        | SelectableRole::MeasurementArrowStart
                        | SelectableRole::MeasurementArrowEnd
                        | SelectableRole::MeasurementArrowLabel
                ) =>
                {
                    let Some(press_pointer) = event.press_pointer else {
                        return;
                    };
                    let press = if event.role == SelectableRole::MeasurementArrowLabel {
                        (0.0, 0.0)
                    } else {
                        let Some(press) = active_data_coordinates_from_local(
                            &self.document,
                            self.resolved.layout.result.axes,
                            press_pointer.0,
                            press_pointer.1,
                        )
                        .and_then(|coordinates| coordinates_for_binding(coordinates, *axes)) else {
                            return;
                        };
                        press
                    };
                    measurement_drag = Some(MeasurementDragContext {
                        start: (*start_x, *start_y),
                        end: (*end_x, *end_y),
                        press,
                        label_offset: (*label_offset_x_pt, *label_offset_y_pt),
                        axes: *axes,
                    });
                    (*start_x, *start_y)
                }
                _ => return,
            };
            self.edit_history.finish_coalescing();
            self.active_artist_drag = Some(ArtistDrag {
                id: event.id.clone(),
                start_x,
                start_y,
                bounds,
                total_delta: egui::Vec2::ZERO,
                press_pointer: event.press_pointer,
                legend: legend_drag,
                mode: event.mode,
                preview_bounds: event.bounds,
                candidate_grid: None,
                candidate_placement: None,
                connector_index,
                connector_text_bounds,
                role: event.role,
                measurement: measurement_drag,
            });
            self.selected_canvas_node = Some(event.id.clone());
            self.selected_canvas_role = Some(event.role);
        }
        let Some(drag) = self
            .active_artist_drag
            .as_mut()
            .filter(|drag| drag.id == event.id)
        else {
            return;
        };
        let frame_delta = drag.frame_delta(event.pointer, event.delta, zoom);
        if frame_delta != egui::Vec2::ZERO && zoom > 0.0 {
            if event.role == SelectableRole::ReferenceLine {
                if let Some((pointer_x, pointer_y)) = event.pointer
                    && let Some(mut record) = self.document.artist_record(&event.id)
                    && let ArtistProperties::ReferenceLine {
                        orientation,
                        value,
                        axes,
                        ..
                    } = &mut record.properties
                    && let Some(coordinates) = active_data_coordinates_from_local(
                        &self.document,
                        self.resolved.layout.result.axes,
                        pointer_x,
                        pointer_y,
                    )
                    && let Some((x, y)) = coordinates_for_binding(coordinates, *axes)
                {
                    *value = match orientation {
                        ReferenceOrientation::Vertical => x,
                        ReferenceOrientation::Horizontal => y,
                    };
                    self.execute_document_edit_with_group(
                        EditCommand::SetArtistRecord(record),
                        self.language.text(Text::ArtistProperties),
                        Some(EditGroup::ArtistProperties),
                    );
                }
            } else if let Some(measurement) = drag.measurement {
                if event.role == SelectableRole::MeasurementArrowLabel {
                    if let (Some(pointer), Some(press_pointer)) =
                        (event.pointer, drag.press_pointer)
                        && let Some(mut record) = self.document.artist_record(&event.id)
                        && let ArtistProperties::MeasurementArrow {
                            label_offset_x_pt,
                            label_offset_y_pt,
                            ..
                        } = &mut record.properties
                    {
                        *label_offset_x_pt =
                            measurement.label_offset.0 + pointer.0 - press_pointer.0;
                        *label_offset_y_pt =
                            measurement.label_offset.1 + pointer.1 - press_pointer.1;
                        self.execute_document_edit_with_group(
                            EditCommand::SetArtistRecord(record),
                            self.language.text(Text::ArtistProperties),
                            Some(EditGroup::ArtistProperties),
                        );
                    }
                    if event.stopped {
                        self.edit_history.finish_coalescing();
                        self.active_artist_drag = None;
                    }
                    return;
                }
                let pointer = event.pointer.map(|(mut pointer_x, mut pointer_y)| {
                    if event.shift
                        && matches!(
                            event.role,
                            SelectableRole::MeasurementArrowStart
                                | SelectableRole::MeasurementArrowEnd
                        )
                    {
                        let fixed_endpoint = if event.role == SelectableRole::MeasurementArrowStart
                        {
                            measurement.end
                        } else {
                            measurement.start
                        };
                        if let Some((fixed_x, fixed_y)) = data_point_to_local(
                            &self.document,
                            self.resolved.layout.result.axes,
                            measurement.axes,
                            fixed_endpoint,
                        ) {
                            let snapped = snap_pointer_to_special_angle(
                                egui::pos2(fixed_x as f32, fixed_y as f32),
                                egui::pos2(pointer_x as f32, pointer_y as f32),
                            );
                            pointer_x = f64::from(snapped.x);
                            pointer_y = f64::from(snapped.y);
                        }
                    }
                    (pointer_x, pointer_y)
                });
                if let Some((pointer_x, pointer_y)) = pointer
                    && let Some(current) = active_data_coordinates_from_local(
                        &self.document,
                        self.resolved.layout.result.axes,
                        pointer_x,
                        pointer_y,
                    )
                    .and_then(|coordinates| coordinates_for_binding(coordinates, measurement.axes))
                    && let Some(mut record) = self.document.artist_record(&event.id)
                    && let ArtistProperties::MeasurementArrow {
                        start_x,
                        start_y,
                        end_x,
                        end_y,
                        constraint,
                        ..
                    } = &mut record.properties
                {
                    match event.role {
                        SelectableRole::MeasurementArrowStart => {
                            *start_x = current.0;
                            *start_y = current.1;
                        }
                        SelectableRole::MeasurementArrowEnd => {
                            *end_x = current.0;
                            *end_y = current.1;
                        }
                        SelectableRole::MeasurementArrow => {
                            let delta = (
                                current.0 - measurement.press.0,
                                current.1 - measurement.press.1,
                            );
                            *start_x = measurement.start.0 + delta.0;
                            *start_y = measurement.start.1 + delta.1;
                            *end_x = measurement.end.0 + delta.0;
                            *end_y = measurement.end.1 + delta.1;
                        }
                        _ => {}
                    }
                    match constraint {
                        MeasurementConstraint::Free => {}
                        MeasurementConstraint::Horizontal => *end_y = *start_y,
                        MeasurementConstraint::Vertical => *end_x = *start_x,
                    }
                    self.execute_document_edit_with_group(
                        EditCommand::SetArtistRecord(record),
                        self.language.text(Text::ArtistProperties),
                        Some(EditGroup::ArtistProperties),
                    );
                }
            } else if let Some(connector_index) = drag.connector_index {
                if let Some((mut pointer_x, mut pointer_y)) = event.pointer
                    && let Some(mut record) = self.document.artist_record(&event.id)
                    && let ArtistProperties::Annotation { connectors, .. } = &mut record.properties
                    && let Some(connector) = connectors.get_mut(connector_index)
                {
                    if event.shift
                        && let Some(bounds) = drag.connector_text_bounds
                    {
                        (pointer_x, pointer_y) =
                            snap_connector_target_to_text_bounds(bounds, (pointer_x, pointer_y));
                    }
                    if let Some((target_x, target_y)) = active_data_coordinates_from_local(
                        &self.document,
                        self.resolved.layout.result.axes,
                        pointer_x,
                        pointer_y,
                    )
                    .and_then(|coordinates| coordinates_for_binding(coordinates, connector.axes))
                    {
                        connector.target_x = target_x;
                        connector.target_y = target_y;
                        self.execute_document_edit_with_group(
                            EditCommand::SetArtistRecord(record),
                            self.language.text(Text::ArtistProperties),
                            Some(EditGroup::ArtistProperties),
                        );
                    }
                }
            } else if let Some(legend) = drag.legend {
                drag.total_delta += frame_delta;
                let dx = f64::from(drag.total_delta.x / zoom);
                let dy = f64::from(drag.total_delta.y / zoom);
                match drag.mode {
                    ArtistDragMode::Move => {
                        let x = drag.start_x + dx;
                        let y = drag.start_y + dy;
                        let width = drag.bounds.2 - drag.bounds.0;
                        let height = drag.bounds.3 - drag.bounds.1;
                        if let Some((pointer_x, pointer_y)) = event.pointer {
                            drag.candidate_placement =
                                Some(legend.placement_at(pointer_x, pointer_y));
                        }
                        let target = drag.candidate_placement.unwrap_or(LegendPlacement::Inside);
                        drag.preview_bounds =
                            legend.preview_bounds((x, y, x + width, y + height), target);
                    }
                    ArtistDragMode::ResizeColumns => {
                        let columns = ((drag.bounds.2 - drag.bounds.0 + dx) / legend.cell_width)
                            .round()
                            .clamp(1.0, legend.entries.min(usize::from(u8::MAX)) as f64)
                            as usize;
                        let rows = legend.entries.div_ceil(columns);
                        drag.candidate_grid = Some(LegendGrid::Columns(columns as u8));
                        drag.preview_bounds = (
                            drag.bounds.0,
                            drag.bounds.1,
                            drag.bounds.0 + columns as f64 * legend.cell_width,
                            drag.bounds.1 + rows as f64 * 12.0 + 6.0,
                        );
                    }
                    ArtistDragMode::ResizeRows => {
                        let rows = ((drag.bounds.3 - drag.bounds.1 + dy - 6.0) / 12.0)
                            .round()
                            .clamp(1.0, legend.entries.min(usize::from(u8::MAX)) as f64)
                            as usize;
                        let columns = legend.entries.div_ceil(rows);
                        drag.candidate_grid = Some(LegendGrid::Rows(rows as u8));
                        drag.preview_bounds = (
                            drag.bounds.0,
                            drag.bounds.1,
                            drag.bounds.0 + columns as f64 * legend.cell_width,
                            drag.bounds.1 + rows as f64 * 12.0 + 6.0,
                        );
                    }
                }
            } else {
                let (x, y) = drag.position_after(frame_delta, zoom, base_width, base_height);
                if let Some(mut record) = self.document.artist_record(&event.id)
                    && let ArtistProperties::Annotation { x_pt, y_pt, .. } = &mut record.properties
                {
                    *x_pt = x;
                    *y_pt = y;
                    self.execute_document_edit_with_group(
                        EditCommand::SetArtistRecord(record),
                        self.language.text(Text::ArtistProperties),
                        Some(EditGroup::ArtistProperties),
                    );
                }
            }
        }
        if event.stopped {
            if event.role == SelectableRole::ReferenceLine {
                self.execute_document_edit_with_group(
                    EditCommand::RefreshAutoscale,
                    "更新自动范围",
                    Some(EditGroup::ArtistProperties),
                );
            }
            if let Some(drag) = self.active_artist_drag.as_ref()
                && drag.legend.is_some()
                && drag.total_delta != egui::Vec2::ZERO
                && let Some(mut record) = self.document.artist_record(&event.id)
                && let ArtistProperties::Legend {
                    x_pt,
                    y_pt,
                    placement,
                    position_custom,
                    grid,
                    ..
                } = &mut record.properties
            {
                match drag.mode {
                    ArtistDragMode::Move => {
                        let target = drag.candidate_placement.unwrap_or(*placement);
                        let legend = drag.legend.expect("legend drag context");
                        *placement = target;
                        *position_custom = true;
                        (*x_pt, *y_pt) = legend.stored_position(drag.preview_bounds, target);
                    }
                    ArtistDragMode::ResizeColumns | ArtistDragMode::ResizeRows => {
                        if let Some(candidate) = drag.candidate_grid {
                            *grid = candidate;
                        }
                    }
                }
                self.execute_document_edit_with_group(
                    EditCommand::SetArtistRecord(record),
                    self.language.text(Text::ArtistProperties),
                    Some(EditGroup::ArtistProperties),
                );
            }
            self.edit_history.finish_coalescing();
            self.active_artist_drag = None;
        }
    }

    pub(super) fn artist_editor(&mut self, ui: &mut egui::Ui, artist_id: &str, compact: bool) {
        let Some(mut record) = self.document.artist_record(artist_id) else {
            return;
        };
        let reference_autoscale_before = matches!(
            record.properties,
            ArtistProperties::ReferenceLine {
                include_in_autoscale: true,
                ..
            }
        );
        let palette = self.document.palette_colors().to_vec();
        let (canvas_width_mm, canvas_height_mm) = self.document.figure_size_mm();
        let mut changed = false;
        let mut continuous_change = false;
        let mut finish_coalescing = false;
        let mut semantic_labels = Vec::new();
        let mut apply_marker_size_to_all = None;
        let mut apply_marker_interval_to_all = None;
        let mut apply_marker_fill_to_all = None;
        let mut series_style_change = None;
        let mut axis_binding_change = None;
        let mut measurement_label_change = None;
        let mut delete_drawing_object = false;
        let properties_title = self.language.text(Text::ArtistProperties);
        let mut draw_properties = |ui: &mut egui::Ui| {
            if matches!(record.role, ArtistRole::Annotation | ArtistRole::Legend) {
                changed |= ui
                    .checkbox(&mut record.visible, self.language.text(Text::Visible))
                    .changed();
            }
            if !compact && !matches!(record.role, ArtistRole::Annotation | ArtistRole::Legend) {
                changed |= role_editor(ui, self.language, &mut record.role);
            }
            if matches!(
                record.properties,
                ArtistProperties::Line { .. } | ArtistProperties::Scatter { .. }
            ) && let Some(current_style) = self.document.series_style(artist_id)
            {
                let mut selected_style = current_style;
                ui.horizontal(|ui| {
                    ui.label("图形类型");
                    egui::ComboBox::from_id_salt(("series-style", artist_id))
                        .selected_text(series_style_name(self.language, selected_style))
                        .show_ui(ui, |ui| {
                            for style in [
                                SeriesCreationStyle::Scatter,
                                SeriesCreationStyle::Line,
                                SeriesCreationStyle::LineAndMarker,
                            ] {
                                ui.selectable_value(
                                    &mut selected_style,
                                    style,
                                    series_style_name(self.language, style),
                                );
                            }
                        });
                });
                if selected_style != current_style {
                    series_style_change = Some(selected_style);
                }
            }
            if matches!(
                record.properties,
                ArtistProperties::Line { .. } | ArtistProperties::Scatter { .. }
            ) && let Some(current_axes) = self.document.series_axis_binding(artist_id)
            {
                let mode = self.document.axis_mode();
                let mut selected_axes = current_axes;
                ui.horizontal(|ui| {
                    ui.label("坐标轴");
                    egui::ComboBox::from_id_salt(("series-axis-binding", artist_id))
                        .selected_text(axis_binding_name(current_axes))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut selected_axes,
                                AxisBinding::PRIMARY,
                                "X1 / Y1",
                            );
                            if mode == AxisMode::DualY {
                                ui.selectable_value(
                                    &mut selected_axes,
                                    AxisBinding {
                                        x: XAxisSlot::X1,
                                        y: YAxisSlot::Y2,
                                    },
                                    "X1 / Y2",
                                );
                            }
                            if mode == AxisMode::DualX {
                                ui.selectable_value(
                                    &mut selected_axes,
                                    AxisBinding {
                                        x: XAxisSlot::X2,
                                        y: YAxisSlot::Y1,
                                    },
                                    "X2 / Y1",
                                );
                            }
                        });
                });
                if !current_axes.is_enabled_in(mode) {
                    ui.weak("该曲线绑定的副轴当前已关闭；重新启用对应模式后会恢复显示。");
                }
                for warning in explicit_axis_unit_conflicts(&self.document, current_axes) {
                    ui.colored_label(egui::Color32::YELLOW, warning);
                }
                if selected_axes != current_axes {
                    axis_binding_change = Some(selected_axes);
                }
            }
            match &mut record.properties {
                ArtistProperties::Line { stroke, .. } => {
                    let edit = stroke_editor(ui, self.language, artist_id, stroke, &palette);
                    changed |= edit.changed;
                    continuous_change |= edit.continuous;
                    finish_coalescing |= edit.finish;
                }
                ArtistProperties::Scatter { marker, .. } => {
                    changed |=
                        color_editor(ui, self.language, artist_id, &mut marker.color_id, &palette);
                    ui.label(self.language.text(Text::MarkerShape));
                    let preview_color = palette
                        .iter()
                        .find(|color| color.id == marker.color_id)
                        .map(|color| {
                            let [r, g, b, _] = color.rgba;
                            egui::Color32::from_rgb(r, g, b)
                        })
                        .unwrap_or(egui::Color32::from_rgb(68, 119, 170));
                    let tile_width = ((ui.available_width() - 100.0) / 2.0).max(96.0);
                    egui::Grid::new(("marker-shape-grid", artist_id))
                        .num_columns(3)
                        .spacing(egui::vec2(8.0, 8.0))
                        .show(ui, |ui| {
                            for shape in PRODUCT_MARKER_SHAPES {
                                ui.label(marker_shape_name(self.language, shape));
                                for (filled, text) in
                                    [(true, Text::FilledMarker), (false, Text::HollowMarker)]
                                {
                                    let response = ui.add_sized(
                                        egui::vec2(tile_width, 40.0),
                                        egui::Button::new(format!(
                                            "    {}",
                                            self.language.text(text)
                                        ))
                                        .selected(marker.shape == shape && marker.filled == filled),
                                    );
                                    paint_marker_preview(
                                        ui.painter(),
                                        response.rect,
                                        shape,
                                        filled,
                                        preview_color,
                                    );
                                    if response.clicked() {
                                        marker.shape = shape;
                                        marker.filled = filled;
                                        changed = true;
                                        if self.marker_fill_for_all {
                                            apply_marker_fill_to_all = Some(filled);
                                        }
                                    }
                                }
                                ui.end_row();
                            }
                        });
                    if ui
                        .checkbox(&mut self.marker_fill_for_all, "实心/空心应用于全部曲线")
                        .changed()
                        && self.marker_fill_for_all
                    {
                        apply_marker_fill_to_all = Some(marker.filled);
                    }
                    if matches!(marker.shape, MarkerShape::Plus | MarkerShape::Cross) {
                        ui.weak(marker_shape_name(self.language, marker.shape));
                    }
                    let (response, apply_on_check) = ui
                        .horizontal(|ui| {
                            let response = ui.add(
                                egui::DragValue::new(&mut marker.size_pt)
                                    .range(0.1..=72.0)
                                    .prefix(format!("{}: ", self.language.text(Text::MarkerSize))),
                            );
                            let apply_on_check = ui
                                .checkbox(&mut self.marker_size_for_all, "全部曲线")
                                .changed()
                                && self.marker_size_for_all;
                            (response, apply_on_check)
                        })
                        .inner;
                    let edit = continuous_edit(&response);
                    changed |= edit.changed;
                    continuous_change |= edit.continuous;
                    finish_coalescing |= edit.finish;
                    if (edit.changed && self.marker_size_for_all) || apply_on_check {
                        apply_marker_size_to_all = Some(marker.size_pt);
                    }
                    let (response, apply_on_check) = ui
                        .horizontal(|ui| {
                            let response = ui.add(
                                egui::DragValue::new(&mut marker.interval)
                                    .range(1..=100_000)
                                    .prefix("每隔点数: "),
                            );
                            let apply_on_check = ui
                                .checkbox(&mut self.marker_interval_for_all, "全部曲线")
                                .changed()
                                && self.marker_interval_for_all;
                            (response, apply_on_check)
                        })
                        .inner;
                    let edit = continuous_edit(&response);
                    changed |= edit.changed;
                    continuous_change |= edit.continuous;
                    finish_coalescing |= edit.finish;
                    if (edit.changed && self.marker_interval_for_all) || apply_on_check {
                        apply_marker_interval_to_all = Some(marker.interval);
                    }
                }
                ArtistProperties::ErrorBar {
                    cap_width_pt,
                    stroke,
                    ..
                } => {
                    let response = ui.add(
                        egui::DragValue::new(cap_width_pt)
                            .range(0.1..=72.0)
                            .prefix(format!("{}: ", self.language.text(Text::CapWidth))),
                    );
                    let edit = continuous_edit(&response);
                    changed |= edit.changed;
                    continuous_change |= edit.continuous;
                    finish_coalescing |= edit.finish;
                    let edit = stroke_editor(ui, self.language, artist_id, stroke, &palette);
                    changed |= edit.changed;
                    continuous_change |= edit.continuous;
                    finish_coalescing |= edit.finish;
                }
                ArtistProperties::ReferenceLine {
                    orientation,
                    value,
                    axes,
                    stroke,
                    include_in_autoscale,
                } => {
                    changed |= reference_axis_binding_editor(
                        ui,
                        ("reference-axis-binding", artist_id),
                        self.document.axis_mode(),
                        *orientation,
                        axes,
                    );
                    let response = ui
                        .horizontal_wrapped(|ui| {
                            ui.label(self.language.text(Text::Orientation));
                            egui::ComboBox::from_id_salt(("reference-orientation", artist_id))
                                .selected_text(reference_orientation_name(
                                    self.language,
                                    *orientation,
                                ))
                                .show_ui(ui, |ui| {
                                    for candidate in [
                                        ReferenceOrientation::Horizontal,
                                        ReferenceOrientation::Vertical,
                                    ] {
                                        changed |= ui
                                            .selectable_value(
                                                orientation,
                                                candidate,
                                                reference_orientation_name(
                                                    self.language,
                                                    candidate,
                                                ),
                                            )
                                            .changed();
                                    }
                                });
                            ui.add(
                                egui::DragValue::new(value)
                                    .prefix(format!("{}: ", self.language.text(Text::Value))),
                            )
                        })
                        .inner;
                    let edit = continuous_edit(&response);
                    changed |= edit.changed;
                    continuous_change |= edit.continuous;
                    finish_coalescing |= edit.finish;
                    if !reference_value_is_visible(&self.document, *orientation, *axes, *value) {
                        ui.colored_label(
                            egui::Color32::YELLOW,
                            "当前值超出坐标轴范围；对象已保存，但画布中不可见。",
                        );
                    }
                    changed |= ui.checkbox(include_in_autoscale, "纳入自动范围").changed();
                    let edit = stroke_editor(ui, self.language, artist_id, stroke, &palette);
                    changed |= edit.changed;
                    continuous_change |= edit.continuous;
                    finish_coalescing |= edit.finish;
                    delete_drawing_object |= ui.button("删除参考线").clicked();
                }
                ArtistProperties::MeasurementArrow {
                    start_x,
                    start_y,
                    end_x,
                    end_y,
                    axes,
                    stroke,
                    start_arrow,
                    end_arrow,
                    arrow_head,
                    arrow_size_pt,
                    constraint,
                    label_id,
                    label_offset_x_pt,
                    label_offset_y_pt,
                } => {
                    changed |= axis_binding_editor(
                        ui,
                        ("measurement-arrow-axis-binding", artist_id),
                        self.document.axis_mode(),
                        axes,
                    );
                    for (name, value) in [
                        ("起点 X", &mut *start_x),
                        ("起点 Y", &mut *start_y),
                        ("终点 X", &mut *end_x),
                        ("终点 Y", &mut *end_y),
                    ] {
                        let response =
                            ui.add(egui::DragValue::new(value).prefix(format!("{name}: ")));
                        let edit = continuous_edit(&response);
                        changed |= edit.changed;
                        continuous_change |= edit.continuous;
                        finish_coalescing |= edit.finish;
                    }
                    if !measurement_points_are_visible(
                        &self.document,
                        *axes,
                        (*start_x, *start_y),
                        (*end_x, *end_y),
                    ) {
                        ui.colored_label(
                            egui::Color32::YELLOW,
                            "一个或多个端点超出坐标轴范围；对象已保存，但超出部分不可见。",
                        );
                    }
                    match constraint {
                        MeasurementConstraint::Horizontal => {
                            let mut delta = *end_x - *start_x;
                            let response = ui.add(egui::DragValue::new(&mut delta).prefix("ΔX: "));
                            if response.changed() {
                                *end_x = *start_x + delta;
                                *end_y = *start_y;
                                changed = true;
                            }
                        }
                        MeasurementConstraint::Vertical => {
                            let mut delta = *end_y - *start_y;
                            let response = ui.add(egui::DragValue::new(&mut delta).prefix("ΔY: "));
                            if response.changed() {
                                *end_y = *start_y + delta;
                                *end_x = *start_x;
                                changed = true;
                            }
                        }
                        MeasurementConstraint::Free => {
                            if let Some((start, end)) = data_point_to_local(
                                &self.document,
                                self.resolved.layout.result.axes,
                                *axes,
                                (*start_x, *start_y),
                            )
                            .zip(data_point_to_local(
                                &self.document,
                                self.resolved.layout.result.axes,
                                *axes,
                                (*end_x, *end_y),
                            )) {
                                let angle = (end.1 - start.1).atan2(end.0 - start.0).to_degrees();
                                ui.weak(format!("屏幕角度: {angle:.1}°"));
                            }
                        }
                    }
                    ui.horizontal(|ui| {
                        ui.label("方向约束");
                        for (candidate, name) in [
                            (MeasurementConstraint::Free, "自由"),
                            (MeasurementConstraint::Horizontal, "水平"),
                            (MeasurementConstraint::Vertical, "垂直"),
                        ] {
                            changed |= ui.selectable_value(constraint, candidate, name).changed();
                        }
                    });
                    ui.horizontal(|ui| {
                        changed |= ui.checkbox(start_arrow, "起点箭头").changed();
                        changed |= ui.checkbox(end_arrow, "末端箭头").changed();
                    });
                    egui::ComboBox::from_id_salt(("measurement-arrow-head", artist_id))
                        .selected_text(match arrow_head {
                            ArrowHead::Open => "空心箭头",
                            ArrowHead::Filled => "实心箭头",
                        })
                        .show_ui(ui, |ui| {
                            changed |= ui
                                .selectable_value(arrow_head, ArrowHead::Open, "空心箭头")
                                .changed();
                            changed |= ui
                                .selectable_value(arrow_head, ArrowHead::Filled, "实心箭头")
                                .changed();
                        });
                    let response = ui.add(
                        egui::DragValue::new(arrow_size_pt)
                            .range(2.0..=18.0)
                            .prefix("箭头大小 (pt): "),
                    );
                    let edit = continuous_edit(&response);
                    changed |= edit.changed;
                    continuous_change |= edit.continuous;
                    finish_coalescing |= edit.finish;
                    stroke.color_id = "object-black".to_owned();
                    let response = ui.add(
                        egui::DragValue::new(&mut stroke.width_pt)
                            .range(0.7..=12.0)
                            .prefix("线宽 (pt): "),
                    );
                    let edit = continuous_edit(&response);
                    changed |= edit.changed;
                    continuous_change |= edit.continuous;
                    finish_coalescing |= edit.finish;
                    ui.horizontal_wrapped(|ui| {
                        ui.label("标签偏移");
                        changed |= ui
                            .add(egui::DragValue::new(label_offset_x_pt).prefix("X: "))
                            .changed();
                        changed |= ui
                            .add(egui::DragValue::new(label_offset_y_pt).prefix("Y: "))
                            .changed();
                    });
                    if let Some(label_id) = label_id {
                        if !compact {
                            semantic_labels.push(label_id.clone());
                        } else {
                            self.quick_semantic_label_editor(ui, label_id);
                        }
                        if ui.button("移除标签").clicked() {
                            measurement_label_change = Some(None);
                        }
                    } else if ui.button("添加标签").clicked() {
                        measurement_label_change =
                            Some(Some(vec![LabelNode::Text("Δx".to_owned())]));
                    }
                    delete_drawing_object |= ui.button("删除测量箭头").clicked();
                }
                ArtistProperties::Annotation {
                    label_id,
                    x_pt,
                    y_pt,
                    connectors,
                } => {
                    let edit = position_editor(
                        ui,
                        self.language,
                        x_pt,
                        y_pt,
                        canvas_width_mm,
                        canvas_height_mm,
                    );
                    changed |= edit.changed;
                    continuous_change |= edit.continuous;
                    finish_coalescing |= edit.finish;
                    if !compact {
                        semantic_labels.push(label_id.clone());
                    } else {
                        self.quick_semantic_label_editor(ui, label_id);
                    }
                    ui.separator();
                    ui.label("连接线与箭头");
                    let mut remove_connector = None;
                    for (index, connector) in connectors.iter_mut().enumerate() {
                        egui::CollapsingHeader::new(format!("连接线 {}", index + 1))
                            .id_salt(("annotation-connector", artist_id, index))
                            .default_open(true)
                            .show(ui, |ui| {
                                changed |= axis_binding_editor(
                                    ui,
                                    ("annotation-connector-axis-binding", artist_id, index),
                                    self.document.axis_mode(),
                                    &mut connector.axes,
                                );
                                ui.horizontal_wrapped(|ui| {
                                    let x_response = ui.add(
                                        egui::DragValue::new(&mut connector.target_x).prefix("X: "),
                                    );
                                    let y_response = ui.add(
                                        egui::DragValue::new(&mut connector.target_y).prefix("Y: "),
                                    );
                                    for response in [x_response, y_response] {
                                        let edit = continuous_edit(&response);
                                        changed |= edit.changed;
                                        continuous_change |= edit.continuous;
                                        finish_coalescing |= edit.finish;
                                    }
                                });
                                let mut arrow_mode =
                                    match (connector.start_arrow, connector.end_arrow) {
                                        (false, false) => 0,
                                        (false, true) => 1,
                                        (true, false) => 2,
                                        (true, true) => 3,
                                    };
                                egui::ComboBox::from_id_salt((
                                    "annotation-arrow-mode",
                                    artist_id,
                                    index,
                                ))
                                .selected_text(
                                    ["无箭头", "末端箭头", "起点箭头", "双向箭头"][arrow_mode],
                                )
                                .show_ui(ui, |ui| {
                                    for (candidate, name) in
                                        ["无箭头", "末端箭头", "起点箭头", "双向箭头"]
                                            .into_iter()
                                            .enumerate()
                                    {
                                        changed |= ui
                                            .selectable_value(&mut arrow_mode, candidate, name)
                                            .changed();
                                    }
                                });
                                (connector.start_arrow, connector.end_arrow) = match arrow_mode {
                                    1 => (false, true),
                                    2 => (true, false),
                                    3 => (true, true),
                                    _ => (false, false),
                                };
                                egui::ComboBox::from_id_salt((
                                    "annotation-arrow-head",
                                    artist_id,
                                    index,
                                ))
                                .selected_text(match connector.arrow_head {
                                    ArrowHead::Open => "空心箭头",
                                    ArrowHead::Filled => "实心箭头",
                                })
                                .show_ui(ui, |ui| {
                                    changed |= ui
                                        .selectable_value(
                                            &mut connector.arrow_head,
                                            ArrowHead::Open,
                                            "空心箭头",
                                        )
                                        .changed();
                                    changed |= ui
                                        .selectable_value(
                                            &mut connector.arrow_head,
                                            ArrowHead::Filled,
                                            "实心箭头",
                                        )
                                        .changed();
                                });
                                let response = ui.add(
                                    egui::DragValue::new(&mut connector.arrow_size_pt)
                                        .range(2.0..=18.0)
                                        .prefix("箭头大小 (pt): "),
                                );
                                let edit = continuous_edit(&response);
                                changed |= edit.changed;
                                continuous_change |= edit.continuous;
                                finish_coalescing |= edit.finish;
                                let connector_id = format!("{artist_id}-connector-{index}");
                                let edit = stroke_editor(
                                    ui,
                                    self.language,
                                    &connector_id,
                                    &mut connector.stroke,
                                    &palette,
                                );
                                changed |= edit.changed;
                                continuous_change |= edit.continuous;
                                finish_coalescing |= edit.finish;
                                if ui.button("删除连接线").clicked() {
                                    remove_connector = Some(index);
                                }
                            });
                    }
                    if let Some(index) = remove_connector {
                        connectors.remove(index);
                        changed = true;
                    }
                    if ui.button("＋连接线").clicked() {
                        let axes = &self.document.project().figure.axes[0];
                        connectors.push(AnnotationConnectorRecord {
                            target_x: (axes.x.minimum + axes.x.maximum) / 2.0,
                            target_y: (axes.y.minimum + axes.y.maximum) / 2.0,
                            axes: AxisBinding::PRIMARY,
                            stroke: StrokeStyle {
                                color_id: ANNOTATION_CONNECTOR_DEFAULT_COLOR_ID.to_owned(),
                                width_pt: ANNOTATION_CONNECTOR_DEFAULT_WIDTH_PT,
                                dash_pt: Vec::new(),
                            },
                            start_arrow: false,
                            end_arrow: true,
                            arrow_head: ArrowHead::Open,
                            arrow_size_pt: 5.0,
                        });
                        changed = true;
                    }
                }
                ArtistProperties::Legend {
                    entries,
                    x_pt,
                    y_pt,
                    placement,
                    position_custom,
                    grid,
                } => {
                    let previous_placement = *placement;
                    let legend_bounds = self.resolved.layout.result.legend;
                    let axes_bounds = self.resolved.layout.result.axes;
                    ui.horizontal(|ui| {
                        ui.label(self.language.text(Text::LegendPlacement));
                        for (candidate, text) in [
                            (LegendPlacement::Auto, Text::Auto),
                            (LegendPlacement::Inside, Text::LegendInside),
                            (LegendPlacement::Above, Text::LegendAbove),
                            (LegendPlacement::Right, Text::LegendRight),
                        ] {
                            let placement_changed = ui
                                .selectable_value(placement, candidate, self.language.text(text))
                                .changed();
                            changed |= placement_changed;
                            if placement_changed {
                                *position_custom = false;
                            }
                        }
                    });
                    let mode_changed = *placement != previous_placement;
                    ui.label(self.language.text(Text::LegendOriginHint));
                    if mode_changed
                        && *placement == LegendPlacement::Inside
                        && let Some(bounds) = legend_bounds
                    {
                        let base_width = canvas_width_mm * 72.0 / 25.4;
                        let base_height = canvas_height_mm * 72.0 / 25.4;
                        let outside_above = bounds.bottom() < axes_bounds.y;
                        let top_extension = if outside_above {
                            f64::from(self.resolved.display.height) - base_height
                        } else {
                            0.0
                        };
                        *x_pt = bounds.x.clamp(0.0, (base_width - bounds.width).max(0.0));
                        *y_pt = (bounds.y - top_extension)
                            .clamp(0.0, (base_height - bounds.height).max(0.0));
                    }
                    if !mode_changed && let Some(bounds) = legend_bounds {
                        let output_width = f64::from(self.resolved.display.width) * 25.4 / 72.0;
                        let output_height = f64::from(self.resolved.display.height) * 25.4 / 72.0;
                        let mut shown_x = bounds.x;
                        let mut shown_y = bounds.y;
                        let edit = position_editor(
                            ui,
                            self.language,
                            &mut shown_x,
                            &mut shown_y,
                            output_width,
                            output_height,
                        );
                        if edit.changed {
                            if *placement == LegendPlacement::Auto {
                                *placement = if bounds.bottom() < axes_bounds.y {
                                    LegendPlacement::Above
                                } else if bounds.x > axes_bounds.right() {
                                    LegendPlacement::Right
                                } else {
                                    LegendPlacement::Inside
                                };
                            }
                            if *placement == LegendPlacement::Right {
                                shown_x = shown_x.max(axes_bounds.right() + 6.0);
                            }
                            *x_pt = shown_x;
                            *y_pt = shown_y;
                            *position_custom = true;
                        }
                        changed |= edit.changed;
                        continuous_change |= edit.continuous;
                        finish_coalescing |= edit.finish;
                    }
                    let visible_entries = entries
                        .iter()
                        .filter(|entry| {
                            entry.visible
                                && self.document.project().figure.artists.iter().any(|artist| {
                                    artist.id == entry.artist_id
                                        && self
                                            .document
                                            .project()
                                            .artist_effectively_visible(artist)
                                })
                        })
                        .count()
                        .max(1);
                    let max_grid = visible_entries.min(usize::from(u8::MAX));
                    let rendered_rows = self
                        .resolved
                        .layout
                        .result
                        .legend
                        .map(|bounds| ((bounds.height - 6.0) / 12.0).round().max(1.0) as usize)
                        .unwrap_or(visible_entries)
                        .clamp(1, visible_entries);
                    let mut rows = match grid {
                        LegendGrid::Rows(value) => usize::from(*value).clamp(1, visible_entries),
                        LegendGrid::Columns(value) => {
                            visible_entries.div_ceil(usize::from(*value).max(1))
                        }
                        LegendGrid::Auto => rendered_rows,
                    };
                    let mut columns = match grid {
                        LegendGrid::Columns(value) => usize::from(*value).clamp(1, visible_entries),
                        LegendGrid::Rows(value) => {
                            visible_entries.div_ceil(usize::from(*value).max(1))
                        }
                        LegendGrid::Auto => visible_entries.div_ceil(rendered_rows),
                    };
                    ui.horizontal(|ui| {
                        ui.label(self.language.text(Text::LegendGrid));
                        if ui
                            .selectable_label(
                                matches!(grid, LegendGrid::Auto),
                                self.language.text(Text::Auto),
                            )
                            .clicked()
                        {
                            *grid = LegendGrid::Auto;
                            changed = true;
                        }
                    });
                    ui.horizontal(|ui| {
                        let row_edit = ui.add(
                            egui::DragValue::new(&mut rows)
                                .range(1..=max_grid)
                                .prefix(format!("{}: ", self.language.text(Text::LegendRows))),
                        );
                        if row_edit.changed() {
                            *grid = LegendGrid::Rows(rows as u8);
                            changed = true;
                        }
                        let column_edit = ui.add(
                            egui::DragValue::new(&mut columns)
                                .range(1..=max_grid)
                                .prefix(format!("{}: ", self.language.text(Text::LegendColumns))),
                        );
                        if column_edit.changed() {
                            *grid = LegendGrid::Columns(columns as u8);
                            changed = true;
                        }
                    });
                    let output_width = f64::from(self.resolved.display.width) * 25.4 / 72.0;
                    let output_height = f64::from(self.resolved.display.height) * 25.4 / 72.0;
                    if *placement != LegendPlacement::Inside {
                        ui.weak(format!(
                            "{}: {output_width:.2} × {output_height:.2} mm",
                            self.language.text(Text::OutputCanvasSize)
                        ));
                    }
                    ui.label(self.language.text(Text::LegendEntries));
                    let mut move_entry = None;
                    let entry_count = entries.len();
                    for (index, entry) in entries.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            changed |= ui.checkbox(&mut entry.visible, "").changed();
                            let mut order = index + 1;
                            if ui
                                .add(
                                    egui::DragValue::new(&mut order)
                                        .range(1..=entry_count)
                                        .speed(0.1)
                                        .prefix(format!("{}: ", self.language.text(Text::Order))),
                                )
                                .changed()
                            {
                                move_entry = Some((index, order - 1));
                            }
                            let name = self
                                .document
                                .semantic_label_nodes(&entry.label_id)
                                .map(label_input::display_text)
                                .unwrap_or_else(|| entry.artist_id.clone());
                            ui.label(name);
                        });
                        if compact {
                            egui::CollapsingHeader::new(format!(
                                "{} {}",
                                self.language.text(Text::EditEntry),
                                index + 1
                            ))
                            .id_salt(("legend-entry-label", &entry.label_id))
                            .default_open(false)
                            .show(ui, |ui| {
                                self.quick_semantic_label_editor(ui, &entry.label_id);
                            });
                        }
                        if !compact {
                            semantic_labels.push(entry.label_id.clone());
                        }
                    }
                    if let Some((from, to)) = move_entry {
                        move_legend_entry(entries, from, to);
                        changed = true;
                    }
                }
            }
        };
        if compact {
            draw_properties(ui);
        } else {
            ui.separator();
            egui::CollapsingHeader::new(properties_title)
                .default_open(true)
                .show(ui, draw_properties);
        }
        if delete_drawing_object {
            self.execute_document_edit(
                EditCommand::DeleteDrawingObject {
                    artist_id: artist_id.to_owned(),
                },
                "删除绘图对象",
            );
            self.selected_canvas_node = None;
            self.selected_canvas_role = None;
            return;
        }
        if let Some(nodes) = measurement_label_change {
            self.execute_document_edit(
                EditCommand::SetMeasurementArrowLabel {
                    artist_id: artist_id.to_owned(),
                    nodes,
                },
                "编辑测量标签",
            );
            return;
        }
        if let Some(style) = series_style_change {
            if self.execute_document_edit(
                EditCommand::SetSeriesStyle {
                    artist_id: artist_id.to_owned(),
                    style,
                },
                "更改图形类型",
            ) {
                ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
            }
            return;
        }
        if let Some(axes) = axis_binding_change {
            self.execute_document_edit(
                EditCommand::SetSeriesAxisBinding {
                    artist_id: artist_id.to_owned(),
                    axes,
                },
                "更改曲线坐标轴",
            );
        }
        let refresh_reference_autoscale = changed
            && (reference_autoscale_before
                || matches!(
                    &record.properties,
                    ArtistProperties::ReferenceLine {
                        include_in_autoscale: true,
                        ..
                    }
                ));
        let edit_group = (continuous_change || refresh_reference_autoscale)
            .then_some(EditGroup::ArtistProperties);
        if changed
            && self.execute_document_edit_with_group(
                EditCommand::SetArtistRecord(record),
                self.language.text(Text::ArtistProperties),
                edit_group,
            )
        {
            if refresh_reference_autoscale {
                self.execute_document_edit_with_group(
                    EditCommand::RefreshAutoscale,
                    "更新自动范围",
                    edit_group,
                );
            }
            ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
        }
        if finish_coalescing {
            self.edit_history.finish_coalescing();
        }
        if let Some(size_pt) = apply_marker_size_to_all {
            self.execute_document_edit(
                EditCommand::SetAllMarkerSizes { size_pt },
                self.language.text(Text::ApplyMarkerToAll),
            );
        }
        if let Some(interval) = apply_marker_interval_to_all {
            self.execute_document_edit(
                EditCommand::SetAllMarkerIntervals { interval },
                self.language.text(Text::ApplyMarkerToAll),
            );
        }
        if let Some(filled) = apply_marker_fill_to_all {
            self.execute_document_edit(
                EditCommand::SetAllMarkerFilled { filled },
                self.language.text(Text::ApplyMarkerToAll),
            );
        }
        for label_id in semantic_labels {
            self.semantic_label_editor(ui, &label_id);
        }
    }

    pub(super) fn quick_semantic_label_editor(&mut self, ui: &mut egui::Ui, label_id: &str) {
        self.semantic_label_editor(ui, label_id);
    }

    pub(super) fn semantic_label_editor(&mut self, ui: &mut egui::Ui, label_id: &str) {
        let Some(nodes) = self.document.semantic_label_nodes(label_id) else {
            return;
        };
        let initial = label_input::format(nodes)
            .or_else(|| {
                self.label_inputs
                    .get(label_id)
                    .filter(|state| state.source_nodes == nodes)
                    .map(|state| state.text.clone())
            })
            .unwrap_or_else(|| label_input::display_text(nodes));
        self.label_input_editor(
            ui,
            label_id,
            nodes.to_vec(),
            initial,
            LabelInputTarget::Semantic(label_id),
        );
    }

    pub(super) fn label_input_editor(
        &mut self,
        ui: &mut egui::Ui,
        key: &str,
        current: Vec<LabelNode>,
        initial: String,
        target: LabelInputTarget<'_>,
    ) {
        let annotation_artist = match target {
            LabelInputTarget::Semantic(label_id) => self.annotation_artist_for_label(label_id),
            LabelInputTarget::Axis(_) => None,
        };
        let measurement_artist = match target {
            LabelInputTarget::Semantic(label_id) => self.measurement_artist_for_label(label_id),
            LabelInputTarget::Axis(_) => None,
        };
        let allows_hard_line_breaks = annotation_artist.is_some() || measurement_artist.is_some();
        let state = self
            .label_inputs
            .entry(key.to_owned())
            .or_insert_with(|| LabelInputState {
                source_nodes: current.clone(),
                text: initial.clone(),
            });
        if state.source_nodes != current {
            state.source_nodes = current.clone();
            state.text = initial;
        }
        let mut text = state.text.clone();
        let before = text.clone();
        let mut output = egui::ScrollArea::vertical()
            .max_height(150.0)
            .show(ui, |ui| {
                egui::TextEdit::multiline(&mut text)
                    .id_salt(("label-input", key))
                    .desired_width(ui.available_width())
                    .desired_rows(2)
                    .hint_text("例如：$T$ <= 300 K；H_{DL} (mT)")
                    .show(ui)
            })
            .inner;
        text_input::auto_pair_brackets(
            ui,
            &before,
            &mut text,
            &mut output,
            text_input::BracketMode::FigureText,
        );
        let mut changed = output.response.changed();
        let mut picked: Option<(String, bool)> = None;
        ui.horizontal(|ui| {
            let button_width = 64.0;
            let total_width = button_width * 4.0 + ui.spacing().item_spacing.x * 3.0;
            ui.add_space(((ui.available_width() - total_width) * 0.5).max(0.0));
            for (caption, snippet, inside, help) in [
                ("斜体", "$$", true, "用 $…$ 标记斜体变量"),
                ("下标", "_{}", true, "插入下标 _{…}"),
                ("上标", "^{}", true, "插入上标 ^{…}"),
                ("正体", "\\mathrm{}", true, "在数学区域内强制使用正体"),
            ] {
                if studio_label_format_button(ui, caption)
                    .on_hover_text(help)
                    .clicked()
                {
                    picked = Some((snippet.to_owned(), inside));
                }
            }
        });
        ui.horizontal(|ui| {
            let symbol_width = 176.0;
            let help_width = 96.0;
            let total_width = symbol_width + help_width + ui.spacing().item_spacing.x;
            ui.add_space(((ui.available_width() - total_width) * 0.5).max(0.0));
            let symbol_button = egui::Button::new(
                egui::RichText::new("数学符号与希腊字母")
                    .size(16.0)
                    .line_height(Some(20.0)),
            )
            .min_size(egui::vec2(symbol_width, 40.0));
            egui::containers::menu::MenuButton::from_button(symbol_button).ui(ui, |ui| {
                ui.set_min_width(370.0);
                egui::ScrollArea::vertical()
                    .max_height(390.0)
                    .show(ui, |ui| {
                        for (heading, range) in [
                            ("希腊字母 · 小写与变体", 0..label_input::GREEK_LOWER_END),
                            (
                                "希腊字母 · 大写",
                                label_input::GREEK_LOWER_END..label_input::GREEK_UPPER_END,
                            ),
                            (
                                "数学运算与关系",
                                label_input::GREEK_UPPER_END..label_input::MATH_RELATIONS_END,
                            ),
                            (
                                "其他科学符号",
                                label_input::MATH_RELATIONS_END..label_input::SYMBOLS.len(),
                            ),
                        ] {
                            ui.label(heading);
                            let greek = range.start < label_input::GREEK_UPPER_END;
                            egui::Grid::new(("label-symbols", key, heading))
                                .num_columns(8)
                                .show(ui, |ui| {
                                    for (index, (symbol, command)) in
                                        label_input::SYMBOLS[range].iter().enumerate()
                                    {
                                        if studio_symbol_button(ui, symbol, greek)
                                            .on_hover_text(*command)
                                            .clicked()
                                        {
                                            picked = Some(((*symbol).to_owned(), false));
                                            ui.close();
                                        }
                                        if index % 8 == 7 {
                                            ui.end_row();
                                        }
                                    }
                                });
                            ui.add_space(8.0);
                        }
                    });
            });
            let help_button = egui::Button::new(
                egui::RichText::new("输入帮助")
                    .size(16.0)
                    .line_height(Some(20.0)),
            )
            .min_size(egui::vec2(help_width, 40.0));
            egui::containers::menu::MenuButton::from_button(help_button).ui(ui, |ui| {
                ui.label("普通区域一律正体；拉丁字母和希腊字母可直接输入");
                ui.label("斜体变量写在一对 $...$ 内，例如 $R$、$α$、$R_x^y$");
                ui.label("比较符号：<=、>=、!=，或直接输入 ≤、≥、≠");
                ui.label("正体上下标：R_x^y；组合上下标会排在同一基字符旁");
                ui.label("数学上下标：$R_x^y$ 中 R、x、y 均为斜体变量");
                ui.label("数学区内强制正体：$R_{\\mathrm{eff}}$ 或使用“正体”按钮");
                ui.label("符号可点选、粘贴，或输入 \\sigma、\\sum、<=");
                ui.label("星号必须转义：输入 \\* 可显示 *");
                ui.label("“斜体”按钮会给所选内容加上一对 $；普通文字不需要 $。");
            });
        });
        if let Some((snippet, inside)) = picked {
            insert_label_snippet(&mut text, &mut output, &snippet, inside, ui.ctx());
            changed = true;
        }
        changed |= normalize_label_editor_line_breaks(&mut text, allows_hard_line_breaks);
        self.label_inputs.get_mut(key).unwrap().text = text.clone();
        let empty_annotation = text.trim().is_empty() && annotation_artist.is_some();
        let empty_measurement = text.trim().is_empty() && measurement_artist.is_some();
        if empty_annotation {
            ui.weak("内容已清空；关闭窗口后将删除此标注");
        } else if empty_measurement {
            ui.weak("内容已清空；测量标签已移除，箭头保留");
            if changed && let Some(artist_id) = measurement_artist {
                self.execute_document_edit(
                    EditCommand::SetMeasurementArrowLabel {
                        artist_id,
                        nodes: None,
                    },
                    "移除测量标签",
                );
            }
        } else {
            match label_input::parse(&text).and_then(|nodes| {
                label_input::validate_glyphs(&nodes)?;
                Ok(nodes)
            }) {
                Ok(nodes) => {
                    ui.weak("预览（画布为准）：");
                    ui.label(label_preview_job(&nodes))
                        .on_hover_text(label_input::display_text(&nodes));
                    if changed && nodes != current {
                        let command = match target {
                            LabelInputTarget::Axis(identity) => {
                                EditCommand::SetAxisLabelByIdentity {
                                    identity,
                                    nodes: nodes.clone(),
                                }
                            }
                            LabelInputTarget::Semantic(label_id) => EditCommand::SetSemanticLabel {
                                label_id: label_id.to_owned(),
                                nodes: nodes.clone(),
                            },
                        };
                        if self.execute_document_edit_with_group(
                            command,
                            self.language.text(Text::ApplyLabel),
                            Some(EditGroup::SemanticLabel),
                        ) {
                            self.label_inputs.get_mut(key).unwrap().source_nodes = nodes;
                        }
                    }
                }
                Err(error) => {
                    ui.colored_label(egui::Color32::from_rgb(180, 75, 50), error);
                }
            }
        }
        if output.response.lost_focus() {
            self.edit_history.finish_coalescing();
        }
    }
}

fn axis_dimension(identity: AxisIdentity) -> AxisDimension {
    match identity {
        AxisIdentity::X1 | AxisIdentity::X2 => AxisDimension::X,
        AxisIdentity::Y1 | AxisIdentity::Y2 => AxisDimension::Y,
    }
}

fn axis_title(language: UiLanguage, identity: AxisIdentity) -> String {
    let suffix = match language {
        UiLanguage::Chinese => "轴",
        UiLanguage::English => "Axis",
    };
    match identity {
        AxisIdentity::X1 => format!("X1 {suffix}"),
        AxisIdentity::X2 => format!("X2 {suffix}"),
        AxisIdentity::Y1 => format!("Y1 {suffix}"),
        AxisIdentity::Y2 => format!("Y2 {suffix}"),
    }
}

fn axis_binding_name(binding: AxisBinding) -> &'static str {
    match (binding.x, binding.y) {
        (XAxisSlot::X1, YAxisSlot::Y1) => "X1 / Y1",
        (XAxisSlot::X2, YAxisSlot::Y1) => "X2 / Y1",
        (XAxisSlot::X1, YAxisSlot::Y2) => "X1 / Y2",
        (XAxisSlot::X2, YAxisSlot::Y2) => "不支持",
    }
}

fn coordinates_for_binding(
    coordinates: HoverDataCoordinates,
    binding: AxisBinding,
) -> Option<(f64, f64)> {
    let x = match binding.x {
        XAxisSlot::X1 => coordinates.x1,
        XAxisSlot::X2 => coordinates.x2?,
    };
    let y = match binding.y {
        YAxisSlot::Y1 => coordinates.y1,
        YAxisSlot::Y2 => coordinates.y2?,
    };
    Some((x, y))
}

fn axis_value_is_visible(axis: &instplot_studio::AxisRecord, value: f64) -> bool {
    value.is_finite()
        && value >= axis.minimum
        && value <= axis.maximum
        && (!matches!(axis.scale, instplot_studio::AxisScale::Log10) || value > 0.0)
}

fn reference_value_is_visible(
    document: &FigureDocument,
    orientation: ReferenceOrientation,
    binding: AxisBinding,
    value: f64,
) -> bool {
    let identity = match orientation {
        ReferenceOrientation::Vertical => match binding.x {
            XAxisSlot::X1 => AxisIdentity::X1,
            XAxisSlot::X2 => AxisIdentity::X2,
        },
        ReferenceOrientation::Horizontal => match binding.y {
            YAxisSlot::Y1 => AxisIdentity::Y1,
            YAxisSlot::Y2 => AxisIdentity::Y2,
        },
    };
    document
        .axis_record_by_identity(identity)
        .is_some_and(|axis| axis_value_is_visible(&axis, value))
}

fn measurement_points_are_visible(
    document: &FigureDocument,
    binding: AxisBinding,
    start: (f64, f64),
    end: (f64, f64),
) -> bool {
    let x_identity = match binding.x {
        XAxisSlot::X1 => AxisIdentity::X1,
        XAxisSlot::X2 => AxisIdentity::X2,
    };
    let y_identity = match binding.y {
        YAxisSlot::Y1 => AxisIdentity::Y1,
        YAxisSlot::Y2 => AxisIdentity::Y2,
    };
    document
        .axis_record_by_identity(x_identity)
        .zip(document.axis_record_by_identity(y_identity))
        .is_some_and(|(x_axis, y_axis)| {
            [start, end].into_iter().all(|(x, y)| {
                axis_value_is_visible(&x_axis, x) && axis_value_is_visible(&y_axis, y)
            })
        })
}

fn axis_binding_editor(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    mode: AxisMode,
    binding: &mut AxisBinding,
) -> bool {
    let before = *binding;
    ui.horizontal(|ui| {
        ui.label("坐标轴");
        egui::ComboBox::from_id_salt(id_salt)
            .selected_text(axis_binding_name(*binding))
            .show_ui(ui, |ui| {
                ui.selectable_value(binding, AxisBinding::PRIMARY, "X1 / Y1");
                if mode == AxisMode::DualY {
                    ui.selectable_value(
                        binding,
                        AxisBinding {
                            x: XAxisSlot::X1,
                            y: YAxisSlot::Y2,
                        },
                        "X1 / Y2",
                    );
                }
                if mode == AxisMode::DualX {
                    ui.selectable_value(
                        binding,
                        AxisBinding {
                            x: XAxisSlot::X2,
                            y: YAxisSlot::Y1,
                        },
                        "X2 / Y1",
                    );
                }
            });
    });
    if !binding.is_enabled_in(mode) {
        ui.weak("所绑定的副轴当前已关闭；此数据坐标对象暂时隐藏。");
    }
    *binding != before
}

fn reference_axis_binding_editor(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    mode: AxisMode,
    orientation: ReferenceOrientation,
    binding: &mut AxisBinding,
) -> bool {
    let before = *binding;
    ui.horizontal(|ui| {
        ui.label("坐标轴");
        egui::ComboBox::from_id_salt(id_salt)
            .selected_text(match orientation {
                ReferenceOrientation::Vertical => match binding.x {
                    XAxisSlot::X1 => "X1",
                    XAxisSlot::X2 => "X2",
                },
                ReferenceOrientation::Horizontal => match binding.y {
                    YAxisSlot::Y1 => "Y1",
                    YAxisSlot::Y2 => "Y2",
                },
            })
            .show_ui(ui, |ui| match orientation {
                ReferenceOrientation::Vertical => {
                    ui.selectable_value(&mut binding.x, XAxisSlot::X1, "X1");
                    if mode == AxisMode::DualX {
                        ui.selectable_value(&mut binding.x, XAxisSlot::X2, "X2");
                    }
                    binding.y = YAxisSlot::Y1;
                }
                ReferenceOrientation::Horizontal => {
                    ui.selectable_value(&mut binding.y, YAxisSlot::Y1, "Y1");
                    if mode == AxisMode::DualY {
                        ui.selectable_value(&mut binding.y, YAxisSlot::Y2, "Y2");
                    }
                    binding.x = XAxisSlot::X1;
                }
            });
    });
    if !binding.is_enabled_in(mode) {
        ui.weak("所绑定的副轴当前已关闭；此参考线暂时隐藏。");
    }
    *binding != before
}

fn axis_identity_for_project_id(
    document: &FigureDocument,
    project_id: &str,
) -> Option<AxisIdentity> {
    let axes = &document.project().figure.axes[0];
    if axes.x.id == project_id || axes.x.label_id == project_id {
        Some(AxisIdentity::X1)
    } else if axes
        .x2
        .as_ref()
        .is_some_and(|axis| axis.id == project_id || axis.label_id == project_id)
    {
        Some(AxisIdentity::X2)
    } else if axes.y.id == project_id || axes.y.label_id == project_id {
        Some(AxisIdentity::Y1)
    } else if axes
        .y2
        .as_ref()
        .is_some_and(|axis| axis.id == project_id || axis.label_id == project_id)
    {
        Some(AxisIdentity::Y2)
    } else {
        None
    }
}

fn explicit_axis_unit_conflicts(document: &FigureDocument, selected: AxisBinding) -> Vec<String> {
    let mut x_units = BTreeSet::new();
    let mut y_units = BTreeSet::new();
    for series in document.series() {
        let Some(binding) = series.binding.as_ref() else {
            continue;
        };
        let Some(axes) = series.axes else {
            continue;
        };
        if axes.x == selected.x
            && let Some(unit) = explicit_column_unit(&binding.x_column)
        {
            x_units.insert(unit.to_owned());
        }
        if axes.y == selected.y
            && let Some(unit) = explicit_column_unit(&binding.y_column)
        {
            y_units.insert(unit.to_owned());
        }
    }
    let mut warnings = Vec::new();
    if x_units.len() > 1 {
        warnings.push(format!(
            "{} 上存在明确且不一致的单位：{}；请检查曲线分配。",
            match selected.x {
                XAxisSlot::X1 => "X1",
                XAxisSlot::X2 => "X2",
            },
            x_units.into_iter().collect::<Vec<_>>().join("、")
        ));
    }
    if y_units.len() > 1 {
        warnings.push(format!(
            "{} 上存在明确且不一致的单位：{}；请检查曲线分配。",
            match selected.y {
                YAxisSlot::Y1 => "Y1",
                YAxisSlot::Y2 => "Y2",
            },
            y_units.into_iter().collect::<Vec<_>>().join("、")
        ));
    }
    warnings
}

pub(super) fn explicit_column_unit(column: &str) -> Option<&str> {
    let trimmed = column.trim();
    for (open, close) in [('(', ')'), ('[', ']')] {
        let Some(start) = trimmed.rfind(open) else {
            continue;
        };
        if trimmed.ends_with(close) && start + open.len_utf8() < trimmed.len() - close.len_utf8() {
            return Some(&trimmed[start + open.len_utf8()..trimmed.len() - close.len_utf8()]);
        }
    }
    None
}

fn managed_format_from_path(path: &Path) -> Option<ManagedDataFormat> {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("csv") => Some(ManagedDataFormat::Csv),
        Some("tsv") => Some(ManagedDataFormat::Tsv),
        Some("txt") => Some(ManagedDataFormat::Txt),
        Some("dat") => Some(ManagedDataFormat::Dat),
        Some("xlsx") => Some(ManagedDataFormat::Xlsx),
        _ => None,
    }
}

fn managed_save_conflict_explanation(diagnostics: &[String]) -> Option<String> {
    let details = diagnostics.join("；");
    if details.contains("changed outside Studio") {
        Some("关联的手动数据文件已被其他程序修改。覆盖会用当前项目数据替换外部更改；另存为可保留两份；重新打开项目会放弃本次未保存修改。".to_owned())
    } else if details.contains("managed manual data file is missing") {
        Some("关联的手动数据文件已移动或删除。覆盖会按原路径重新创建；另存为可选择新位置；重新打开项目会放弃本次未保存修改。".to_owned())
    } else if details.contains("refusing to overwrite an unrelated file") {
        Some(
            "目标位置已有不属于当前项目的文件。覆盖会替换该文件；另存为可选择安全的新位置。"
                .to_owned(),
        )
    } else {
        None
    }
}
