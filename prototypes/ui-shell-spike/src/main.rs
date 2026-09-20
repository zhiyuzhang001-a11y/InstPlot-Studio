use std::path::PathBuf;
use std::time::Instant;

use eframe::egui;
use studio_render_spike::{DisplayList, compile, fixed_figure, to_svg};
use ui_shell_spike::{ScreenTransform, paint_display_list};

fn main() -> eframe::Result {
    if let Some(path) = argument_path("--headless-svg") {
        let display = display_list();
        std::fs::write(path, to_svg(&display).svg).expect("write headless SVG");
        return Ok(());
    }
    #[cfg(feature = "publication-stack")]
    if let Some(path) = argument_path("--publication-pdf") {
        let resolved = export_backend_spike::resolve(&display_list());
        let pdf = export_backend_spike::to_pdf(&resolved).expect("render publication PDF");
        std::fs::write(path, pdf).expect("write publication PDF");
        return Ok(());
    }

    let started = Instant::now();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("SciPlot Studio Shell")
            .with_inner_size([960.0, 680.0])
            .with_min_inner_size([720.0, 480.0]),
        ..Default::default()
    };
    eframe::run_native(
        "sciplot-studio-shell",
        options,
        Box::new(move |creation| Ok(Box::new(StudioShell::new(creation, started)))),
    )
}

fn argument_path(expected: &str) -> Option<PathBuf> {
    let mut arguments = std::env::args_os();
    arguments.next();
    match (arguments.next(), arguments.next()) {
        (Some(flag), Some(path)) if flag == expected => Some(path.into()),
        _ => None,
    }
}

fn display_list() -> DisplayList {
    compile(&fixed_figure()).expect("compile fixed A1 display list")
}

struct StudioShell {
    display: DisplayList,
    canvas_zoom: f32,
    note: String,
    selected_file: Option<PathBuf>,
    first_frame: bool,
    started: Instant,
}

impl StudioShell {
    fn new(creation: &eframe::CreationContext<'_>, started: Instant) -> Self {
        creation.egui_ctx.options_mut(|options| {
            options.zoom_with_keyboard = true;
        });
        Self {
            display: display_list(),
            canvas_zoom: 1.5,
            note: "Keyboard input probe: μ₀ 温度".to_owned(),
            selected_file: None,
            first_frame: true,
            started,
        }
    }

    fn open_file(&mut self) {
        self.selected_file = rfd::FileDialog::new()
            .set_title("Open SciPlot data")
            .add_filter("Data", &["csv", "tsv", "xlsx"])
            .pick_file();
    }
}

impl eframe::App for StudioShell {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let context = ui.ctx().clone();
        if context.input_mut(|input| {
            input.consume_shortcut(&egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND,
                egui::Key::O,
            ))
        }) {
            self.open_file();
        }

        egui::Panel::top("toolbar").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                if ui.button("Open…  ⌘O").clicked() {
                    self.open_file();
                }
                ui.separator();
                ui.label("SciPlot Studio shell — A6 validation");
            });
        });

        egui::Panel::right("inspector")
            .default_size(240.0)
            .show(ui, |ui| {
                ui.heading("Inspector");
                ui.label("Placeholder — edits Figure Document only");
                ui.add(egui::Slider::new(&mut self.canvas_zoom, 0.5..=3.0).text("Canvas zoom"));
                ui.label("Text input");
                ui.text_edit_singleline(&mut self.note);
                ui.separator();
                ui.label(format!(
                    "UI pixels/point: {:.2}",
                    context.pixels_per_point()
                ));
                ui.label(format!("Canvas zoom: {:.2}×", self.canvas_zoom));
                if let Some(path) = &self.selected_file {
                    ui.label(format!("Selected: {}", path.display()));
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
            let transform =
                ScreenTransform::new(origin, self.canvas_zoom, context.pixels_per_point());
            paint_display_list(ui.painter(), &self.display, transform);
            let framebuffer = transform.framebuffer_size(self.display.width, self.display.height);
            ui.painter().text(
                figure_rect.left_bottom() + egui::vec2(0.0, 20.0),
                egui::Align2::LEFT_TOP,
                format!(
                    "preview framebuffer: {} × {} px",
                    framebuffer[0], framebuffer[1]
                ),
                egui::FontId::monospace(12.0),
                ui.visuals().text_color(),
            );
        });

        if self.first_frame {
            self.first_frame = false;
            eprintln!(
                "FIRST_CANVAS elapsed_ms={} pixels_per_point={:.3} canvas_zoom={:.2}",
                self.started.elapsed().as_millis(),
                context.pixels_per_point(),
                self.canvas_zoom
            );
        }
    }
}
