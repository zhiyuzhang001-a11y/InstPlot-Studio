use super::*;

impl StudioApp {
    pub(crate) fn inspector(&mut self, ui: &mut egui::Ui) {
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

    pub(crate) fn publication_check_window(&mut self, context: &egui::Context) {
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

    pub(crate) fn palette_window(&mut self, context: &egui::Context) {
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

    pub(crate) fn reference_draft_window(&mut self, context: &egui::Context) {
        let Some(mut draft) = self.reference_draft.clone() else {
            return;
        };
        let title = "添加参考线";
        let embedded_id = egui::Id::new("reference-draft-window");
        let viewport_id = egui::ViewportId::from_hash_of("reference-draft-viewport");
        if std::mem::take(&mut self.focus_reference_draft) {
            instplot_ui::ToolWindowPolicy::raise(context, viewport_id, embedded_id);
        }
        let window_spec = instplot_ui::ToolWindowSpec::new([440.0, 380.0], [380.0, 330.0]);
        let mut add = false;
        let mut add_and_continue = false;
        let mut cancel = false;
        let mut draw_fields = |ui: &mut egui::Ui| {
            let control_height = ui.spacing().interact_size.y;
            ui.horizontal_wrapped(|ui| {
                ui.label(self.language.text(Text::Orientation));
                egui::ComboBox::from_id_salt("reference-draft-orientation")
                    .selected_text(reference_orientation_name(self.language, draft.orientation))
                    .show_ui(ui, |ui| {
                        for candidate in [
                            ReferenceOrientation::Horizontal,
                            ReferenceOrientation::Vertical,
                        ] {
                            ui.selectable_value(
                                &mut draft.orientation,
                                candidate,
                                reference_orientation_name(self.language, candidate),
                            );
                        }
                    });
                ui.add_sized(
                    [120.0, control_height],
                    egui::DragValue::new(&mut draft.value)
                        .prefix(format!("{}: ", self.language.text(Text::Value))),
                );
            });
            reference_axis_binding_editor(
                ui,
                "reference-draft-axis-binding",
                self.document.axis_mode(),
                draft.orientation,
                &mut draft.axes,
            );
            ui.checkbox(&mut draft.include_in_autoscale, "纳入自动范围");
            let palette = self.document.palette_colors().to_vec();
            stroke_editor(
                ui,
                self.language,
                "reference-draft-stroke",
                &mut draft.stroke,
                &palette,
            );
            ui.separator();
            ui.horizontal(|ui| {
                add |= ui.button("添加").clicked();
                add_and_continue |= ui.button("添加并继续").clicked();
                cancel |= ui.button(self.language.text(Text::Cancel)).clicked();
            });
        };
        if instplot_ui::ToolWindowPolicy::mode(context) == instplot_ui::ToolWindowMode::Embedded {
            let mut open = true;
            window_spec
                .embedded(title, embedded_id)
                .open(&mut open)
                .frame(studio_card_frame(context.theme() == egui::Theme::Dark))
                .show(context, |ui| draw_fields(ui));
            cancel |= !open;
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
                        .show(ui, |ui| draw_fields(ui));
                    close_requested
                });
            cancel |= close_requested;
        }

        if add || add_and_continue {
            self.reference_draft = Some(draft);
            self.commit_reference_draft(add_and_continue);
        } else if cancel {
            self.cancel_reference_draft();
        } else {
            self.reference_draft = Some(draft);
        }
    }

    pub(crate) fn commit_reference_draft(&mut self, continue_adding: bool) -> bool {
        let Some(draft) = self.reference_draft.clone() else {
            return false;
        };
        let previous = self
            .document
            .project()
            .figure
            .artists
            .iter()
            .map(|artist| artist.id.clone())
            .collect::<BTreeSet<_>>();
        if !self.execute_document_edit(
            EditCommand::AddReferenceLine {
                orientation: draft.orientation,
                value: draft.value,
                axes: draft.axes,
                stroke: draft.stroke,
                include_in_autoscale: draft.include_in_autoscale,
            },
            "添加参考线",
        ) {
            return false;
        }
        if let Some(created) = self
            .document
            .project()
            .figure
            .artists
            .iter()
            .find(|artist| {
                artist.kind == instplot_studio::ArtistKind::ReferenceLine
                    && !previous.contains(&artist.id)
            })
            .map(|artist| artist.id.clone())
        {
            self.selected_canvas_node = Some(created);
            self.selected_canvas_role = Some(SelectableRole::ReferenceLine);
            self.selected_series = None;
        }
        self.reference_draft = None;
        self.drawing_tool = if continue_adding {
            DrawingTool::Reference {
                orientation: draft.orientation,
                axes: draft.axes,
            }
        } else {
            DrawingTool::Select
        };
        true
    }

    pub(crate) fn cancel_reference_draft(&mut self) {
        self.reference_draft = None;
        self.drawing_tool = DrawingTool::Select;
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
}
