use super::*;

impl StudioApp {
    pub(crate) fn new(
        creation: &eframe::CreationContext<'_>,
        started: Instant,
        startup: Option<HandoffImport>,
    ) -> Self {
        // Keep Studio's dark interface consistent across operating system themes.
        creation.egui_ctx.set_theme(egui::ThemePreference::Dark);
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
            branding: instplot_ui::Branding::new(PRODUCT_NAME, env!("CARGO_PKG_VERSION")),
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
            axis_numeric_scale_sessions: Vec::new(),
            axis_scale_transitions: Vec::new(),
            x_fixed_ticks,
            y_fixed_ticks,
            workspace,
            edit_history,
            pending_action: None,
            pending_export: None,
            allow_close: false,
            canvas_zoom: 1.5,
            canvas_scroll: egui::Vec2::ZERO,
            last_canvas_figure_center: None,
            hover_data_coordinates: None,
            trackpad_scroll_active: false,
            show_layers: false,
            show_inspector: false,
            show_palette: false,
            show_messages: false,
            focus_inspector: false,
            focus_palette: false,
            focus_manual_data: false,
            show_axis_visibility: false,
            focus_axis_visibility: false,
            axis_visibility_identity: AxisIdentity::X1,
            context_editor_focus_target: None,
            context_editor_targets: Vec::new(),
            active_artist_drag: None,
            drawing_tool: DrawingTool::Select,
            tool_draft: None,
            reference_draft: None,
            focus_reference_draft: false,
            messages: Vec::new(),
            status: Some((status, Instant::now())),
            manual_data: ManualDataState::default(),
            marker_size_for_all: false,
            marker_interval_for_all: false,
            marker_fill_for_all: false,
            language,
            update: AppUpdateState::default(),
            #[cfg(target_os = "macos")]
            macos_open_files: None,
            first_frame: true,
            started,
        }
    }

    pub(crate) fn open_data(&mut self) {
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

    pub(crate) fn load_data_paths(&mut self, paths: Vec<PathBuf>) {
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
            affected_axes,
        } = effect
        else {
            unreachable!("import action must produce an import effect");
        };
        self.clear_axis_numeric_drafts(&affected_axes);
        if replaced_showcase {
            self.selected_series = None;
            self.selected_canvas_node = None;
            self.selected_canvas_role = None;
            self.selection_candidates.clear();
            self.context_editor_targets.clear();
            self.active_artist_drag = None;
            self.drawing_tool = DrawingTool::Select;
            self.tool_draft = None;
            self.reference_draft = None;
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
        self.last_canvas_figure_center = None;
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

    pub(crate) fn open_paths(&mut self, paths: Vec<PathBuf>) {
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
    pub(crate) fn open_lite_handoff(&mut self) {
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
                self.drawing_tool = DrawingTool::Select;
                self.tool_draft = None;
                self.reference_draft = None;
                self.selected_dataset = None;
                self.clear_all_axis_numeric_drafts();
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

    pub(crate) fn open_project(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title(self.language.text(Text::OpenProjectDialog))
            .add_filter(self.language.text(Text::ProjectFile), &["instplot"])
            .pick_file()
        else {
            return;
        };
        self.open_project_path(path);
    }

    pub(crate) fn open_project_path(&mut self, path: PathBuf) {
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
                self.drawing_tool = DrawingTool::Select;
                self.tool_draft = None;
                self.reference_draft = None;
                self.selected_dataset = None;
                self.show_layers = !self.session.datasets().is_empty();
                self.first_frame = true;
                self.clear_all_axis_numeric_drafts();
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

    pub(crate) fn new_project(&mut self) {
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
        self.drawing_tool = DrawingTool::Select;
        self.tool_draft = None;
        self.reference_draft = None;
        self.selected_dataset = None;
        self.clear_all_axis_numeric_drafts();
        self.sync_axis_editors();
        self.messages.clear();
        self.set_success(self.language.text(Text::Ready).to_owned());
    }

    pub(crate) fn save_project(&mut self, save_as: bool) -> bool {
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

    pub(crate) fn execute_project_save(&mut self, path: PathBuf, overwrite_managed: bool) -> bool {
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

    pub(crate) fn managed_save_conflict_dialog(&mut self, context: &egui::Context) {
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
}
