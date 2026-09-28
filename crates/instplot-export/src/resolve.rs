use std::borrow::Cow;
use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;

use instplot_render::TextAnchor;
use instplot_render::{Color, DisplayItem, DisplayList, NodeId, PathVerb};
use instplot_text::{FontMetadata, SpanFlow, Style, font_metadata};
use parley::fontique::{Blob, FontInfoOverride};
use parley::{FontContext, FontFamily, FontStyle, FontWeight, LayoutContext, StyleProperty};
use tiny_skia::{
    LineCap as SkLineCap, LineJoin as SkLineJoin, PathBuilder as SkPathBuilder, Stroke as SkStroke,
    StrokeDash,
};
use ttf_parser::{Face, GlyphId, OutlineBuilder};

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
    pub geometry: ResolvedCanvasGeometry,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ResolvedBounds {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl ResolvedBounds {
    pub const fn width(self) -> f32 {
        self.max_x - self.min_x
    }

    pub const fn height(self) -> f32 {
        self.max_y - self.min_y
    }

    fn intersect(self, other: Self) -> Option<Self> {
        let result = Self {
            min_x: self.min_x.max(other.min_x),
            min_y: self.min_y.max(other.min_y),
            max_x: self.max_x.min(other.max_x),
            max_y: self.max_y.min(other.max_y),
        };
        (result.max_x >= result.min_x && result.max_y >= result.min_y).then_some(result)
    }

    fn union(self, other: Self) -> Self {
        Self {
            min_x: self.min_x.min(other.min_x),
            min_y: self.min_y.min(other.min_y),
            max_x: self.max_x.max(other.max_x),
            max_y: self.max_y.max(other.max_y),
        }
    }

    fn inflate(self, amount: f32) -> Self {
        Self {
            min_x: self.min_x - amount,
            min_y: self.min_y - amount,
            max_x: self.max_x + amount,
            max_y: self.max_y + amount,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedCanvasGeometry {
    pub plot_bounds: ResolvedBounds,
    pub ink_bounds: ResolvedBounds,
    pub export_bounds: ResolvedBounds,
    /// Scene-to-page translation applied exactly once by the resolver.
    pub export_translation: (f32, f32),
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
    /// Actual rotated glyph-outline ink in scene coordinates.
    pub ink_bounds: Option<ResolvedBounds>,
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
    let full = ResolvedBounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: list.width.get() as f32,
        max_y: list.height.get() as f32,
    };
    ResolvedDisplayList {
        width: list.width.get() as f32,
        height: list.height.get() as f32,
        items,
        resources,
        geometry: ResolvedCanvasGeometry {
            plot_bounds: full,
            ink_bounds: full,
            export_bounds: full,
            export_translation: (0.0, 0.0),
        },
    }
}

/// Resolves a fixed-plot figure and derives one clip-aware export page from
/// visible ink. The returned items are translated into page coordinates once;
/// layout and persisted document coordinates remain in scene space.
pub fn resolve_tight(
    list: &DisplayList,
    plot_bounds: ResolvedBounds,
    safety_pt: f32,
) -> ResolvedDisplayList {
    resolve_tight_with_resources(list, BTreeMap::new(), plot_bounds, safety_pt)
}

/// Resource-aware variant of [`resolve_tight`]. Raster assets are retained for
/// the export backends, and only assets with valid, non-transparent pixels
/// contribute to the tight page bounds.
pub fn resolve_tight_with_resources(
    list: &DisplayList,
    resources: BTreeMap<String, RasterAsset>,
    plot_bounds: ResolvedBounds,
    safety_pt: f32,
) -> ResolvedDisplayList {
    let mut resolved = resolve_with_resources(list, resources);
    let ink_bounds =
        visible_ink_bounds(&resolved.items, &resolved.resources).unwrap_or(plot_bounds);
    let content = plot_bounds.union(ink_bounds).inflate(safety_pt.max(0.0));
    let translation = (-content.min_x, -content.min_y);
    resolved.width = content.width().max(1.0);
    resolved.height = content.height().max(1.0);
    resolved.geometry = ResolvedCanvasGeometry {
        plot_bounds,
        ink_bounds,
        export_bounds: content,
        export_translation: translation,
    };
    resolved
}

mod bounds;
mod text;

use bounds::visible_ink_bounds;
#[cfg(test)]
use text::FlowCursor;
use text::shape_text;

#[cfg(test)]
mod tests {
    use super::*;
    use instplot_render::{LineCap, LineJoin, Path, Pt, Stroke};

    #[test]
    fn combined_scripts_share_a_start_and_advance_by_the_wider_branch() {
        let mut cursor = FlowCursor::default();
        assert_eq!(cursor.place(SpanFlow::Inline, 10.0), 0.0);
        assert_eq!(cursor.place(SpanFlow::Subscript(0), 4.0), 10.0);
        assert_eq!(cursor.place(SpanFlow::Superscript(0), 6.0), 10.0);
        assert_eq!(cursor.place(SpanFlow::Inline, 3.0), 16.0);
        assert_eq!(cursor.finish(), 19.0);
    }

    #[test]
    fn tight_geometry_ignores_ink_outside_the_active_clip() {
        let pt = |value| Pt::new(value).unwrap();
        let display = DisplayList {
            width: pt(100.0),
            height: pt(100.0),
            items: vec![
                DisplayItem::ClipPush {
                    source: NodeId(1),
                    x: pt(10.0),
                    y: pt(10.0),
                    width: pt(80.0),
                    height: pt(80.0),
                },
                DisplayItem::Path {
                    source: NodeId(2),
                    path: Path {
                        verbs: vec![
                            PathVerb::MoveTo(pt(20.0), pt(20.0)),
                            PathVerb::LineTo(pt(10_000.0), pt(20.0)),
                        ],
                    },
                    fill: None,
                    stroke: Some(Stroke {
                        color: Color(0, 0, 0, 255),
                        width: pt(1.0),
                        cap: LineCap::Butt,
                        join: LineJoin::Miter,
                        dash: Vec::new(),
                    }),
                },
                DisplayItem::ClipPop { source: NodeId(1) },
            ],
        };
        let resolved = resolve_tight(
            &display,
            ResolvedBounds {
                min_x: 10.0,
                min_y: 10.0,
                max_x: 90.0,
                max_y: 90.0,
            },
            3.0,
        );
        assert!(resolved.width < 100.0);
        assert_eq!(resolved.geometry.ink_bounds.max_x, 90.0);
    }
}
