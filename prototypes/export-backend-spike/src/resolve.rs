use std::borrow::Cow;
use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;

use parley::fontique::{Blob, FontInfoOverride};
use parley::{FontContext, FontFamily, LayoutContext, StyleProperty};
use studio_render_spike::TextAnchor;
use studio_render_spike::{Color, DisplayItem, DisplayList, NodeId};
use text_shaping_spike::{FontMetadata, font_metadata};

const PRIMARY_FAMILY: &str = "SciPlot Source Sans 3";
const REGULAR: &[u8] =
    include_bytes!("../../text-shaping-spike/assets/fonts/SourceSans3-Regular.otf");
const ITALIC: &[u8] = include_bytes!("../../text-shaping-spike/assets/fonts/SourceSans3-It.otf");
const BOLD: &[u8] = include_bytes!("../../text-shaping-spike/assets/fonts/SourceSans3-Bold.otf");

#[derive(Clone, Debug)]
pub struct ResolvedDisplayList {
    pub width: f32,
    pub height: f32,
    pub items: Vec<ResolvedItem>,
    pub resources: BTreeMap<String, RasterAsset>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RasterAsset {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Debug)]
pub enum ResolvedItem {
    Graphics(DisplayItem),
    Text(ResolvedText),
}

#[derive(Clone, Debug)]
pub struct ResolvedText {
    pub source: NodeId,
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub size: f32,
    pub color: Color,
    pub rotation_degrees: f32,
    pub runs: Vec<ResolvedRun>,
}

#[derive(Clone, Debug)]
pub struct ResolvedRun {
    pub font_data: Arc<Vec<u8>>,
    pub font_index: u32,
    pub font: FontMetadata,
    pub start_x: f32,
    pub glyphs: Vec<ResolvedGlyph>,
}

#[derive(Clone, Debug)]
pub struct ResolvedGlyph {
    pub id: u32,
    pub text_range: Range<usize>,
    pub advance: f32,
    pub x_offset: f32,
    pub y_offset: f32,
}

pub fn resolve(list: &DisplayList) -> ResolvedDisplayList {
    resolve_with_resources(list, BTreeMap::new())
}

pub fn resolve_with_resources(
    list: &DisplayList,
    resources: BTreeMap<String, RasterAsset>,
) -> ResolvedDisplayList {
    let mut font_context = FontContext::new();
    let family_override = Some(FontInfoOverride {
        family_name: Some(PRIMARY_FAMILY),
        ..Default::default()
    });
    for bytes in [REGULAR, ITALIC, BOLD] {
        font_context
            .collection
            .register_fonts(Blob::from(bytes.to_vec()), family_override);
    }
    let mut layout_context: LayoutContext<[u8; 4]> = LayoutContext::new();
    let items = list
        .items
        .iter()
        .map(|item| match item {
            DisplayItem::GlyphRun(text) => {
                ResolvedItem::Text(shape_text(text, &mut font_context, &mut layout_context))
            }
            other => ResolvedItem::Graphics(other.clone()),
        })
        .collect();
    ResolvedDisplayList {
        width: list.width.get() as f32,
        height: list.height.get() as f32,
        items,
        resources,
    }
}

fn shape_text(
    source: &studio_render_spike::GlyphRun,
    font_context: &mut FontContext,
    layout_context: &mut LayoutContext<[u8; 4]>,
) -> ResolvedText {
    let size = source.size.get() as f32;
    let mut builder = layout_context.ranged_builder(font_context, &source.text, 1.0, false);
    builder.push_default(StyleProperty::FontFamily(FontFamily::Source(
        Cow::Borrowed("'SciPlot Source Sans 3', sans-serif"),
    )));
    builder.push_default(StyleProperty::FontSize(size));
    let mut layout = builder.build(&source.text);
    layout.break_all_lines(None);

    let mut runs = Vec::new();
    let mut cursor_x = 0.0_f32;
    for line in layout.lines() {
        for run in line.runs() {
            let font = run.font().clone();
            let data = Arc::new(font.data.as_ref().to_vec());
            let mut glyphs: Vec<ResolvedGlyph> = Vec::new();
            for cluster in run.visual_clusters() {
                if cluster.is_ligature_continuation() {
                    if let Some(glyph) = glyphs.last_mut() {
                        glyph.text_range.end = cluster.text_range().end;
                    }
                    continue;
                }
                for glyph in cluster.glyphs() {
                    glyphs.push(ResolvedGlyph {
                        id: glyph.id,
                        text_range: cluster.text_range(),
                        advance: glyph.advance,
                        x_offset: glyph.x,
                        y_offset: glyph.y,
                    });
                }
            }
            let advance = glyphs.iter().map(|glyph| glyph.advance).sum::<f32>();
            runs.push(ResolvedRun {
                font_data: data.clone(),
                font_index: font.index,
                font: font_metadata(data.as_slice(), font.index),
                start_x: cursor_x,
                glyphs,
            });
            cursor_x += advance;
        }
    }
    let anchor_offset = match source.anchor {
        TextAnchor::Start => 0.0,
        TextAnchor::Middle => -cursor_x / 2.0,
        TextAnchor::End => -cursor_x,
    };
    for run in &mut runs {
        run.start_x += anchor_offset;
    }

    ResolvedText {
        source: source.source,
        text: source.text.clone(),
        x: source.x.get() as f32,
        y: source.y.get() as f32,
        size,
        color: source.color,
        rotation_degrees: source.rotation_degrees as f32,
        runs,
    }
}
