use std::cell::RefCell;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

use eframe::egui::{self, Painter, Pos2};
use instplot_export::{
    Background, ResolvedDisplayList, ResolvedItem, checked_raster_dimensions, rasterize_direct,
};
use instplot_ui::{ScreenTransform, paint_resolved_graphics_only};

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

#[derive(Default)]
pub struct EguiPreviewAdapter {
    text_texture: RefCell<Option<CachedTextTexture>>,
}

struct CachedTextTexture {
    key: u64,
    handle: egui::TextureHandle,
}

impl EguiPreviewAdapter {
    fn text_texture(
        &self,
        painter: &Painter,
        display: &ResolvedDisplayList,
    ) -> Option<egui::TextureId> {
        let key = text_cache_key(display);
        let mut cache = self.text_texture.borrow_mut();
        if cache.as_ref().is_none_or(|cached| cached.key != key) {
            let text_display = ResolvedDisplayList {
                width: display.width,
                height: display.height,
                items: display
                    .items
                    .iter()
                    .filter(|item| matches!(item, ResolvedItem::Text(_)))
                    .cloned()
                    .collect(),
                resources: BTreeMap::new(),
                geometry: display.geometry,
            };
            let raster = [360, 240, 180, 120, 90]
                .into_iter()
                .filter(|&dpi| {
                    checked_raster_dimensions(display.width.into(), display.height.into(), dpi)
                        .is_ok_and(|(width, height)| {
                            u64::from(width) * u64::from(height) <= 16_000_000
                        })
                })
                .find_map(|dpi| {
                    rasterize_direct(&text_display, dpi, Background::Transparent).ok()
                })?;
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [raster.width as usize, raster.height as usize],
                &raster.rgba,
            );
            let handle = painter.ctx().load_texture(
                "studio-publication-text-preview",
                image,
                egui::TextureOptions::LINEAR,
            );
            *cache = Some(CachedTextTexture { key, handle });
        }
        cache.as_ref().map(|cached| cached.handle.id())
    }
}

fn text_cache_key(display: &ResolvedDisplayList) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    display.width.to_bits().hash(&mut hasher);
    display.height.to_bits().hash(&mut hasher);
    display
        .geometry
        .export_translation
        .0
        .to_bits()
        .hash(&mut hasher);
    display
        .geometry
        .export_translation
        .1
        .to_bits()
        .hash(&mut hasher);
    for item in &display.items {
        let ResolvedItem::Text(text) = item else {
            continue;
        };
        text.source.hash(&mut hasher);
        text.text.hash(&mut hasher);
        text.x.to_bits().hash(&mut hasher);
        text.y.to_bits().hash(&mut hasher);
        text.rotation_degrees.to_bits().hash(&mut hasher);
        [text.color.0, text.color.1, text.color.2, text.color.3].hash(&mut hasher);
        for run in &text.runs {
            run.font.postscript_name.hash(&mut hasher);
            run.font_size.to_bits().hash(&mut hasher);
            run.start_x.to_bits().hash(&mut hasher);
            run.baseline_shift.to_bits().hash(&mut hasher);
            for glyph in &run.glyphs {
                glyph.id.hash(&mut hasher);
                glyph.advance.to_bits().hash(&mut hasher);
                glyph.x_offset.to_bits().hash(&mut hasher);
                glyph.y_offset.to_bits().hash(&mut hasher);
            }
        }
    }
    hasher.finish()
}

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
        paint_resolved_graphics_only(painter, display, transform);
        let figure = egui::Rect::from_min_size(
            origin,
            egui::vec2(display.width, display.height) * canvas_zoom,
        );
        if let Some(texture) = self.text_texture(painter, display) {
            painter.image(
                texture,
                figure,
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
        let framebuffer_width = (display.width * canvas_zoom * pixels_per_point).round() as u32;
        let framebuffer_height = (display.height * canvas_zoom * pixels_per_point).round() as u32;
        PreviewMetrics {
            framebuffer_width,
            framebuffer_height,
        }
    }
}
