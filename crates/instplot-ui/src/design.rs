use eframe::egui;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InterfaceMetrics {
    pub small_text: f32,
    pub body_text: f32,
    pub button_text: f32,
    pub heading_text: f32,
    pub control_height: f32,
    pub item_gap_x: f32,
    pub item_gap_y: f32,
}

impl InterfaceMetrics {
    pub const STUDIO: Self = Self {
        small_text: 15.0,
        body_text: 16.0,
        button_text: 16.0,
        heading_text: 22.0,
        control_height: 40.0,
        item_gap_x: 9.0,
        item_gap_y: 12.0,
    };
}

pub fn configure_interface_style(context: &egui::Context) {
    let metrics = InterfaceMetrics::STUDIO;
    context.all_styles_mut(|style| {
        use egui::{FontFamily, FontId, TextStyle};
        for (text_style, size, family) in [
            (
                TextStyle::Small,
                metrics.small_text,
                FontFamily::Proportional,
            ),
            (TextStyle::Body, metrics.body_text, FontFamily::Proportional),
            (
                TextStyle::Button,
                metrics.button_text,
                FontFamily::Proportional,
            ),
            (TextStyle::Monospace, 14.0, FontFamily::Monospace),
            (
                TextStyle::Heading,
                metrics.heading_text,
                FontFamily::Proportional,
            ),
        ] {
            style
                .text_styles
                .insert(text_style, FontId::new(size, family));
        }
        style.spacing.button_padding = egui::vec2(14.0, 9.0);
        style.spacing.interact_size.y = metrics.control_height;
        style.spacing.item_spacing = egui::vec2(metrics.item_gap_x, metrics.item_gap_y);
        style.spacing.extra_text_line_spacing = 4.0;
        style.spacing.window_margin = egui::Margin::symmetric(18, 16);
        style.spacing.menu_margin = egui::Margin::same(10);
        style.interaction.resize_grab_radius_side = 8.0;
        style.visuals.panel_fill = if style.visuals.dark_mode {
            egui::Color32::from_rgb(34, 38, 44)
        } else {
            egui::Color32::from_rgb(247, 248, 250)
        };
        style.visuals.window_fill = if style.visuals.dark_mode {
            egui::Color32::from_rgb(47, 51, 57)
        } else {
            egui::Color32::WHITE
        };
    });
}

pub fn surface(dark: bool) -> egui::Color32 {
    if dark {
        egui::Color32::from_rgb(37, 41, 47)
    } else {
        egui::Color32::from_rgb(242, 244, 247)
    }
}

pub fn card_frame(dark: bool) -> egui::Frame {
    egui::Frame::new()
        .fill(if dark {
            egui::Color32::from_rgb(48, 52, 59)
        } else {
            egui::Color32::WHITE
        })
        .stroke(egui::Stroke::new(
            1.0,
            if dark {
                egui::Color32::from_rgb(86, 92, 101)
            } else {
                egui::Color32::from_rgb(214, 220, 227)
            },
        ))
        .corner_radius(12)
        .inner_margin(egui::Margin::symmetric(18, 16))
        .shadow(egui::Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 0,
            color: egui::Color32::from_black_alpha(55),
        })
}

pub fn side_frame(dark: bool) -> egui::Frame {
    egui::Frame::new()
        .fill(if dark {
            egui::Color32::from_rgb(43, 47, 54)
        } else {
            egui::Color32::WHITE
        })
        .inner_margin(egui::Margin::symmetric(16, 14))
}

pub fn file_card_frame(dark: bool) -> egui::Frame {
    egui::Frame::new()
        .fill(if dark {
            egui::Color32::from_rgb(49, 54, 62)
        } else {
            egui::Color32::from_rgb(247, 249, 252)
        })
        .stroke(egui::Stroke::new(
            1.0,
            if dark {
                egui::Color32::from_rgb(67, 73, 83)
            } else {
                egui::Color32::from_rgb(222, 227, 234)
            },
        ))
        .corner_radius(7)
        .inner_margin(egui::Margin::symmetric(10, 9))
}

pub fn close_button_sized(ui: &mut egui::Ui, tooltip: &str, size: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
    let visuals = ui.style().interact(&response);
    ui.painter().rect(
        rect,
        3.0,
        visuals.bg_fill,
        visuals.bg_stroke,
        egui::StrokeKind::Inside,
    );
    let center = rect.center();
    let arm = 4.5;
    let stroke = egui::Stroke::new(1.6, visuals.fg_stroke.color);
    ui.painter().line_segment(
        [
            center + egui::vec2(-arm, -arm),
            center + egui::vec2(arm, arm),
        ],
        stroke,
    );
    ui.painter().line_segment(
        [
            center + egui::vec2(-arm, arm),
            center + egui::vec2(arm, -arm),
        ],
        stroke,
    );
    response.on_hover_text(tooltip)
}

pub fn dialog_heading(ui: &mut egui::Ui, title: &str) {
    ui.strong(title);
    ui.add_space(6.0);
    ui.separator();
    ui.add_space(6.0);
}

pub fn symbol_button(ui: &mut egui::Ui, symbol: &str, _greek: bool) -> egui::Response {
    let face = if matches!(symbol, "≤" | "≥") {
        "STIXTwoMath-Regular"
    } else {
        "TeXGyreHeros-Regular"
    };
    let label = egui::RichText::new(symbol).font(egui::FontId::new(
        20.0,
        egui::FontFamily::Name(std::sync::Arc::from(face)),
    ));
    ui.add_sized([40.0, 40.0], egui::Button::new(label))
}

pub fn label_format_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add_sized(
        [64.0, InterfaceMetrics::STUDIO.control_height],
        egui::Button::new(
            egui::RichText::new(label)
                .size(InterfaceMetrics::STUDIO.button_text)
                .line_height(Some(20.0)),
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_metrics_keep_controls_and_text_aligned() {
        let metrics = InterfaceMetrics::STUDIO;
        assert_eq!(metrics.control_height, 40.0);
        assert!(metrics.control_height > metrics.button_text * 2.0);
        assert!(metrics.item_gap_x < metrics.item_gap_y);
    }
}
