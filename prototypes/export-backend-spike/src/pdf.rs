use std::collections::HashMap;
use std::sync::Arc;

use krilla::Document;
use krilla::color::rgb;
use krilla::geom::{Path as KrillaPath, PathBuilder, Point, Size, Transform};
use krilla::image::Image as KrillaImage;
use krilla::metadata::Metadata;
use krilla::num::NormalizedF32;
use krilla::page::PageSettings;
use krilla::paint::{
    Fill as KrillaFill, FillRule as KrillaFillRule, LineCap as KrillaLineCap,
    LineJoin as KrillaLineJoin, Stroke as KrillaStroke, StrokeDash,
};
use krilla::text::{Font, GlyphId, KrillaGlyph};
use studio_render_spike::{
    Color, DisplayItem, FillRule, LineCap, LineJoin, Path, PathVerb, Stroke,
};

use crate::raster::encode_asset_png;
use crate::{RasterAsset, ResolvedDisplayList, ResolvedItem, ResolvedText};

#[derive(Debug)]
pub enum ExportError {
    InvalidPage,
    InvalidPath,
    InvalidFont(String),
    Pdf(String),
}

impl core::fmt::Display for ExportError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ExportError {}

pub fn to_pdf(list: &ResolvedDisplayList) -> Result<Vec<u8>, ExportError> {
    let mut document = Document::new();
    document.set_metadata(
        Metadata::new()
            .title("SciPlot A4 publication fixture".into())
            .creator("SciPlot export-backend-spike".into()),
    );
    let settings =
        PageSettings::from_wh(list.width, list.height).ok_or(ExportError::InvalidPage)?;
    let mut page = document.start_page_with(settings);
    let mut surface = page.surface();
    let mut fonts: HashMap<(String, u32), Font> = HashMap::new();

    for item in &list.items {
        match item {
            ResolvedItem::Graphics(item) => draw_graphics(item, &list.resources, &mut surface)?,
            ResolvedItem::Text(text) => draw_text(text, &mut surface, &mut fonts)?,
        }
    }

    surface.finish();
    page.finish();
    document
        .finish()
        .map_err(|error| ExportError::Pdf(error.to_string()))
}

fn draw_graphics(
    item: &DisplayItem,
    resources: &std::collections::BTreeMap<String, RasterAsset>,
    surface: &mut krilla::surface::Surface<'_>,
) -> Result<(), ExportError> {
    match item {
        DisplayItem::Path {
            path, fill, stroke, ..
        } => {
            surface.set_fill(fill.map(|fill| KrillaFill {
                paint: color(fill.color).into(),
                opacity: opacity(fill.color),
                rule: match fill.rule {
                    FillRule::NonZero => KrillaFillRule::NonZero,
                    FillRule::EvenOdd => KrillaFillRule::EvenOdd,
                },
            }));
            surface.set_stroke(stroke.as_ref().map(pdf_stroke));
            surface.draw_path(&pdf_path(path)?);
        }
        DisplayItem::ClipPush {
            x,
            y,
            width,
            height,
            ..
        } => {
            let mut builder = PathBuilder::new();
            builder.move_to(x.get() as f32, y.get() as f32);
            builder.line_to((x.get() + width.get()) as f32, y.get() as f32);
            builder.line_to(
                (x.get() + width.get()) as f32,
                (y.get() + height.get()) as f32,
            );
            builder.line_to(x.get() as f32, (y.get() + height.get()) as f32);
            builder.close();
            surface.push_clip_path(
                &builder.finish().ok_or(ExportError::InvalidPath)?,
                &KrillaFillRule::NonZero,
            );
        }
        DisplayItem::ClipPop { .. } => surface.pop(),
        DisplayItem::Image(image) => {
            let Some(asset) = resources.get(&image.resource_id) else {
                return Ok(());
            };
            let png =
                encode_asset_png(asset).map_err(|error| ExportError::Pdf(error.to_string()))?;
            let image_data =
                KrillaImage::from_png(Arc::new(png).into(), true).map_err(ExportError::Pdf)?;
            surface.push_transform(&Transform::from_translate(
                image.x.get() as f32,
                image.y.get() as f32,
            ));
            surface.draw_image(
                image_data,
                Size::from_wh(image.width.get() as f32, image.height.get() as f32)
                    .ok_or(ExportError::InvalidPath)?,
            );
            surface.pop();
        }
        DisplayItem::GlyphRun(_) => unreachable!("text is resolved before backend dispatch"),
    }
    Ok(())
}

fn draw_text(
    text: &ResolvedText,
    surface: &mut krilla::surface::Surface<'_>,
    fonts: &mut HashMap<(String, u32), Font>,
) -> Result<(), ExportError> {
    surface.set_stroke(None);
    surface.set_fill(Some(KrillaFill {
        paint: color(text.color).into(),
        opacity: opacity(text.color),
        rule: KrillaFillRule::NonZero,
    }));
    if text.rotation_degrees != 0.0 {
        surface.push_transform(&Transform::from_rotate_at(
            text.rotation_degrees,
            text.x,
            text.y,
        ));
    }
    for run in &text.runs {
        let key = (run.font.postscript_name.clone(), run.font_index);
        let font = if let Some(font) = fonts.get(&key) {
            font.clone()
        } else {
            let font = Font::new(run.font_data.clone().into(), run.font_index)
                .ok_or_else(|| ExportError::InvalidFont(run.font.postscript_name.clone()))?;
            fonts.insert(key, font.clone());
            font
        };
        let glyphs = run
            .glyphs
            .iter()
            .map(|glyph| {
                KrillaGlyph::new(
                    GlyphId::new(glyph.id),
                    glyph.advance / run.font_size,
                    glyph.x_offset / run.font_size,
                    glyph.y_offset / run.font_size,
                    0.0,
                    glyph.text_range.clone(),
                    None,
                )
            })
            .collect::<Vec<_>>();
        surface.draw_glyphs(
            Point::from_xy(text.x + run.start_x, text.y + run.baseline_shift),
            &glyphs,
            font,
            &text.text,
            run.font_size,
            false,
        );
    }
    if text.rotation_degrees != 0.0 {
        surface.pop();
    }
    Ok(())
}

fn pdf_path(path: &Path) -> Result<KrillaPath, ExportError> {
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
    builder.finish().ok_or(ExportError::InvalidPath)
}

fn pdf_stroke(stroke: &Stroke) -> KrillaStroke {
    KrillaStroke {
        paint: color(stroke.color).into(),
        width: stroke.width.get() as f32,
        miter_limit: 10.0,
        line_cap: match stroke.cap {
            LineCap::Butt => KrillaLineCap::Butt,
            LineCap::Round => KrillaLineCap::Round,
            LineCap::Square => KrillaLineCap::Square,
        },
        line_join: match stroke.join {
            LineJoin::Miter => KrillaLineJoin::Miter,
            LineJoin::Round => KrillaLineJoin::Round,
            LineJoin::Bevel => KrillaLineJoin::Bevel,
        },
        opacity: opacity(stroke.color),
        dash: (!stroke.dash.is_empty()).then(|| StrokeDash {
            array: stroke.dash.iter().map(|value| value.get() as f32).collect(),
            offset: 0.0,
        }),
    }
}

fn color(value: Color) -> rgb::Color {
    rgb::Color::new(value.0, value.1, value.2)
}

fn opacity(value: Color) -> NormalizedF32 {
    NormalizedF32::new(value.3 as f32 / 255.0).unwrap()
}
