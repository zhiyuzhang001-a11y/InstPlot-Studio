use super::*;

impl eframe::App for StudioApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let context = ui.ctx().clone();
        let dropped_paths = context.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .map(|file| file.path().to_path_buf())
                .collect::<Vec<_>>()
        });
        if !dropped_paths.is_empty() {
            self.open_paths(dropped_paths);
        }
        #[cfg(target_os = "macos")]
        {
            let externally_opened_paths = self
                .macos_open_files
                .as_ref()
                .map(crate::macos_open_files::MacOpenFiles::drain)
                .unwrap_or_default();
            if !externally_opened_paths.is_empty() {
                self.open_paths(externally_opened_paths);
            }
        }
        let dirty = self.edit_history.is_dirty(&self.document);
        context.send_viewport_cmd(egui::ViewportCommand::Title(
            self.branding
                .window_title(self.workspace.display_name(), dirty),
        ));
        let features = self.features;
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
        if context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.drawing_tool = DrawingTool::Select;
            self.tool_draft = None;
        }

        egui::Panel::top("product_header")
            .frame(
                egui::Frame::new()
                    .fill(if context.theme() == egui::Theme::Dark {
                        egui::Color32::from_rgb(43, 47, 54)
                    } else {
                        egui::Color32::WHITE
                    })
                    .inner_margin(egui::Margin::symmetric(18, 10))
                    .stroke(egui::Stroke::new(
                        1.0,
                        if context.theme() == egui::Theme::Dark {
                            egui::Color32::from_rgb(62, 67, 75)
                        } else {
                            egui::Color32::from_rgb(224, 229, 235)
                        },
                    )),
            )
            .show(ui, |ui| {
                egui::MenuBar::new().ui(ui, |ui| {
                    ui.menu_button(self.language.text(Text::File), |ui| {
                        if ui.button(self.language.text(Text::NewProject)).clicked() {
                            ui.close();
                            self.request_replacement(PendingAction::NewProject);
                        }
                        ui.separator();
                        if features.data_import
                            && ui.button(self.language.text(Text::OpenData)).clicked()
                        {
                            ui.close();
                            self.open_data();
                        }
                        if ui.button(self.language.text(Text::OpenLite)).clicked() {
                            ui.close();
                            self.request_replacement(PendingAction::OpenLiteHandoff);
                        }
                        if features.project_files
                            && ui.button(self.language.text(Text::OpenProject)).clicked()
                        {
                            ui.close();
                            self.request_replacement(PendingAction::OpenProject);
                        }
                        ui.separator();
                        if features.project_files
                            && ui.button(self.language.text(Text::Save)).clicked()
                        {
                            ui.close();
                            self.save_project(false);
                        }
                        if features.project_files
                            && ui.button(self.language.text(Text::SaveAs)).clicked()
                        {
                            ui.close();
                            self.save_project(true);
                        }
                        ui.separator();
                        ui.menu_button(self.language.text(Text::Export), |ui| {
                            if features.data_import
                                && ui.button(self.language.text(Text::ExportData)).clicked()
                            {
                                ui.close();
                                self.export_manual_data();
                            }
                            if features.data_import {
                                ui.separator();
                            }
                            if features.export_pdf
                                && ui.button(self.language.text(Text::ExportPdf)).clicked()
                            {
                                ui.close();
                                self.pending_export = Some(ExportKind::Pdf);
                            }
                            if features.export_png
                                && ui.button(self.language.text(Text::ExportPng)).clicked()
                            {
                                ui.close();
                                self.pending_export = Some(ExportKind::Png);
                            }
                            if features.export_svg
                                && ui.button(self.language.text(Text::ExportSvg)).clicked()
                            {
                                ui.close();
                                self.pending_export = Some(ExportKind::Svg);
                            }
                        });
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
                    if features.annotations
                        && ui
                            .add_enabled(
                                self.edit_history.undo_description().is_some(),
                                egui::Button::new(self.language.text(Text::Undo)),
                            )
                            .clicked()
                    {
                        self.undo();
                    }
                    if ui
                        .add_enabled(
                            self.edit_history.redo_description().is_some(),
                            egui::Button::new(self.language.text(Text::Redo)),
                        )
                        .clicked()
                    {
                        self.redo();
                    }
                    if ui
                        .button(self.language.text(Text::InsertAnnotation))
                        .clicked()
                    {
                        self.add_text_annotation();
                    }
                    ui.menu_button(
                        if matches!(self.drawing_tool, DrawingTool::Reference { .. }) {
                            "参考线 ●"
                        } else {
                            "参考线"
                        },
                        |ui| {
                            let mode = self.document.axis_mode();
                            let mut choose =
                                |ui: &mut egui::Ui,
                                 label: &str,
                                 orientation: ReferenceOrientation,
                                 axes: AxisBinding| {
                                    if ui.button(label).clicked() {
                                        let candidate =
                                            DrawingTool::Reference { orientation, axes };
                                        self.drawing_tool = if self.drawing_tool == candidate {
                                            DrawingTool::Select
                                        } else {
                                            candidate
                                        };
                                        self.tool_draft = None;
                                        ui.close();
                                    }
                                };
                            choose(
                                ui,
                                "竖直 · X1",
                                ReferenceOrientation::Vertical,
                                AxisBinding::PRIMARY,
                            );
                            if mode == AxisMode::DualX {
                                choose(
                                    ui,
                                    "竖直 · X2",
                                    ReferenceOrientation::Vertical,
                                    AxisBinding {
                                        x: XAxisSlot::X2,
                                        y: YAxisSlot::Y1,
                                    },
                                );
                            }
                            choose(
                                ui,
                                "水平 · Y1",
                                ReferenceOrientation::Horizontal,
                                AxisBinding::PRIMARY,
                            );
                            if mode == AxisMode::DualY {
                                choose(
                                    ui,
                                    "水平 · Y2",
                                    ReferenceOrientation::Horizontal,
                                    AxisBinding {
                                        x: XAxisSlot::X1,
                                        y: YAxisSlot::Y2,
                                    },
                                );
                            }
                        },
                    );
                    ui.menu_button(
                        if matches!(self.drawing_tool, DrawingTool::Measurement { .. }) {
                            "测量箭头 ●"
                        } else {
                            "测量箭头"
                        },
                        |ui| {
                            let mut bindings = vec![("主轴 · X1 / Y1", AxisBinding::PRIMARY)];
                            match self.document.axis_mode() {
                                AxisMode::DualX => bindings.push((
                                    "副轴 · X2 / Y1",
                                    AxisBinding {
                                        x: XAxisSlot::X2,
                                        y: YAxisSlot::Y1,
                                    },
                                )),
                                AxisMode::DualY => bindings.push((
                                    "副轴 · X1 / Y2",
                                    AxisBinding {
                                        x: XAxisSlot::X1,
                                        y: YAxisSlot::Y2,
                                    },
                                )),
                                AxisMode::Single => {}
                            }
                            for (binding_label, binding) in bindings {
                                ui.menu_button(binding_label, |ui| {
                                    for (label, constraint, start_arrow, end_arrow) in [
                                        ("普通指示", MeasurementConstraint::Free, false, true),
                                        ("水平双向", MeasurementConstraint::Horizontal, true, true),
                                        ("垂直双向", MeasurementConstraint::Vertical, true, true),
                                    ] {
                                        if ui.button(label).clicked() {
                                            let candidate = DrawingTool::Measurement {
                                                axes: binding,
                                                constraint,
                                                start_arrow,
                                                end_arrow,
                                            };
                                            self.drawing_tool = if self.drawing_tool == candidate {
                                                DrawingTool::Select
                                            } else {
                                                candidate
                                            };
                                            self.tool_draft = None;
                                            ui.close();
                                        }
                                    }
                                });
                            }
                        },
                    );
                    if features.manual_data
                        && ui.button(self.language.text(Text::EnterData)).clicked()
                    {
                        self.prepare_manual_data_window();
                    }
                    let current_axis_mode = self.document.axis_mode();
                    let mut selected_axis_mode = current_axis_mode;
                    egui::ComboBox::from_id_salt("axis-mode-selector")
                        .selected_text(match current_axis_mode {
                            AxisMode::Single => "单轴",
                            AxisMode::DualY => "双 Y 轴",
                            AxisMode::DualX => "双 X 轴",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut selected_axis_mode, AxisMode::Single, "单轴");
                            ui.selectable_value(
                                &mut selected_axis_mode,
                                AxisMode::DualY,
                                "双 Y 轴",
                            );
                            ui.selectable_value(
                                &mut selected_axis_mode,
                                AxisMode::DualX,
                                "双 X 轴",
                            );
                        });
                    if selected_axis_mode != current_axis_mode {
                        self.execute_document_edit(
                            EditCommand::SetAxisMode(selected_axis_mode),
                            "切换坐标轴模式",
                        );
                        self.sync_axis_editors();
                    }
                    if ui
                        .selectable_label(self.show_palette, self.language.text(Text::ColorScheme))
                        .clicked()
                    {
                        self.request_palette_window();
                    }
                    let check_count = self.publication_report.error_count()
                        + self.publication_report.warning_count();
                    let check_label = if check_count == 0 {
                        self.language.text(Text::PublicationCheck).to_owned()
                    } else {
                        format!(
                            "{} ({check_count})",
                            self.language.text(Text::PublicationCheck)
                        )
                    };
                    if features.publication_check
                        && ui
                            .selectable_label(self.show_inspector, check_label)
                            .clicked()
                    {
                        self.request_publication_window();
                    }
                    if !self.messages.is_empty()
                        && ui
                            .selectable_label(
                                self.show_messages,
                                format!(
                                    "{} ({})",
                                    self.language.text(Text::WarningsAndErrors),
                                    self.messages.len()
                                ),
                            )
                            .clicked()
                    {
                        self.show_messages = !self.show_messages;
                    }
                });
            });

        egui::Panel::bottom("product_footer")
            .frame(
                egui::Frame::new()
                    .fill(studio_surface(context.theme() == egui::Theme::Dark))
                    .inner_margin(egui::Margin::symmetric(14, 7)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.weak(self.branding.footer_label());
                    if let Some(coordinates) = self.hover_data_coordinates {
                        ui.separator();
                        let mut text = format!(
                            "X1: {}   Y1: {}",
                            format_data_coordinate(coordinates.x1),
                            format_data_coordinate(coordinates.y1)
                        );
                        if let Some(x2) = coordinates.x2 {
                            text.push_str(&format!("   X2: {}", format_data_coordinate(x2)));
                        }
                        if let Some(y2) = coordinates.y2 {
                            text.push_str(&format!("   Y2: {}", format_data_coordinate(y2)));
                        }
                        ui.monospace(text);
                    }
                    if let Some((status, _)) = &self.status {
                        ui.separator();
                        ui.add(egui::Label::new(status.as_str()).truncate());
                    }
                });
            });

        if self.show_layers {
            egui::Panel::left("series_tree")
                .resizable(true)
                .show_separator_line(true)
                .default_size(DATA_SIDEBAR_DEFAULT_WIDTH)
                .min_size(DATA_SIDEBAR_MIN_WIDTH)
                .max_size(DATA_SIDEBAR_MAX_WIDTH)
                .frame(studio_side_frame(context.theme() == egui::Theme::Dark))
                .show(ui, |ui| {
                    ui.style_mut().spacing.scroll = egui::style::ScrollStyle::solid();
                    let scroll_width = ui.available_width();
                    let scroll_height = ui.available_height();
                    egui::ScrollArea::both()
                        .auto_shrink([false, false])
                        .max_width(scroll_width)
                        .max_height(scroll_height)
                        .scroll_bar_visibility(
                            egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded,
                        )
                        .show(ui, |ui| {
                            ui.set_width(DATA_SIDEBAR_CONTENT_MIN_WIDTH);
                            self.series_tree(ui);
                        });
                });
        }

        if self.show_messages && !self.messages.is_empty() {
            egui::Panel::bottom("warning_panel")
                .default_size(90.0)
                .frame(studio_side_frame(context.theme() == egui::Theme::Dark))
                .show(ui, |ui| {
                    ui.heading(self.language.text(Text::WarningsAndErrors));
                    egui::ScrollArea::vertical().show(ui, |ui| {
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
                    });
                });
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(studio_surface(context.theme() == egui::Theme::Dark)))
            .show(ui, |ui| {
                let viewport = ui.available_size();
                if self.first_frame {
                    self.fit_canvas(viewport);
                }
                let old_zoom = self.canvas_zoom;
                let drawing_tool = self.drawing_tool;
                let output = egui::ScrollArea::both()
                    .id_salt("figure-canvas-scroll")
                    .scroll_offset(self.canvas_scroll)
                    .scroll_source(egui::scroll_area::ScrollSource::SCROLL_BAR)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let figure_size =
                            egui::vec2(self.resolved.display.width, self.resolved.display.height)
                                * old_zoom;
                        let content_size = egui::vec2(
                            (figure_size.x + 64.0).max(viewport.x),
                            (figure_size.y + 84.0).max(viewport.y),
                        );
                        let (content_rect, _) =
                            ui.allocate_exact_size(content_size, egui::Sense::hover());
                        let origin = content_rect.min
                            + egui::vec2(
                                ((content_size.x - figure_size.x) * 0.5).max(32.0),
                                ((content_size.y - figure_size.y) * 0.5).max(32.0),
                            );
                        let figure_rect = egui::Rect::from_min_size(origin, figure_size);
                        let response = ui.interact(
                            figure_rect.expand(14.0),
                            ui.id().with("figure-canvas"),
                            egui::Sense::click_and_drag(),
                        );
                        ui.painter().rect_filled(
                            figure_rect.expand(6.0).translate(egui::vec2(0.0, 4.0)),
                            4.0,
                            egui::Color32::from_black_alpha(
                                if context.theme() == egui::Theme::Dark {
                                    38
                                } else {
                                    18
                                },
                            ),
                        );
                        ui.painter()
                            .rect_filled(figure_rect, 0.0, egui::Color32::WHITE);
                        ui.painter().rect_stroke(
                            figure_rect,
                            0.0,
                            egui::Stroke::new(1.0, egui::Color32::GRAY),
                            egui::StrokeKind::Outside,
                        );
                        self.preview.paint(
                            ui.painter(),
                            &self.resolved.display,
                            origin,
                            old_zoom,
                            context.pixels_per_point(),
                        );
                        if let Some(draft) = self.tool_draft
                            && let Some((start, end)) = data_point_to_local(
                                &self.document,
                                self.resolved.layout.result.axes,
                                draft.axes,
                                draft.start,
                            )
                            .zip(data_point_to_local(
                                &self.document,
                                self.resolved.layout.result.axes,
                                draft.axes,
                                draft.end,
                            ))
                        {
                            let start =
                                origin + egui::vec2(start.0 as f32, start.1 as f32) * old_zoom;
                            let end = origin + egui::vec2(end.0 as f32, end.1 as f32) * old_zoom;
                            let stroke = egui::Stroke::new(1.5, egui::Color32::BLACK);
                            ui.painter().line_segment([start, end], stroke);
                            ui.painter().circle_filled(start, 3.0, egui::Color32::BLACK);
                            ui.painter().circle_filled(end, 3.0, egui::Color32::BLACK);
                        }

                        if let Some(selected) = self.selected_canvas_node.as_deref() {
                            let dragging_legend = self
                                .active_artist_drag
                                .as_ref()
                                .filter(|drag| drag.id == selected && drag.legend.is_some());
                            let path_highlighted = dragging_legend.is_none() && {
                                let mut painted = false;
                                for linked in self.document.linked_series_ids(selected) {
                                    painted |= paint_object_path_overlay(
                                        ui.painter(),
                                        &self.resolved,
                                        &linked,
                                        self.selected_canvas_role,
                                        origin,
                                        old_zoom,
                                        true,
                                    );
                                }
                                painted
                            };
                            let selection_bounds =
                                dragging_legend.map(|drag| drag.preview_bounds).or_else(|| {
                                    selected_hit_bounds_for_role(
                                        &self.resolved,
                                        selected,
                                        self.selected_canvas_role,
                                    )
                                });
                            if !path_highlighted && let Some(bounds) = selection_bounds {
                                let overlay = egui::Rect::from_min_max(
                                    origin
                                        + egui::vec2(bounds.0 as f32, bounds.1 as f32) * old_zoom,
                                    origin
                                        + egui::vec2(bounds.2 as f32, bounds.3 as f32) * old_zoom,
                                )
                                .expand(4.0);
                                let accent = egui::Color32::from_rgb(56, 145, 225);
                                ui.painter().rect_filled(
                                    overlay,
                                    4.0,
                                    egui::Color32::from_rgba_unmultiplied(56, 145, 225, 10),
                                );
                                ui.painter().rect_stroke(
                                    overlay.expand(2.0),
                                    5.0,
                                    egui::Stroke::new(
                                        4.0,
                                        egui::Color32::from_rgba_unmultiplied(56, 145, 225, 45),
                                    ),
                                    egui::StrokeKind::Outside,
                                );
                                ui.painter().rect_stroke(
                                    overlay,
                                    4.0,
                                    egui::Stroke::new(1.8, accent),
                                    egui::StrokeKind::Outside,
                                );
                                if matches!(
                                    self.selected_canvas_role,
                                    Some(SelectableRole::Legend | SelectableRole::Annotation)
                                ) {
                                    for corner in [
                                        overlay.left_top(),
                                        overlay.right_top(),
                                        overlay.left_bottom(),
                                        overlay.right_bottom(),
                                    ] {
                                        ui.painter().circle_filled(corner, 3.0, accent);
                                    }
                                }
                                if self.selected_canvas_role == Some(SelectableRole::Legend) {
                                    for center in [overlay.right_center(), overlay.center_bottom()]
                                    {
                                        ui.painter().rect_filled(
                                            egui::Rect::from_center_size(
                                                center,
                                                egui::vec2(10.0, 10.0),
                                            ),
                                            2.0,
                                            egui::Color32::WHITE,
                                        );
                                        ui.painter().rect_stroke(
                                            egui::Rect::from_center_size(
                                                center,
                                                egui::vec2(10.0, 10.0),
                                            ),
                                            2.0,
                                            egui::Stroke::new(1.5, accent),
                                            egui::StrokeKind::Outside,
                                        );
                                    }
                                    if let Some(drag) = dragging_legend {
                                        let target = drag
                                            .candidate_placement
                                            .unwrap_or(LegendPlacement::Inside);
                                        let mode = match drag.mode {
                                            ArtistDragMode::Move => match target {
                                                LegendPlacement::Above => {
                                                    self.language.text(Text::LegendAbove)
                                                }
                                                LegendPlacement::Right => {
                                                    self.language.text(Text::LegendRight)
                                                }
                                                _ => self.language.text(Text::LegendInside),
                                            }
                                            .to_owned(),
                                            ArtistDragMode::ResizeColumns
                                            | ArtistDragMode::ResizeRows => {
                                                let context =
                                                    drag.legend.expect("legend drag context");
                                                let columns = ((bounds.2 - bounds.0)
                                                    / context.cell_width)
                                                    .round()
                                                    .max(1.0)
                                                    as usize;
                                                format!(
                                                    "{columns} × {}",
                                                    context.entries.div_ceil(columns)
                                                )
                                            }
                                        };
                                        ui.painter().text(
                                            overlay.left_top() + egui::vec2(6.0, -7.0),
                                            egui::Align2::LEFT_BOTTOM,
                                            mode,
                                            egui::FontId::proportional(12.0),
                                            accent,
                                        );
                                    }
                                }
                            }
                            if self.active_artist_drag.as_ref().is_some_and(|drag| {
                                drag.id == selected && drag.role == SelectableRole::ReferenceLine
                            }) && let Some(record) = self.document.artist_record(selected)
                                && let ArtistProperties::ReferenceLine { value, .. } =
                                    record.properties
                                && let Some(bounds) = selected_hit_bounds_for_role(
                                    &self.resolved,
                                    selected,
                                    Some(SelectableRole::ReferenceLine),
                                )
                            {
                                let anchor = origin
                                    + egui::vec2(bounds.0 as f32, bounds.1 as f32) * old_zoom
                                    + egui::vec2(7.0, -7.0);
                                ui.painter().text(
                                    anchor,
                                    egui::Align2::LEFT_BOTTOM,
                                    format!("{value:.6}"),
                                    egui::FontId::monospace(12.0),
                                    egui::Color32::from_rgb(56, 145, 225),
                                );
                            }
                        }

                        let quick_drag =
                            context.input(|input| same_frame_primary_drag(&input.raw.events));
                        let pointer = quick_drag
                            .map(|(_, end)| end)
                            .or_else(|| response.interact_pointer_pos())
                            .or_else(|| context.input(|input| input.pointer.interact_pos()))
                            .or_else(|| context.input(|input| input.pointer.latest_pos()));
                        let hovered = pointer
                            .and_then(|point| hit_project(&self.resolved, point, origin, old_zoom));
                        if let Some(hovered) = hovered.as_ref() {
                            ui.output_mut(|output| {
                                output.cursor_icon = egui::CursorIcon::PointingHand
                            });
                            if (self.selected_canvas_node.as_deref()
                                != Some(hovered.project_id.as_str())
                                || self.selected_canvas_role != Some(hovered.role))
                                && !paint_object_path_overlay(
                                    ui.painter(),
                                    &self.resolved,
                                    &hovered.project_id,
                                    Some(hovered.role),
                                    origin,
                                    old_zoom,
                                    false,
                                )
                                && let Some(bounds) = selected_hit_bounds_for_role(
                                    &self.resolved,
                                    &hovered.project_id,
                                    Some(hovered.role),
                                )
                            {
                                let overlay = egui::Rect::from_min_max(
                                    origin
                                        + egui::vec2(bounds.0 as f32, bounds.1 as f32) * old_zoom,
                                    origin
                                        + egui::vec2(bounds.2 as f32, bounds.3 as f32) * old_zoom,
                                )
                                .expand(3.0);
                                ui.painter().rect_stroke(
                                    overlay,
                                    3.0,
                                    egui::Stroke::new(1.5, egui::Color32::from_rgb(76, 157, 255)),
                                    egui::StrokeKind::Outside,
                                );
                            }
                        }
                        let clicked =
                            (quick_drag.is_none() && response.clicked()).then(|| hovered.clone());
                        let edit_requested =
                            quick_drag.is_none() && response.double_clicked() && hovered.is_some();
                        let mut drag_event = None;
                        let active_handle =
                            if self.selected_canvas_role == Some(SelectableRole::Legend) {
                                self.selected_canvas_node
                                    .as_deref()
                                    .and_then(|selected| {
                                        selected_hit_bounds_for_role(
                                            &self.resolved,
                                            selected,
                                            Some(SelectableRole::Legend),
                                        )
                                    })
                                    .and_then(|bounds| {
                                        pointer.and_then(|point| {
                                            legend_resize_handle_at(point, bounds, origin, old_zoom)
                                        })
                                    })
                            } else {
                                None
                            };
                        if let Some(handle) = active_handle {
                            ui.output_mut(|output| {
                                output.cursor_icon = match handle {
                                    ArtistDragMode::ResizeColumns => {
                                        egui::CursorIcon::ResizeHorizontal
                                    }
                                    ArtistDragMode::ResizeRows => egui::CursorIcon::ResizeVertical,
                                    ArtistDragMode::Move => egui::CursorIcon::Grab,
                                }
                            });
                        }
                        if active_handle.is_none()
                            && matches!(
                                hovered.as_ref().map(|hit| hit.role),
                                Some(
                                    SelectableRole::Legend
                                        | SelectableRole::Annotation
                                        | SelectableRole::ReferenceLine
                                        | SelectableRole::MeasurementArrow
                                        | SelectableRole::MeasurementArrowStart
                                        | SelectableRole::MeasurementArrowEnd
                                        | SelectableRole::MeasurementArrowLabel,
                                )
                            )
                        {
                            ui.output_mut(|output| output.cursor_icon = egui::CursorIcon::Grab);
                        }
                        if drawing_tool == DrawingTool::Select
                            && (response.drag_started()
                                || (response.drag_stopped() && self.active_artist_drag.is_none())
                                || (quick_drag.is_some() && self.active_artist_drag.is_none()))
                        {
                            let press_origin = quick_drag.map(|(start, _)| start).or_else(|| {
                                context.input(|input| {
                                    input.pointer.press_origin().or_else(|| {
                                        input.raw.events.iter().find_map(|event| {
                                            if let egui::Event::PointerButton {
                                                pos,
                                                button: egui::PointerButton::Primary,
                                                pressed: true,
                                                ..
                                            } = event
                                            {
                                                Some(*pos)
                                            } else {
                                                None
                                            }
                                        })
                                    })
                                })
                            });
                            let handle_hit = press_origin.and_then(|point| {
                                self.selected_canvas_node.as_deref().and_then(|selected| {
                                    selected_hit_bounds_for_role(
                                        &self.resolved,
                                        selected,
                                        Some(SelectableRole::Legend),
                                    )
                                    .and_then(|bounds| {
                                        legend_resize_handle_at(point, bounds, origin, old_zoom)
                                            .map(|mode| {
                                                (
                                                    CanvasHit {
                                                        project_id: selected.to_owned(),
                                                        role: SelectableRole::Legend,
                                                        data_index: None,
                                                    },
                                                    mode,
                                                )
                                            })
                                    })
                                })
                            });
                            let pressed_hit = handle_hit.or_else(|| {
                                press_origin
                                    .and_then(|point| {
                                        hit_project(&self.resolved, point, origin, old_zoom)
                                    })
                                    .map(|hit| (hit, ArtistDragMode::Move))
                            });
                            if let Some((hit, mode)) = pressed_hit.filter(|hit| {
                                matches!(
                                    hit.0.role,
                                    SelectableRole::Legend
                                        | SelectableRole::Annotation
                                        | SelectableRole::AnnotationConnector
                                        | SelectableRole::ReferenceLine
                                        | SelectableRole::MeasurementArrow
                                        | SelectableRole::MeasurementArrowStart
                                        | SelectableRole::MeasurementArrowEnd
                                        | SelectableRole::MeasurementArrowLabel
                                )
                            }) && let Some(bounds) = selected_hit_bounds_for_role(
                                &self.resolved,
                                &hit.project_id,
                                Some(hit.role),
                            ) {
                                drag_event = Some(CanvasDragEvent {
                                    id: hit.project_id,
                                    role: hit.role,
                                    bounds,
                                    delta: response.drag_delta(),
                                    pointer: pointer.map(|point| {
                                        (
                                            f64::from((point.x - origin.x) / old_zoom),
                                            f64::from((point.y - origin.y) / old_zoom),
                                        )
                                    }),
                                    press_pointer: press_origin.map(|point| {
                                        (
                                            f64::from((point.x - origin.x) / old_zoom),
                                            f64::from((point.y - origin.y) / old_zoom),
                                        )
                                    }),
                                    mode,
                                    started: true,
                                    stopped: response.drag_stopped() || quick_drag.is_some(),
                                    data_index: hit.data_index,
                                    shift: context.input(|input| input.modifiers.shift),
                                });
                            }
                        } else if (response.dragged() || response.drag_stopped())
                            && let Some(drag) = self.active_artist_drag.as_ref()
                        {
                            drag_event = Some(CanvasDragEvent {
                                id: drag.id.clone(),
                                role: drag.role,
                                bounds: drag.bounds,
                                delta: response.drag_delta(),
                                pointer: pointer.map(|point| {
                                    (
                                        f64::from((point.x - origin.x) / old_zoom),
                                        f64::from((point.y - origin.y) / old_zoom),
                                    )
                                }),
                                press_pointer: None,
                                mode: drag.mode,
                                started: false,
                                stopped: response.drag_stopped(),
                                data_index: drag.connector_index,
                                shift: context.input(|input| input.modifiers.shift),
                            });
                        }
                        let fit_requested = response.double_clicked() && hovered.is_none();
                        let (wheel, pan, next_trackpad_scroll_active) = context.input(|input| {
                            let (trackpad, next_active) = trackpad_scroll_state(
                                &input.raw.events,
                                self.trackpad_scroll_active,
                            );
                            if !response.hovered() {
                                return (0.0, egui::Vec2::ZERO, next_active);
                            }
                            let delta = input.smooth_scroll_delta();
                            let pinch = input.zoom_delta();
                            let zoom = if (pinch - 1.0).abs() > f32::EPSILON {
                                pinch.ln() / 0.002
                            } else if trackpad {
                                0.0
                            } else {
                                delta.y
                            };
                            let pan = if trackpad { delta } else { egui::Vec2::ZERO };
                            (zoom, pan, next_active)
                        });
                        let zoom_anchor = (wheel.abs() > f32::EPSILON)
                            .then(|| pointer.map(|point| (point, origin, wheel)))
                            .flatten();
                        let candidates = if edit_requested {
                            pointer.map_or_else(Vec::new, |point| {
                                hit_project_candidates(&self.resolved, point, origin, old_zoom)
                            })
                        } else {
                            Vec::new()
                        };
                        let data_coordinates = pointer.and_then(|point| {
                            active_data_coordinates_at(
                                &self.resolved,
                                &self.document,
                                point,
                                origin,
                                old_zoom,
                            )
                        });
                        let press_screen = quick_drag
                            .map(|(start, _)| start)
                            .or_else(|| context.input(|input| input.pointer.press_origin()));
                        let shift = context.input(|input| input.modifiers.shift);
                        let tool_pointer = if shift {
                            press_screen
                                .zip(pointer)
                                .map(|(start, end)| snap_pointer_to_special_angle(start, end))
                                .or(pointer)
                        } else {
                            pointer
                        };
                        let tool_event =
                            (drawing_tool != DrawingTool::Select).then(|| CanvasToolEvent {
                                clicked: response.clicked(),
                                started: response.drag_started() || quick_drag.is_some(),
                                stopped: response.drag_stopped() || quick_drag.is_some(),
                                press: press_screen.and_then(|point| {
                                    active_data_coordinates_at(
                                        &self.resolved,
                                        &self.document,
                                        point,
                                        origin,
                                        old_zoom,
                                    )
                                }),
                                current: tool_pointer.and_then(|point| {
                                    active_data_coordinates_at(
                                        &self.resolved,
                                        &self.document,
                                        point,
                                        origin,
                                        old_zoom,
                                    )
                                }),
                                shift,
                            });
                        (
                            clicked,
                            zoom_anchor,
                            pointer,
                            fit_requested,
                            drag_event,
                            candidates,
                            edit_requested,
                            pan,
                            next_trackpad_scroll_active,
                            data_coordinates,
                            tool_event,
                        )
                    });
                self.canvas_scroll = output.state.offset;
                self.trackpad_scroll_active = output.inner.8;
                self.hover_data_coordinates = output.inner.9;
                if output.inner.7 != egui::Vec2::ZERO {
                    self.canvas_scroll =
                        (self.canvas_scroll - output.inner.7).max(egui::Vec2::ZERO);
                }
                if let Some(tool_event) = output.inner.10 {
                    self.handle_drawing_tool_event(tool_event);
                }
                if drawing_tool == DrawingTool::Select
                    && let Some(event) = output.inner.4
                {
                    self.handle_artist_drag(event, old_zoom);
                }
                if drawing_tool == DrawingTool::Select
                    && let Some(clicked) = output.inner.0
                {
                    self.selection_candidates = output.inner.5;
                    self.selected_canvas_node = clicked.as_ref().map(|hit| hit.project_id.clone());
                    self.selected_canvas_role = clicked.as_ref().map(|hit| hit.role);
                    if output.inner.6
                        && self.selection_candidates.len() <= 1
                        && let Some(clicked) = clicked.clone()
                    {
                        self.open_context_editor(clicked);
                    }
                    self.selected_series = clicked.as_ref().and_then(|hit| {
                        self.document
                            .series()
                            .into_iter()
                            .find(|series| series.id == hit.project_id)
                            .map(|series| series.id)
                    });
                    if let Some(selected) = self.selected_series.clone()
                        && let Some(series) = self
                            .document
                            .series()
                            .into_iter()
                            .find(|series| series.id == selected)
                    {
                        self.select_series_for_editing(&series);
                    }
                }
                if let Some((pointer, origin, wheel)) = output.inner.1 {
                    (self.canvas_scroll, self.canvas_zoom) =
                        zoom_about_pointer(self.canvas_scroll, pointer, origin, old_zoom, wheel);
                }
                if output.inner.3 {
                    self.fit_canvas(viewport);
                }
            });

        self.publication_check_window(&context);
        self.palette_window(&context);
        self.context_editor(&context);
        self.manual_data_window(&context);

        if self.first_frame {
            self.first_frame = false;
            eprintln!(
                "FIRST_CANVAS product=instplot-studio elapsed_ms={} pixels_per_point={:.3}",
                self.started.elapsed().as_millis(),
                context.pixels_per_point()
            );
        }
        self.unsaved_dialog(&context);
        self.managed_save_conflict_dialog(&context);
        self.data_removal_dialog(&context);
        self.export_dialog(&context);
    }
}
