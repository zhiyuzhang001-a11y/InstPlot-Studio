use super::*;

impl StudioApp {
    pub(crate) fn select_dataset(&mut self, data_source_id: &str) {
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

    pub(crate) fn select_series_for_editing(&mut self, series: &SeriesDescriptor) {
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

    pub(crate) fn open_context_editor(&mut self, target: CanvasHit) {
        let target = self.canonical_context_editor_target(target);
        if !self.context_editor_targets.contains(&target) {
            self.context_editor_targets.push(target.clone());
        }
        self.context_editor_focus_target = Some(target);
    }

    pub(crate) fn context_editor_target_exists(&self, target: &CanvasHit) -> bool {
        let selected_id = &target.project_id;
        self.document.artist_record(selected_id).is_some()
            || self
                .document
                .axis_identity_for_project_id(selected_id)
                .is_some()
            || self
                .document
                .project()
                .figure
                .axes
                .first()
                .is_some_and(|axes| axes.id == *selected_id)
            || self.document.semantic_label_nodes(selected_id).is_some()
    }

    pub(crate) fn prune_context_editor_targets(&mut self) {
        let targets = std::mem::take(&mut self.context_editor_targets);
        self.context_editor_targets = targets
            .into_iter()
            .filter(|target| self.context_editor_target_exists(target))
            .collect();
        if self
            .context_editor_focus_target
            .as_ref()
            .is_some_and(|target| !self.context_editor_target_exists(target))
        {
            self.context_editor_focus_target = None;
        }
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

    pub(crate) fn request_palette_window(&mut self) {
        self.show_palette = true;
        self.focus_palette = true;
    }

    pub(crate) fn request_publication_window(&mut self) {
        self.show_inspector = true;
        self.focus_inspector = true;
    }

    pub(crate) fn request_axis_visibility_window(&mut self) {
        self.show_axis_visibility = true;
        self.focus_axis_visibility = true;
    }

    fn axis_visibility_fields(&mut self, ui: &mut egui::Ui) {
        let identities: &[AxisIdentity] = match self.document.axis_mode() {
            AxisMode::Single => &[AxisIdentity::X1, AxisIdentity::Y1],
            AxisMode::DualX => &[AxisIdentity::X1, AxisIdentity::Y1, AxisIdentity::X2],
            AxisMode::DualY => &[AxisIdentity::X1, AxisIdentity::Y1, AxisIdentity::Y2],
        };
        if !identities.contains(&self.axis_visibility_identity) {
            self.axis_visibility_identity = AxisIdentity::X1;
        }
        let previous_identity = self.axis_visibility_identity;
        ui.horizontal(|ui| {
            ui.label(if self.language == UiLanguage::Chinese {
                "坐标轴"
            } else {
                "Axis"
            });
            egui::ComboBox::from_id_salt("axis-visibility-identity")
                .width(64.0)
                .selected_text(axis_title(self.language, self.axis_visibility_identity))
                .show_ui(ui, |ui| {
                    for identity in identities {
                        ui.selectable_value(
                            &mut self.axis_visibility_identity,
                            *identity,
                            axis_title(self.language, *identity),
                        );
                    }
                });
        });
        if self.axis_visibility_identity != previous_identity {
            self.clear_axis_numeric_drafts(&[previous_identity]);
        }
        ui.add_space(6.0);
        let identity = self.axis_visibility_identity;
        let Some(mut visibility) = self.document.axis_visibility(identity) else {
            return;
        };
        let before = visibility;
        ui.strong(if self.language == UiLanguage::Chinese {
            "显示"
        } else {
            "Visibility"
        });
        ui.add_space(2.0);
        egui::Grid::new("axis-visibility-switches")
            .num_columns(2)
            .spacing([22.0, 8.0])
            .show(ui, |ui| {
                ui.checkbox(
                    &mut visibility.spine,
                    if self.language == UiLanguage::Chinese {
                        "轴线"
                    } else {
                        "Spine"
                    },
                );
                ui.checkbox(
                    &mut visibility.ticks,
                    if self.language == UiLanguage::Chinese {
                        "刻度线"
                    } else {
                        "Ticks"
                    },
                );
                ui.end_row();
                ui.checkbox(
                    &mut visibility.tick_labels,
                    if self.language == UiLanguage::Chinese {
                        "刻度数字"
                    } else {
                        "Tick labels"
                    },
                );
                ui.checkbox(
                    &mut visibility.label,
                    if self.language == UiLanguage::Chinese {
                        "轴标签"
                    } else {
                        "Axis label"
                    },
                );
                ui.end_row();
            });
        if visibility != before {
            self.execute_document_edit(
                EditCommand::SetAxisVisibility {
                    identity,
                    visibility,
                },
                if self.language == UiLanguage::Chinese {
                    "坐标轴显示已更新"
                } else {
                    "Axis visibility updated"
                },
            );
        }
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(5.0);
        self.axis_quick_editor_by_identity(ui, identity);
    }

    pub(crate) fn axis_visibility_window(&mut self, context: &egui::Context) {
        if !self.show_axis_visibility {
            return;
        }
        let title = if self.language == UiLanguage::Chinese {
            "坐标轴显示"
        } else {
            "Axis visibility"
        };
        let embedded_id = egui::Id::new("axis-visibility-window");
        let viewport_id = egui::ViewportId::from_hash_of("axis-visibility-viewport");
        if std::mem::take(&mut self.focus_axis_visibility) {
            instplot_ui::ToolWindowPolicy::raise(context, viewport_id, embedded_id);
        }
        let spec = instplot_ui::ToolWindowSpec::new([460.0, 480.0], [380.0, 300.0]);
        let draw_fields = |this: &mut Self, ui: &mut egui::Ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| this.axis_visibility_fields(ui));
        };
        if instplot_ui::ToolWindowPolicy::mode(context) == instplot_ui::ToolWindowMode::Embedded {
            let mut open = self.show_axis_visibility;
            spec.embedded(title, embedded_id)
                .open(&mut open)
                .frame(studio_card_frame(context.theme() == egui::Theme::Dark))
                .show(context, |ui| draw_fields(self, ui));
            self.show_axis_visibility = open;
            if !open {
                self.clear_axis_numeric_drafts(&[self.axis_visibility_identity]);
            }
        } else {
            let builder = spec.viewport(title);
            let close_requested =
                context.show_viewport_immediate(viewport_id, builder, |ui, _class| {
                    let child_context = ui.ctx().clone();
                    let close_requested = instplot_ui::viewport_close_requested(&child_context);
                    egui::CentralPanel::default()
                        .frame(
                            egui::Frame::new()
                                .fill(studio_surface(child_context.theme() == egui::Theme::Dark))
                                .inner_margin(egui::Margin::same(14)),
                        )
                        .show(ui, |ui| draw_fields(self, ui));
                    close_requested
                });
            if close_requested {
                self.show_axis_visibility = false;
                self.clear_axis_numeric_drafts(&[self.axis_visibility_identity]);
            }
        }
    }

    pub(crate) fn annotation_artist_for_label(&self, label_id: &str) -> Option<String> {
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

    pub(crate) fn measurement_artist_for_label(&self, label_id: &str) -> Option<String> {
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

    pub(crate) fn delete_empty_annotation_on_close(&mut self, artist_id: &str) {
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

    pub(crate) fn add_text_annotation(&mut self) {
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
}
