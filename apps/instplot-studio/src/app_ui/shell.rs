use super::*;

impl eframe::App for StudioApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let context = ui.ctx().clone();
        if self.update.is_launching() {
            if context.input(|input| input.viewport().close_requested()) {
                context.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            }
            ui.spinner();
            ui.label("更新助手准备中，当前工作已锁定；失败时会返回原窗口。");
            self.update.window(&context, &[]);
            if self.update.take_close_request() {
                self.allow_close = true;
                context.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            return;
        }
        #[cfg(target_os = "macos")]
        if self.update_health.is_some() {
            // Do not consume shortcuts, file drops, native open-file events or
            // editor input until the updater confirms initialization succeeded.
            ui.disable();
            self.show_canvas(ui, &context);
            match self.update_health.as_mut().unwrap().frame_ready(&context) {
                Ok(true) => {
                    self.update_health = None;
                }
                Ok(false) => {}
                Err(error) => {
                    eprintln!("Update initialization failed: {error}");
                    context.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            return;
        }
        #[cfg(all(windows, feature = "in-place-update-preview"))]
        if self.update_health.is_some() {
            use instplot_studio::update_windows::WindowsHealthFrame;
            // Same first-canvas gate as macOS: no shortcuts, file drops or
            // editor/configuration changes until durable helper commitment.
            ui.disable();
            self.show_canvas(ui, &context);
            match self.update_health.as_mut().unwrap().first_canvas_ready() {
                Ok(WindowsHealthFrame::Committed) => self.update_health = None,
                Ok(WindowsHealthFrame::Pending) => {
                    context.request_repaint_after(std::time::Duration::from_millis(100));
                }
                Ok(WindowsHealthFrame::StopRequested) => {
                    context.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Err(error) => {
                    eprintln!("Update initialization failed: {error}");
                    context.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            return;
        }
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
            self.reference_draft = None;
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
                                        self.reference_draft = None;
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
                                            self.reference_draft = None;
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
                        .width(86.0)
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
                        self.reference_draft = None;
                        self.tool_draft = None;
                        self.drawing_tool = DrawingTool::Select;
                        if self.execute_document_edit(
                            EditCommand::SetAxisMode(selected_axis_mode),
                            "切换坐标轴模式",
                        ) {
                            self.clear_all_axis_numeric_drafts();
                        }
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
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(self.branding.footer_label()).weak(),
                            )
                            .frame(false),
                        )
                        .on_hover_text(self.language.text(Text::CheckUpdates))
                        .clicked()
                    {
                        self.update.request_check(&context);
                    }
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

        self.show_canvas(ui, &context);

        self.publication_check_window(&context);
        self.palette_window(&context);
        self.axis_visibility_window(&context);
        self.reference_draft_window(&context);
        self.context_editor(&context);
        self.manual_data_window(&context);
        let update_drafts = if self.update.needs_work_inventory() {
            self.pending_update_drafts()
        } else {
            Vec::new()
        };
        self.update.window(&context, &update_drafts);
        if self.update.take_restart_request() {
            let drafts = self.pending_update_drafts();
            if drafts.is_empty() {
                self.request_replacement(PendingAction::RestartForUpdate);
            } else {
                self.update.explain_blocked(drafts.join("\n"));
            }
        }
        if self.update.take_close_request() {
            self.allow_close = true;
            context.send_viewport_cmd(egui::ViewportCommand::Close);
        }

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
