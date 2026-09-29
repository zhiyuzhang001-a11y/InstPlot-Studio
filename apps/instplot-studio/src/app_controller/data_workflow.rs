use super::*;

impl StudioApp {
    pub(crate) fn manual_data_window(&mut self, context: &egui::Context) {
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

    pub(crate) fn prepare_manual_data_window(&mut self) {
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

    pub(crate) fn manual_data_fields(
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

    pub(crate) fn insert_manual_data(&mut self) -> Result<(), String> {
        let reset_axis_labels = !self.document.series().iter().any(|series| {
            series.effective_visible
                && series.binding.is_some()
                && matches!(
                    series.kind,
                    SeriesKind::Line | SeriesKind::Scatter | SeriesKind::ErrorBar
                )
        });
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
        let changed_source_ids = parsed_groups
            .iter()
            .map(|group| group.dataset.plot_id.clone())
            .collect::<BTreeSet<_>>();
        let mut affected_axes = Vec::new();
        for series in self.document.logical_series().into_iter().filter(|series| {
            series.effective_visible
                && series
                    .binding
                    .as_ref()
                    .is_some_and(|binding| changed_source_ids.contains(&binding.data_source_id))
        }) {
            if let Some(axes) = series.axes {
                extend_axis_identities(&mut affected_axes, axes);
            }
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
        if reset_axis_labels {
            document.set_axis_label(
                AxisDimension::X,
                vec![LabelNode::Text(first.series.x_column.clone())],
            )?;
            document.set_axis_label(
                AxisDimension::Y,
                vec![LabelNode::Text(first.series.y_column.clone())],
            )?;
        }
        for series in document.logical_series().into_iter().filter(|series| {
            series.effective_visible
                && series
                    .binding
                    .as_ref()
                    .is_some_and(|binding| changed_source_ids.contains(&binding.data_source_id))
        }) {
            if let Some(axes) = series.axes {
                extend_axis_identities(&mut affected_axes, axes);
            }
        }
        document.restore_autoscale_after_data_change(&affected_axes)?;
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
                affected_axes: affected_axes.clone(),
            },
        )
        .map_err(|error| error.diagnostics.join("；"))?;
        let effect = self.commit_app_outcome(outcome);
        let AppEffect::PreparedDataCommitted {
            selected_dataset,
            affected_axes,
        } = effect
        else {
            unreachable!("prepared data action must produce a prepared-data effect");
        };
        self.clear_axis_numeric_drafts(&affected_axes);
        self.sync_axis_editors();
        self.select_dataset(&selected_dataset);
        self.show_layers = true;
        self.canvas_scroll = egui::Vec2::ZERO;
        self.last_canvas_figure_center = None;
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

    pub(crate) fn data_removal_dialog(&mut self, context: &egui::Context) {
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
                                let AppEffect::RemovedData { affected_axes } = effect else {
                                    unreachable!("remove action must produce a removal effect");
                                };
                                self.clear_axis_numeric_drafts(&affected_axes);
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

    pub(crate) fn series_tree(&mut self, ui: &mut egui::Ui) {
        self.data_source_controls(ui);
    }

    pub(crate) fn data_source_controls(&mut self, ui: &mut egui::Ui) {
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

    pub(crate) fn column_controls_for_group(
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
}
