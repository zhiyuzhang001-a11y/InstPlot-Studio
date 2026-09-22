use eframe::egui::{Painter, Pos2};
use export_backend_spike::ResolvedDisplayList;
use ui_shell_spike::{ScreenTransform, paint_resolved_display_list};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreviewMetrics {
    pub framebuffer_width: u32,
    pub framebuffer_height: u32,
}

pub trait PreviewAdapter {
    fn paint(
        &self,
        painter: &Painter,
        display: &ResolvedDisplayList,
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
        display: &ResolvedDisplayList,
        origin: Pos2,
        canvas_zoom: f32,
        pixels_per_point: f32,
    ) -> PreviewMetrics {
        let transform = ScreenTransform::new(origin, canvas_zoom, pixels_per_point);
        paint_resolved_display_list(painter, display, transform);
        let framebuffer_width = (display.width * canvas_zoom * pixels_per_point).round() as u32;
        let framebuffer_height = (display.height * canvas_zoom * pixels_per_point).round() as u32;
        PreviewMetrics {
            framebuffer_width,
            framebuffer_height,
        }
    }
}
