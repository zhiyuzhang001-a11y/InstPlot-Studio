use std::error::Error;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Instant;

use eframe::egui;
use instplot_studio::{
    AxisRanges, EguiPreviewAdapter, FigureDocument, PRODUCT_NAME, PreviewAdapter, SeriesDescriptor,
    StudioSession, product_info, save_fixed_figure_pdf,
};
use studio_render_spike::DisplayList;

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
        StartupCommand::Gui => launch_gui().map_err(Into::into),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum StartupCommand {
    Gui,
    ProductInfo,
    ExportFixedPdf(PathBuf),
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
            _ => Err("usage: instplot-studio [--product-info | --export-fixed-pdf PATH]"),
        }
    }
}

fn launch_gui() -> eframe::Result {
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
        Box::new(move |creation| Ok(Box::new(StudioApp::new(creation, started)))),
    )
}

struct StudioApp {
    session: StudioSession,
    document: FigureDocument,
    display: DisplayList,
    preview: EguiPreviewAdapter,
    selected_series: Option<u64>,
    canvas_zoom: f32,
    warnings: Vec<String>,
    status: String,
    first_frame: bool,
    started: Instant,
}

impl StudioApp {
    fn new(creation: &eframe::CreationContext<'_>, started: Instant) -> Self {
        creation.egui_ctx.options_mut(|options| {
            options.zoom_with_keyboard = true;
        });
        let document = FigureDocument::fixed();
        let display = document
            .compile()
            .expect("the validated B1 fixed Figure Document compiles");
        Self {
            session: StudioSession::default(),
            document,
            display,
            preview: EguiPreviewAdapter,
            selected_series: None,
            canvas_zoom: 1.5,
            warnings: Vec::new(),
            status: "Ready".to_owned(),
            first_frame: true,
            started,
        }
    }

    fn open_data(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Open data in InstPlot Studio")
            .add_filter(
                "Supported data",
                &["txt", "csv", "dat", "tsv", "xlsx", "xls"],
            )
            .pick_file()
        else {
            return;
        };
        match self.session.import_data_file(&path) {
            Ok(outcome) => {
                self.status = format!(
                    "Imported {} dataset(s): {} added, {} replaced",
                    outcome.read, outcome.added, outcome.replaced
                );
                self.clear_warning("Import failed:");
            }
            Err(error) => self.push_warning(format!("Import failed: {error}")),
        }
    }

    fn export_fixed_pdf(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Export fixed publication figure")
            .add_filter("PDF", &["pdf"])
            .set_file_name("instplot-studio-fixed.pdf")
            .save_file()
        else {
            return;
        };
        self.export_fixed_pdf_to(&path);
    }

    fn export_fixed_pdf_to(&mut self, path: &Path) {
        match save_fixed_figure_pdf(path) {
            Ok(size) => {
                self.status = format!("Exported {} bytes to {}", size, path.display());
                self.clear_warning("Export failed:");
            }
            Err(error) => self.push_warning(format!("Export failed: {error}")),
        }
    }

    fn apply_ranges(&mut self, ranges: AxisRanges) {
        match self.document.set_axis_ranges(ranges) {
            Ok(()) => match self.document.compile() {
                Ok(display) => {
                    self.display = display;
                    self.status = "Figure Document updated".to_owned();
                    self.clear_warning("Layout failed:");
                    self.clear_warning("Invalid axes:");
                }
                Err(error) => self.push_warning(format!("Layout failed: {error}")),
            },
            Err(error) => self.push_warning(format!("Invalid axes: {error}")),
        }
    }

    fn push_warning(&mut self, warning: String) {
        if !self.warnings.contains(&warning) {
            self.warnings.push(warning.clone());
        }
        self.status = warning;
    }

    fn clear_warning(&mut self, prefix: &str) {
        self.warnings.retain(|warning| !warning.starts_with(prefix));
    }

    fn series_tree(&mut self, ui: &mut egui::Ui) {
        ui.heading("Series");
        ui.collapsing("Fixed publication figure", |ui| {
            for series in self.document.series() {
                let selected = self.selected_series == Some(series.id.0);
                if ui.selectable_label(selected, &series.label).clicked() {
                    self.selected_series = Some(series.id.0);
                }
            }
        });
        ui.separator();
        ui.heading("Data");
        if self.session.datasets().is_empty() {
            ui.weak("No imported datasets");
        } else {
            for dataset in self.session.datasets() {
                ui.label(dataset.display_name());
                ui.weak(format!(
                    "{} rows · {} columns",
                    dataset.row_count,
                    dataset.columns.len()
                ));
            }
        }
    }

    fn inspector(&mut self, ui: &mut egui::Ui) {
        ui.heading("Inspector");
        let series = self.selected_series.and_then(|id| {
            self.document
                .series()
                .into_iter()
                .find(|series| series.id.0 == id)
        });
        describe_selection(ui, series.as_ref());

        ui.separator();
        ui.label("Axes ranges");
        let mut ranges = self.document.axis_ranges();
        let mut changed = false;
        egui::Grid::new("axis_ranges")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("X min");
                changed |= ui.add(egui::DragValue::new(&mut ranges.x_min)).changed();
                ui.end_row();
                ui.label("X max");
                changed |= ui.add(egui::DragValue::new(&mut ranges.x_max)).changed();
                ui.end_row();
                ui.label("Y min");
                changed |= ui.add(egui::DragValue::new(&mut ranges.y_min)).changed();
                ui.end_row();
                ui.label("Y max");
                changed |= ui.add(egui::DragValue::new(&mut ranges.y_max)).changed();
                ui.end_row();
            });
        if changed {
            self.apply_ranges(ranges);
        }

        ui.separator();
        ui.label("View only");
        ui.add(egui::Slider::new(&mut self.canvas_zoom, 0.5..=3.0).text("Canvas zoom"));
        ui.weak("Canvas zoom is not stored in the Figure Document.");
    }
}

impl eframe::App for StudioApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let context = ui.ctx().clone();
        if context.input_mut(|input| {
            input.consume_shortcut(&egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND,
                egui::Key::O,
            ))
        }) {
            self.open_data();
        }

        egui::Panel::top("product_header").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.heading(PRODUCT_NAME);
                ui.separator();
                if ui.button("Open data…").clicked() {
                    self.open_data();
                }
                if ui.button("Export fixed PDF…").clicked() {
                    self.export_fixed_pdf();
                }
                ui.separator();
                ui.label(&self.status);
            });
        });

        egui::Panel::left("series_tree")
            .default_size(230.0)
            .show(ui, |ui| self.series_tree(ui));

        egui::Panel::right("inspector")
            .default_size(260.0)
            .show(ui, |ui| self.inspector(ui));

        egui::Panel::bottom("warning_panel")
            .default_size(90.0)
            .show(ui, |ui| {
                ui.heading("Warnings");
                if self.warnings.is_empty() {
                    ui.weak("No warnings");
                } else {
                    for warning in &self.warnings {
                        ui.colored_label(egui::Color32::LIGHT_RED, warning);
                    }
                }
            });

        egui::CentralPanel::default().show(ui, |ui| {
            let available = ui.available_size();
            let figure_size = egui::vec2(
                self.display.width.get() as f32,
                self.display.height.get() as f32,
            ) * self.canvas_zoom;
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
                &self.display,
                origin,
                self.canvas_zoom,
                context.pixels_per_point(),
            );
            ui.painter().text(
                figure_rect.left_bottom() + egui::vec2(0.0, 18.0),
                egui::Align2::LEFT_TOP,
                format!(
                    "preview framebuffer: {} × {} px",
                    metrics.framebuffer_width, metrics.framebuffer_height
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
    }
}

fn describe_selection(ui: &mut egui::Ui, series: Option<&SeriesDescriptor>) {
    if let Some(series) = series {
        ui.label(&series.label);
        ui.weak(format!("Kind: {}", series.kind));
        ui.weak(format!("Stable node: {}", series.id.0));
    } else {
        ui.weak("Select a series to inspect it.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(StartupCommand::parse([OsString::from("--unknown")]).is_err());
    }
}
