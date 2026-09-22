use eframe::egui;
use instplot_studio::{PRODUCT_NAME, StudioSession, product_info};

fn main() -> eframe::Result {
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "--product-info")
    {
        println!("{}", product_info());
        return Ok(());
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(PRODUCT_NAME)
            .with_inner_size([960.0, 680.0])
            .with_min_inner_size([720.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        "instplot-studio",
        options,
        Box::new(|creation| Ok(Box::new(StudioApp::new(creation)))),
    )
}

#[derive(Default)]
struct StudioApp {
    session: StudioSession,
}

impl StudioApp {
    fn new(creation: &eframe::CreationContext<'_>) -> Self {
        creation.egui_ctx.options_mut(|options| {
            options.zoom_with_keyboard = true;
        });
        Self::default()
    }
}

impl eframe::App for StudioApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("product_header").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading(PRODUCT_NAME);
                ui.separator();
                ui.label("Publication figure editor");
            });
        });

        egui::Panel::bottom("status_bar").show(ui, |ui| {
            ui.label("B1.1 · independent Studio application boundary");
        });

        egui::CentralPanel::default().show(ui, |ui| {
            ui.centered_and_justified(|ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("Application shell ready");
                    ui.label("Shared data import and Figure Document editing follow in B1.");
                    ui.label(format!("Datasets: {}", self.session.dataset_count()));
                });
            });
        });
    }
}
