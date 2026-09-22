use std::borrow::Cow;
use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;

use parley::fontique::{Blob, FontInfoOverride};
use parley::{FontContext, FontFamily, FontStyle, FontWeight, LayoutContext, StyleProperty};
use studio_render_spike::TextAnchor;
use studio_render_spike::{Color, DisplayItem, DisplayList, NodeId};
use text_shaping_spike::{FontMetadata, Style, font_metadata};

const PRIMARY_FAMILY: &str = "InstPlot Studio TeX Gyre Heros";
const REGULAR: &[u8] =
    include_bytes!("../../text-shaping-spike/assets/fonts/TeXGyreHeros-Regular.otf");
const ITALIC: &[u8] =
    include_bytes!("../../text-shaping-spike/assets/fonts/TeXGyreHeros-Italic.otf");
const BOLD: &[u8] = include_bytes!("../../text-shaping-spike/assets/fonts/TeXGyreHeros-Bold.otf");
const BOLD_ITALIC: &[u8] =
    include_bytes!("../../text-shaping-spike/assets/fonts/TeXGyreHeros-BoldItalic.otf");

#[derive(Clone, Copy, Debug)]
pub struct BundledFontFace {
    pub postscript_name: &'static str,
    pub data: &'static [u8],
}

pub fn bundled_font_faces() -> [BundledFontFace; 4] {
    [
        BundledFontFace {
            postscript_name: "TeXGyreHeros-Regular",
            data: REGULAR,
        },
        BundledFontFace {
            postscript_name: "TeXGyreHeros-Italic",
            data: ITALIC,
        },
        BundledFontFace {
            postscript_name: "TeXGyreHeros-Bold",
            data: BOLD,
        },
        BundledFontFace {
            postscript_name: "TeXGyreHeros-BoldItalic",
            data: BOLD_ITALIC,
        },
    ]
}

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
    pub baseline_shift: f32,
    pub font_size: f32,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontOrigin {
    BundledPrimary,
    SystemFallback,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontDiagnostic {
    pub source: NodeId,
    pub text: String,
    pub origin: FontOrigin,
    pub postscript_name: String,
    pub version: String,
    pub embedding: String,
    pub embedding_allowed: bool,
    pub subsetting_allowed: bool,
    pub missing_glyph: bool,
}

impl ResolvedDisplayList {
    pub fn font_diagnostics(&self) -> Vec<FontDiagnostic> {
        let mut diagnostics = Vec::new();
        for item in &self.items {
            let ResolvedItem::Text(text) = item else {
                continue;
            };
            for run in &text.runs {
                let start = run
                    .glyphs
                    .iter()
                    .map(|glyph| glyph.text_range.start)
                    .min()
                    .unwrap_or(0);
                let end = run
                    .glyphs
                    .iter()
                    .map(|glyph| glyph.text_range.end)
                    .max()
                    .unwrap_or(start);
                diagnostics.push(FontDiagnostic {
                    source: text.source,
                    text: text.text.get(start..end).unwrap_or("").to_owned(),
                    origin: if run.font.postscript_name.starts_with("TeXGyreHeros-") {
                        FontOrigin::BundledPrimary
                    } else {
                        FontOrigin::SystemFallback
                    },
                    postscript_name: run.font.postscript_name.clone(),
                    version: run.font.version.clone(),
                    embedding: run.font.embedding.clone(),
                    embedding_allowed: run.font.embedding_allowed,
                    subsetting_allowed: run.font.subsetting_allowed,
                    missing_glyph: run.glyphs.iter().any(|glyph| glyph.id == 0),
                });
            }
        }
        diagnostics
    }
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
    for bytes in [REGULAR, ITALIC, BOLD, BOLD_ITALIC] {
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
    let text = source.label.normalized_text();
    let mut runs = Vec::new();
    let mut cursor_x = 0.0_f32;
    let mut byte_offset = 0_usize;
    for span in source.label.spans() {
        let font_size = size * span.scale;
        let is_unit_separator = span.is_unit_separator;
        if is_unit_separator {
            let face = ttf_parser::Face::parse(REGULAR, 0).expect("valid bundled regular face");
            let glyph_id = face.glyph_index(' ').expect("bundled space glyph").0.into();
            let advance = size * 0.2;
            let data = Arc::new(REGULAR.to_vec());
            runs.push(ResolvedRun {
                font_data: data.clone(),
                font_index: 0,
                font: font_metadata(data.as_slice(), 0),
                start_x: cursor_x,
                baseline_shift: span.baseline_shift_em * size,
                font_size,
                glyphs: vec![ResolvedGlyph {
                    id: glyph_id,
                    text_range: byte_offset..byte_offset + span.text.len(),
                    advance,
                    x_offset: 0.0,
                    y_offset: 0.0,
                }],
            });
            cursor_x += advance;
            byte_offset += span.text.len();
            continue;
        }
        let shaping_text = span.text.as_str();
        let mut builder = layout_context.ranged_builder(font_context, shaping_text, 1.0, false);
        builder.push_default(StyleProperty::FontFamily(FontFamily::Source(
            Cow::Borrowed("'InstPlot Studio TeX Gyre Heros'"),
        )));
        builder.push_default(StyleProperty::FontSize(font_size));
        match span.style {
            Style::Upright => {}
            Style::Italic => builder.push_default(StyleProperty::FontStyle(FontStyle::Italic)),
            Style::Bold => builder.push_default(StyleProperty::FontWeight(FontWeight::BOLD)),
            Style::BoldItalic => {
                builder.push_default(StyleProperty::FontStyle(FontStyle::Italic));
                builder.push_default(StyleProperty::FontWeight(FontWeight::BOLD));
            }
        }
        let mut layout = builder.build(shaping_text);
        layout.break_all_lines(None);

        for line in layout.lines() {
            for run in line.runs() {
                let font = run.font().clone();
                let data = Arc::new(font.data.as_ref().to_vec());
                let mut glyphs: Vec<ResolvedGlyph> = Vec::new();
                for cluster in run.visual_clusters() {
                    if cluster.is_ligature_continuation() {
                        if let Some(glyph) = glyphs.last_mut() {
                            glyph.text_range.end = byte_offset + cluster.text_range().end;
                        }
                        continue;
                    }
                    for glyph in cluster.glyphs() {
                        glyphs.push(ResolvedGlyph {
                            id: glyph.id,
                            text_range: byte_offset + cluster.text_range().start
                                ..byte_offset + cluster.text_range().end,
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
                    baseline_shift: span.baseline_shift_em * size,
                    font_size,
                    glyphs,
                });
                cursor_x += advance;
            }
        }
        byte_offset += span.text.len();
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
        text,
        x: source.x.get() as f32,
        y: source.y.get() as f32,
        size,
        color: source.color,
        rotation_degrees: source.rotation_degrees as f32,
        runs,
    }
}
