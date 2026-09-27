#[cfg(test)]
use eframe::egui;

pub(super) use instplot_ui::{
    card_frame as studio_card_frame, close_button_sized as studio_close_button_sized,
    configure_interface_style, dialog_heading as studio_dialog_heading,
    file_card_frame as studio_file_card_frame, label_format_button as studio_label_format_button,
    side_frame as studio_side_frame, surface as studio_surface,
    symbol_button as studio_symbol_button,
};

#[cfg(test)]
use super::SIDEBAR_CLOSE_BUTTON_SIZE;

#[cfg(test)]
pub(super) fn studio_close_button(ui: &mut egui::Ui, tooltip: &str) -> egui::Response {
    studio_close_button_sized(ui, tooltip, SIDEBAR_CLOSE_BUTTON_SIZE)
}
