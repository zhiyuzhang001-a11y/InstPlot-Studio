use std::io::Cursor;
#[cfg(feature = "comparison-raster")]
use std::sync::Arc;
use std::{error::Error, fmt};

use instplot_render::{DisplayItem, FillRule, LineCap, LineJoin, Path, PathVerb};
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

pub const MAX_RASTER_PIXELS: u64 = 64_000_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RasterError {
    InvalidDimensions,
    TooLarge { width: u32, height: u32 },
    AllocationFailed { width: u32, height: u32 },
}

impl fmt::Display for RasterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDimensions => formatter.write_str("invalid raster size or DPI"),
            Self::TooLarge { width, height } => write!(
                formatter,
                "raster {width} × {height} px exceeds the {MAX_RASTER_PIXELS} pixel safety limit"
            ),
            Self::AllocationFailed { width, height } => {
                write!(formatter, "could not allocate raster {width} × {height} px")
            }
        }
    }
}

impl Error for RasterError {}

#[cfg(feature = "comparison-raster")]
pub fn rasterize_via_svg(
    list: &ResolvedDisplayList,
    dpi: u32,
    background: Background,
) -> Result<RasterImage, RasterError> {
    let (width, height, mut pixmap) = empty_pixmap(list, dpi, background)?;
    let mut database = resvg::usvg::fontdb::Database::new();
    for bytes in [
        include_bytes!("../../instplot-text/assets/fonts/TeXGyreHeros-Regular.otf").as_slice(),
        include_bytes!("../../instplot-text/assets/fonts/TeXGyreHeros-Italic.otf").as_slice(),
        include_bytes!("../../instplot-text/assets/fonts/TeXGyreHeros-Bold.otf").as_slice(),
        include_bytes!("../../instplot-text/assets/fonts/TeXGyreHeros-BoldItalic.otf").as_slice(),
    ] {
        database.load_font_data(bytes.to_vec());
    }
    let options = resvg::usvg::Options {
        fontdb: Arc::new(database),
        ..Default::default()
    };
    let svg = to_svg(list);
    let tree = resvg::usvg::Tree::from_str(&svg, &options).expect("backend-generated SVG");
    let scale = dpi as f32 / 96.0;
    resvg::render(
        &tree,
        Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    Ok(RasterImage {
        width,
        height,
        rgba: straight_rgba(&pixmap)?,
        dpi,
    })
}

pub fn rasterize_direct(
    list: &ResolvedDisplayList,
    dpi: u32,
    background: Background,
) -> Result<RasterImage, RasterError> {
    let (width, height, mut pixmap) = empty_pixmap(list, dpi, background)?;
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

    Ok(RasterImage {
        width,
        height,
        rgba: straight_rgba(&pixmap)?,
        dpi,
    })
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
        encoder.add_text_chunk("Software".into(), "InstPlot Studio".into())?;
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
) -> Result<(u32, u32, Pixmap), RasterError> {
    let (width, height) = checked_raster_dimensions(list.width as f64, list.height as f64, dpi)?;
    let mut pixmap =
        Pixmap::new(width, height).ok_or(RasterError::AllocationFailed { width, height })?;
    if background == Background::White {
        pixmap.fill(Color::WHITE);
    }
    Ok((width, height, pixmap))
}

pub fn checked_raster_dimensions(
    width_pt: f64,
    height_pt: f64,
    dpi: u32,
) -> Result<(u32, u32), RasterError> {
    if dpi == 0
        || !width_pt.is_finite()
        || !height_pt.is_finite()
        || width_pt <= 0.0
        || height_pt <= 0.0
    {
        return Err(RasterError::InvalidDimensions);
    }
    let width_px = width_pt * f64::from(dpi) / 72.0;
    let height_px = height_pt * f64::from(dpi) / 72.0;
    if !width_px.is_finite()
        || !height_px.is_finite()
        || width_px < 0.5
        || height_px < 0.5
        || width_px > f64::from(u32::MAX) - 0.5
        || height_px > f64::from(u32::MAX) - 0.5
    {
        return Err(RasterError::InvalidDimensions);
    }
    let width = round_half_away(width_px);
    let height = round_half_away(height_px);
    if u64::from(width) * u64::from(height) > MAX_RASTER_PIXELS {
        return Err(RasterError::TooLarge { width, height });
    }
    Ok((width, height))
}

fn draw_text(pixmap: &mut Pixmap, text: &ResolvedText, scale: f32, clip: Option<&Mask>) {
    let mut paint = Paint::default();
    paint.set_color_rgba8(text.color.0, text.color.1, text.color.2, text.color.3);
    paint.anti_alias = true;
    for run in &text.runs {
        let Ok(face) = Face::parse(run.font_data.as_slice(), run.font_index) else {
            continue;
        };
        let font_scale = run.font_size / face.units_per_em() as f32;
        let mut cursor_x = text.x + run.start_x;
        for glyph in &run.glyphs {
            let mut builder = GlyphPathBuilder::new(
                cursor_x + glyph.x_offset,
                text.y + run.baseline_shift - glyph.y_offset,
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

fn straight_rgba(pixmap: &Pixmap) -> Result<Vec<u8>, RasterError> {
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(pixmap.data().len())
        .map_err(|_| RasterError::AllocationFailed {
            width: pixmap.width(),
            height: pixmap.height(),
        })?;
    for pixel in pixmap.pixels() {
        let pixel = pixel.demultiply();
        rgba.extend_from_slice(&[pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()]);
    }
    Ok(rgba)
}

fn asset_pixmap(asset: &RasterAsset) -> Option<Pixmap> {
    if asset.rgba.len() != (asset.width * asset.height * 4) as usize {
        return None;
    }
    let mut premultiplied = Vec::with_capacity(asset.rgba.len());
    for pixel in asset.rgba.as_chunks::<4>().0 {
        let value = ColorU8::from_rgba(pixel[0], pixel[1], pixel[2], pixel[3]).premultiply();
        premultiplied.extend_from_slice(&[value.red(), value.green(), value.blue(), value.alpha()]);
    }
    Pixmap::from_vec(premultiplied, IntSize::from_wh(asset.width, asset.height)?)
}

fn round_half_away(value: f64) -> u32 {
    (value + 0.5).floor() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_and_invalid_rasters_fail_before_allocation() {
        let side_pt = 500.0 * 72.0 / 25.4;
        assert!(matches!(
            checked_raster_dimensions(side_pt, side_pt, 1200),
            Err(RasterError::TooLarge { .. })
        ));
        assert_eq!(
            checked_raster_dimensions(side_pt, side_pt, 0),
            Err(RasterError::InvalidDimensions)
        );
        assert_eq!(
            checked_raster_dimensions(f64::MAX, 100.0, 300),
            Err(RasterError::InvalidDimensions)
        );
    }
}
