use std::borrow::Cow;
use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;

use instplot_render::TextAnchor;
use instplot_render::{Color, DisplayItem, DisplayList, NodeId};
use instplot_text::{FontMetadata, SpanFlow, Style, font_metadata};
use parley::fontique::{Blob, FontInfoOverride};
use parley::{FontContext, FontFamily, FontStyle, FontWeight, LayoutContext, StyleProperty};

const PRIMARY_FAMILY: &str = "InstPlot Studio TeX Gyre Heros";
const RELATION_FAMILY: &str = "InstPlot Studio STIX Two Math";
const RELATION_FONT: &[u8] =
    include_bytes!("../../instplot-text/assets/fonts/STIXTwoMath-Regular.otf");
const REGULAR: &[u8] = include_bytes!("../../instplot-text/assets/fonts/TeXGyreHeros-Regular.otf");
const ITALIC: &[u8] = include_bytes!("../../instplot-text/assets/fonts/TeXGyreHeros-Italic.otf");
const BOLD: &[u8] = include_bytes!("../../instplot-text/assets/fonts/TeXGyreHeros-Bold.otf");
const BOLD_ITALIC: &[u8] =
    include_bytes!("../../instplot-text/assets/fonts/TeXGyreHeros-BoldItalic.otf");

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

pub fn bundled_relation_face() -> BundledFontFace {
    BundledFontFace {
        postscript_name: "STIXTwoMath-Regular",
        data: RELATION_FONT,
    }
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
    BundledSymbol,
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
                    } else if run.font.postscript_name == "STIXTwoMath-Regular" {
                        FontOrigin::BundledSymbol
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
    font_context.collection.register_fonts(
        Blob::from(RELATION_FONT.to_vec()),
        Some(FontInfoOverride {
            family_name: Some(RELATION_FAMILY),
            ..Default::default()
        }),
    );
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

#[derive(Default)]
struct FlowCursor {
    cursor: f32,
    script_group: Option<u32>,
    subscript_advance: f32,
    superscript_advance: f32,
}

impl FlowCursor {
    fn place(&mut self, flow: SpanFlow, advance: f32) -> f32 {
        match flow {
            SpanFlow::Inline => {
                self.finish_script_group();
                let start = self.cursor;
                self.cursor += advance;
                start
            }
            SpanFlow::Subscript(group) => {
                self.start_script_group(group);
                let start = self.cursor + self.subscript_advance;
                self.subscript_advance += advance;
                start
            }
            SpanFlow::Superscript(group) => {
                self.start_script_group(group);
                let start = self.cursor + self.superscript_advance;
                self.superscript_advance += advance;
                start
            }
        }
    }

    fn start_script_group(&mut self, group: u32) {
        if self.script_group != Some(group) {
            self.finish_script_group();
            self.script_group = Some(group);
        }
    }

    fn finish_script_group(&mut self) {
        if self.script_group.take().is_some() {
            self.cursor += self.subscript_advance.max(self.superscript_advance);
            self.subscript_advance = 0.0;
            self.superscript_advance = 0.0;
        }
    }

    fn finish(mut self) -> f32 {
        self.finish_script_group();
        self.cursor
    }
}

fn shape_text(
    source: &instplot_render::GlyphRun,
    font_context: &mut FontContext,
    layout_context: &mut LayoutContext<[u8; 4]>,
) -> ResolvedText {
    let size = source.size.get() as f32;
    let text = source.label.normalized_text();
    let mut runs = Vec::new();
    let mut flow_cursor = FlowCursor::default();
    let mut byte_offset = 0_usize;
    for span in source.label.spans() {
        let font_size = size * span.scale;
        let is_unit_separator = span.is_unit_separator;
        if is_unit_separator {
            let face = ttf_parser::Face::parse(REGULAR, 0).expect("valid bundled regular face");
            let glyph_id = face.glyph_index(' ').expect("bundled space glyph").0.into();
            let advance = size * 0.2;
            let start_x = flow_cursor.place(span.flow, advance);
            let data = Arc::new(REGULAR.to_vec());
            runs.push(ResolvedRun {
                font_data: data.clone(),
                font_index: 0,
                font: font_metadata(data.as_slice(), 0),
                start_x,
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
            byte_offset += span.text.len();
            continue;
        }
        let shaping_text = span.text.as_str();
        let mut builder = layout_context.ranged_builder(font_context, shaping_text, 1.0, false);
        let family = if matches!(span.text.as_str(), "≤" | "≥") {
            "'InstPlot Studio STIX Two Math'"
        } else {
            "'InstPlot Studio TeX Gyre Heros'"
        };
        builder.push_default(StyleProperty::FontFamily(FontFamily::Source(
            Cow::Borrowed(family),
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
                let start_x = flow_cursor.place(span.flow, advance);
                runs.push(ResolvedRun {
                    font_data: data.clone(),
                    font_index: font.index,
                    font: font_metadata(data.as_slice(), font.index),
                    start_x,
                    baseline_shift: span.baseline_shift_em * size,
                    font_size,
                    glyphs,
                });
            }
        }
        byte_offset += span.text.len();
    }
    let cursor_x = flow_cursor.finish();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combined_scripts_share_a_start_and_advance_by_the_wider_branch() {
        let mut cursor = FlowCursor::default();
        assert_eq!(cursor.place(SpanFlow::Inline, 10.0), 0.0);
        assert_eq!(cursor.place(SpanFlow::Subscript(0), 4.0), 10.0);
        assert_eq!(cursor.place(SpanFlow::Superscript(0), 6.0), 10.0);
        assert_eq!(cursor.place(SpanFlow::Inline, 3.0), 16.0);
        assert_eq!(cursor.finish(), 19.0);
    }
}
