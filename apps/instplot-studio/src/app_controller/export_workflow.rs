use super::*;

impl StudioApp {
    pub(crate) fn export_manual_data(&mut self) {
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

    pub(crate) fn export_pdf(&mut self) {
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

    pub(crate) fn export_pdf_to(&mut self, path: &Path) {
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

    pub(crate) fn export_png(&mut self) {
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

    pub(crate) fn export_svg(&mut self) {
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

    pub(crate) fn export_dialog(&mut self, context: &egui::Context) {
        let Some(kind) = self.pending_export else {
            return;
        };
        let mut open = true;
        let mut close_requested = false;
        let mut confirm = false;
        let mut switch_to_single_and_confirm = false;
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
                let empty_secondary = self.document.empty_active_secondary_axis();
                if let Some(identity) = empty_secondary {
                    let axis = match identity {
                        AxisIdentity::X2 => "X2",
                        AxisIdentity::Y2 => "Y2",
                        AxisIdentity::X1 | AxisIdentity::Y1 => unreachable!(),
                    };
                    ui.colored_label(
                        egui::Color32::YELLOW,
                        match self.language {
                            UiLanguage::Chinese => {
                                format!("{axis} 已启用，但没有绑定可见曲线。建议改为单轴后导出。")
                            }
                            UiLanguage::English => format!(
                                "{axis} is enabled but has no visible bound series. Switch to single-axis mode before export."
                            ),
                        },
                    );
                }
                ui.horizontal_wrapped(|ui| {
                    if empty_secondary.is_some() {
                        if ui
                            .button(match self.language {
                                UiLanguage::Chinese => "改为单轴并导出",
                                UiLanguage::English => "Switch to single axis and export",
                            })
                            .clicked()
                        {
                            switch_to_single_and_confirm = true;
                        }
                        if ui.button(self.language.text(Text::ExportAnyway)).clicked() {
                            confirm = true;
                        }
                    } else {
                        if ui
                            .button(export_confirmation_text(
                                self.language,
                                self.publication_report.error_count(),
                            ))
                            .clicked()
                        {
                            confirm = true;
                        }
                    }
                    let cancel_text = if empty_secondary.is_some() {
                        match self.language {
                            UiLanguage::Chinese => "返回检查",
                            UiLanguage::English => "Return to inspect",
                        }
                    } else {
                        self.language.text(Text::Cancel)
                    };
                    if ui.button(cancel_text).clicked() {
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
        if switch_to_single_and_confirm {
            if self.execute_document_edit(
                EditCommand::SetAxisMode(AxisMode::Single),
                match self.language {
                    UiLanguage::Chinese => "导出前改为单轴",
                    UiLanguage::English => "Switch to single axis before export",
                },
            ) {
                self.pending_export = None;
                match kind {
                    ExportKind::Pdf => self.export_pdf(),
                    ExportKind::Png => self.export_png(),
                    ExportKind::Svg => self.export_svg(),
                }
            }
        } else if confirm {
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
}
