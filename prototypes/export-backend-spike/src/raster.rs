use std::io::Cursor;
#[cfg(feature = "comparison-raster")]
use std::sync::Arc;

use studio_render_spike::{DisplayItem, FillRule, LineCap, LineJoin, Path, PathVerb};
use tiny_skia::{
    Color, ColorU8, FillRule as SkFillRule, IntSize, LineCap as SkLineCap, LineJoin as SkLineJoin,
    Mask, Paint, Path as SkPath, PathBuilder, Pixmap, PixmapPaint, Stroke as SkStroke, StrokeDash,
    Transform,
};
use ttf_parser::{Face, GlyphId, OutlineBuilder};

#[cfg(feature = "comparison-raster")]
use crate::to_svg;
use crate::{RasterAsset, ResolvedDisplayList, ResolvedItem, ResolvedText};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Background {
    White,
    Transparent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RasterImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub dpi: u32,
}

#[cfg(feature = "comparison-raster")]
pub fn rasterize_via_svg(
    list: &ResolvedDisplayList,
    dpi: u32,
    background: Background,
) -> RasterImage {
    let (width, height, mut pixmap) = empty_pixmap(list, dpi, background);
    let mut database = resvg::usvg::fontdb::Database::new();
    database.load_system_fonts();
    let mut options = resvg::usvg::Options::default();
    options.fontdb = Arc::new(database);
    let svg = to_svg(list);
    let tree = resvg::usvg::Tree::from_str(&svg, &options).expect("backend-generated SVG");
    let scale = dpi as f32 / 96.0;
    resvg::render(
        &tree,
        Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    RasterImage {
        width,
        height,
        rgba: straight_rgba(&pixmap),
        dpi,
    }
}

pub fn rasterize_direct(
    list: &ResolvedDisplayList,
    dpi: u32,
    background: Background,
) -> RasterImage {
    let (width, height, mut pixmap) = empty_pixmap(list, dpi, background);
    let scale = dpi as f32 / 72.0;
    let transform = Transform::from_scale(scale, scale);
    let mut clips: Vec<Mask> = Vec::new();

    for item in &list.items {
        match item {
            ResolvedItem::Text(text) => draw_text(&mut pixmap, text, scale, clips.last()),
            ResolvedItem::Graphics(DisplayItem::Path {
                path, fill, stroke, ..
            }) => {
                let Some(path) = sk_path(path) else { continue };
                if let Some(fill) = fill {
                    let mut paint = Paint::default();
                    paint.set_color_rgba8(fill.color.0, fill.color.1, fill.color.2, fill.color.3);
                    paint.anti_alias = true;
                    pixmap.fill_path(
                        &path,
                        &paint,
                        match fill.rule {
                            FillRule::NonZero => SkFillRule::Winding,
                            FillRule::EvenOdd => SkFillRule::EvenOdd,
                        },
                        transform,
                        clips.last(),
                    );
                }
                if let Some(stroke) = stroke {
                    let mut paint = Paint::default();
                    paint.set_color_rgba8(
                        stroke.color.0,
                        stroke.color.1,
                        stroke.color.2,
                        stroke.color.3,
                    );
                    paint.anti_alias = true;
                    let sk_stroke = SkStroke {
                        width: stroke.width.get() as f32,
                        miter_limit: 10.0,
                        line_cap: match stroke.cap {
                            LineCap::Butt => SkLineCap::Butt,
                            LineCap::Round => SkLineCap::Round,
                            LineCap::Square => SkLineCap::Square,
                        },
                        line_join: match stroke.join {
                            LineJoin::Miter => SkLineJoin::Miter,
                            LineJoin::Round => SkLineJoin::Round,
                            LineJoin::Bevel => SkLineJoin::Bevel,
                        },
                        dash: (!stroke.dash.is_empty()).then(|| {
                            StrokeDash::new(
                                stroke.dash.iter().map(|value| value.get() as f32).collect(),
                                0.0,
                            )
                            .expect("validated dash array")
                        }),
                    };
                    pixmap.stroke_path(&path, &paint, &sk_stroke, transform, clips.last());
                }
            }
            ResolvedItem::Graphics(DisplayItem::ClipPush {
                x,
                y,
                width: clip_width,
                height: clip_height,
                ..
            }) => {
                let rect = tiny_skia::Rect::from_xywh(
                    x.get() as f32,
                    y.get() as f32,
                    clip_width.get() as f32,
                    clip_height.get() as f32,
                )
                .unwrap();
                let mut mask = Mask::new(width, height).unwrap();
                mask.fill_path(
                    &PathBuilder::from_rect(rect),
                    SkFillRule::Winding,
                    true,
                    transform,
                );
                debug_assert!(mask.data().iter().any(|value| *value != 0));
                if let Some(parent) = clips.last() {
                    for (value, parent_value) in mask.data_mut().iter_mut().zip(parent.data()) {
                        *value = ((*value as u16 * *parent_value as u16) / 255) as u8;
                    }
                }
                clips.push(mask);
            }
            ResolvedItem::Graphics(DisplayItem::ClipPop { .. }) => {
                clips.pop();
            }
            ResolvedItem::Graphics(DisplayItem::Image(image)) => {
                if let Some(asset) = list.resources.get(&image.resource_id)
                    && let Some(source) = asset_pixmap(asset)
                {
                    let transform = Transform::from_row(
                        image.width.get() as f32 * scale / asset.width as f32,
                        0.0,
                        0.0,
                        image.height.get() as f32 * scale / asset.height as f32,
                        image.x.get() as f32 * scale,
                        image.y.get() as f32 * scale,
                    );
                    pixmap.draw_pixmap(
                        0,
                        0,
                        source.as_ref(),
                        &PixmapPaint::default(),
                        transform,
                        clips.last(),
                    );
                }
            }
            ResolvedItem::Graphics(DisplayItem::GlyphRun(_)) => unreachable!(),
        }
    }

    RasterImage {
        width,
        height,
        rgba: straight_rgba(&pixmap),
        dpi,
    }
}

pub fn encode_png(image: &RasterImage) -> Result<Vec<u8>, png::EncodingError> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(Cursor::new(&mut bytes), image.width, image.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let pixels_per_meter = (image.dpi as f64 / 0.0254).round() as u32;
        encoder.set_pixel_dims(Some(png::PixelDimensions {
            xppu: pixels_per_meter,
            yppu: pixels_per_meter,
            unit: png::Unit::Meter,
        }));
        encoder.add_text_chunk("Software".into(), "SciPlot export-backend-spike".into())?;
        encoder.add_text_chunk("DPI".into(), image.dpi.to_string())?;
        let mut writer = encoder.write_header()?;
        writer.write_image_data(&image.rgba)?;
    }
    Ok(bytes)
}

pub(crate) fn encode_asset_png(asset: &RasterAsset) -> Result<Vec<u8>, png::EncodingError> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(Cursor::new(&mut bytes), asset.width, asset.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(&asset.rgba)?;
    }
    Ok(bytes)
}

fn empty_pixmap(
    list: &ResolvedDisplayList,
    dpi: u32,
    background: Background,
) -> (u32, u32, Pixmap) {
    let width = round_half_away(list.width as f64 * dpi as f64 / 72.0);
    let height = round_half_away(list.height as f64 * dpi as f64 / 72.0);
    let mut pixmap = Pixmap::new(width, height).expect("validated raster dimensions");
    if background == Background::White {
        pixmap.fill(Color::WHITE);
    }
    (width, height, pixmap)
}

fn draw_text(pixmap: &mut Pixmap, text: &ResolvedText, scale: f32, clip: Option<&Mask>) {
    let mut paint = Paint::default();
    paint.set_color_rgba8(text.color.0, text.color.1, text.color.2, text.color.3);
    paint.anti_alias = true;
    for run in &text.runs {
        let Ok(face) = Face::parse(run.font_data.as_slice(), run.font_index) else {
            continue;
        };
        let font_scale = text.size / face.units_per_em() as f32;
        let mut cursor_x = text.x + run.start_x;
        for glyph in &run.glyphs {
            let mut builder = GlyphPathBuilder::new(
                cursor_x + glyph.x_offset,
                text.y - glyph.y_offset,
                font_scale,
                text.rotation_degrees,
                text.x,
                text.y,
                scale,
            );
            if face
                .outline_glyph(GlyphId(glyph.id as u16), &mut builder)
                .is_some()
                && let Some(path) = builder.finish()
            {
                pixmap.fill_path(
                    &path,
                    &paint,
                    SkFillRule::Winding,
                    Transform::identity(),
                    clip,
                );
            }
            cursor_x += glyph.advance;
        }
    }
}

fn sk_path(path: &Path) -> Option<SkPath> {
    let mut builder = PathBuilder::new();
    for verb in &path.verbs {
        match *verb {
            PathVerb::MoveTo(x, y) => builder.move_to(x.get() as f32, y.get() as f32),
            PathVerb::LineTo(x, y) => builder.line_to(x.get() as f32, y.get() as f32),
            PathVerb::CurveTo(x1, y1, x2, y2, x3, y3) => builder.cubic_to(
                x1.get() as f32,
                y1.get() as f32,
                x2.get() as f32,
                y2.get() as f32,
                x3.get() as f32,
                y3.get() as f32,
            ),
            PathVerb::Close => builder.close(),
        }
    }
    builder.finish()
}

struct GlyphPathBuilder {
    path: PathBuilder,
    origin_x: f32,
    baseline_y: f32,
    font_scale: f32,
    sin: f32,
    cos: f32,
    pivot_x: f32,
    pivot_y: f32,
    raster_scale: f32,
}

impl GlyphPathBuilder {
    #[allow(clippy::too_many_arguments)]
    fn new(
        origin_x: f32,
        baseline_y: f32,
        font_scale: f32,
        rotation_degrees: f32,
        pivot_x: f32,
        pivot_y: f32,
        raster_scale: f32,
    ) -> Self {
        let angle = rotation_degrees.to_radians();
        Self {
            path: PathBuilder::new(),
            origin_x,
            baseline_y,
            font_scale,
            sin: angle.sin(),
            cos: angle.cos(),
            pivot_x,
            pivot_y,
            raster_scale,
        }
    }

    fn point(&self, x: f32, y: f32) -> (f32, f32) {
        let x = self.origin_x + x * self.font_scale;
        let y = self.baseline_y - y * self.font_scale;
        let dx = x - self.pivot_x;
        let dy = y - self.pivot_y;
        (
            (self.pivot_x + dx * self.cos - dy * self.sin) * self.raster_scale,
            (self.pivot_y + dx * self.sin + dy * self.cos) * self.raster_scale,
        )
    }

    fn finish(self) -> Option<SkPath> {
        self.path.finish()
    }
}

impl OutlineBuilder for GlyphPathBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.point(x, y);
        self.path.move_to(x, y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.point(x, y);
        self.path.line_to(x, y);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (x1, y1) = self.point(x1, y1);
        let (x, y) = self.point(x, y);
        self.path.quad_to(x1, y1, x, y);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (x1, y1) = self.point(x1, y1);
        let (x2, y2) = self.point(x2, y2);
        let (x, y) = self.point(x, y);
        self.path.cubic_to(x1, y1, x2, y2, x, y);
    }

    fn close(&mut self) {
        self.path.close();
    }
}

fn straight_rgba(pixmap: &Pixmap) -> Vec<u8> {
    let mut rgba = Vec::with_capacity((pixmap.width() * pixmap.height() * 4) as usize);
    for pixel in pixmap.pixels() {
        let pixel = pixel.demultiply();
        rgba.extend_from_slice(&[pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()]);
    }
    rgba
}

fn asset_pixmap(asset: &RasterAsset) -> Option<Pixmap> {
    if asset.rgba.len() != (asset.width * asset.height * 4) as usize {
        return None;
    }
    let mut premultiplied = Vec::with_capacity(asset.rgba.len());
    for pixel in asset.rgba.chunks_exact(4) {
        let value = ColorU8::from_rgba(pixel[0], pixel[1], pixel[2], pixel[3]).premultiply();
        premultiplied.extend_from_slice(&[value.red(), value.green(), value.blue(), value.alpha()]);
    }
    Pixmap::from_vec(premultiplied, IntSize::from_wh(asset.width, asset.height)?)
}

fn round_half_away(value: f64) -> u32 {
    (value + 0.5).floor() as u32
}
