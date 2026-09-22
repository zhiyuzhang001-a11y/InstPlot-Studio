use eframe::egui::{Painter, Pos2};
use studio_render_spike::DisplayList;
use ui_shell_spike::{ScreenTransform, paint_display_list};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreviewMetrics {
    pub framebuffer_width: u32,
    pub framebuffer_height: u32,
}

pub trait PreviewAdapter {
    fn paint(
        &self,
        painter: &Painter,
        display: &DisplayList,
        origin: Pos2,
        canvas_zoom: f32,
        pixels_per_point: f32,
    ) -> PreviewMetrics;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct EguiPreviewAdapter;

impl PreviewAdapter for EguiPreviewAdapter {
    fn paint(
        &self,
        painter: &Painter,
        display: &DisplayList,
        origin: Pos2,
        canvas_zoom: f32,
        pixels_per_point: f32,
    ) -> PreviewMetrics {
        let transform = ScreenTransform::new(origin, canvas_zoom, pixels_per_point);
        paint_display_list(painter, display, transform);
        let [framebuffer_width, framebuffer_height] =
            transform.framebuffer_size(display.width, display.height);
        PreviewMetrics {
            framebuffer_width,
            framebuffer_height,
        }
    }
}
