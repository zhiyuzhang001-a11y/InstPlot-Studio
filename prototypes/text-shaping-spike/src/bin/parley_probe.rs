use std::borrow::Cow;

use parley::{
    FontContext, FontFamily, FontStyle, LayoutContext, PositionedLayoutItem, StyleProperty,
    fontique::{Blob, FontInfoOverride},
};
use text_shaping_spike::{
    ProbeResult, ShapedGlyph, ShapedRun, Style, contains_missing_glyph, font_metadata,
    validation_labels,
};

const BASE_SIZE: f32 = 9.0;
const UPRIGHT: &[u8] = include_bytes!("../../assets/fonts/SourceSans3-Regular.otf");
const ITALIC: &[u8] = include_bytes!("../../assets/fonts/SourceSans3-It.otf");
const BOLD: &[u8] = include_bytes!("../../assets/fonts/SourceSans3-Bold.otf");
const PRIMARY_FAMILY: &str = "SciPlot Source Sans 3";

fn main() {
    let result = shape_probe();
    print!("{}", result.snapshot());
}

fn shape_probe() -> ProbeResult {
    let mut font_context = FontContext::new();
    let family_override = Some(FontInfoOverride {
        family_name: Some(PRIMARY_FAMILY),
        ..Default::default()
    });
    font_context
        .collection
        .register_fonts(Blob::from(UPRIGHT.to_vec()), family_override);
    font_context
        .collection
        .register_fonts(Blob::from(ITALIC.to_vec()), family_override);
    font_context
        .collection
        .register_fonts(Blob::from(BOLD.to_vec()), family_override);
    let mut layout_context: LayoutContext<[u8; 4]> = LayoutContext::new();
    let mut labels = Vec::new();
    let mut warnings = Vec::new();

    for (label_name, label) in validation_labels() {
        let mut runs = Vec::new();
        let mut cursor_x = 0.0_f32;
        for span in label.spans() {
            let font_size = BASE_SIZE * span.scale;
            let mut builder =
                layout_context.ranged_builder(&mut font_context, &span.text, 1.0, false);
            builder.push_default(StyleProperty::FontFamily(FontFamily::Source(
                Cow::Borrowed("'SciPlot Source Sans 3', sans-serif"),
            )));
            builder.push_default(StyleProperty::FontSize(font_size));
            if span.style == Style::Italic {
                builder.push_default(StyleProperty::FontStyle(FontStyle::Italic));
            }
            let mut layout = builder.build(&span.text);
            layout.break_all_lines(None);
            let mut span_advance = 0.0_f32;
            for line in layout.lines() {
                for item in line.items() {
                    let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                        continue;
                    };
                    let font = glyph_run.run().font();
                    let glyphs = glyph_run
                        .positioned_glyphs()
                        .map(|glyph| ShapedGlyph {
                            id: glyph.id,
                            x: cursor_x + glyph.x,
                            y: glyph.y + span.baseline_shift_em * BASE_SIZE,
                            advance: glyph.advance,
                        })
                        .collect();
                    let shaped = ShapedRun {
                        source: span.text.clone(),
                        font: font_metadata(font.data.as_ref(), font.index),
                        font_size: glyph_run.run().font_size(),
                        style: span.style,
                        glyphs,
                    };
                    if contains_missing_glyph(&shaped) {
                        warnings.push(format!("{label_name}: missing glyph in {:?}", span.text));
                    }
                    span_advance = span_advance.max(glyph_run.offset() + glyph_run.advance());
                    runs.push(shaped);
                }
            }
            cursor_x += span_advance;
        }
        labels.push((label_name.into(), runs));
    }

    ProbeResult {
        engine: "parley-0.11.1".into(),
        labels,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_primary_is_repeatable_and_not_shadowed_by_system_font() {
        let first = shape_probe();
        let second = shape_probe();
        assert_eq!(first, second);

        let plain = &first.labels[0].1[0];
        assert_eq!(plain.font.postscript_name, "SourceSans3-Regular");
        assert_eq!(plain.glyphs[0].id, 6); // E in the pinned static OTF.
        assert!((plain.glyphs[0].advance - 4.464).abs() < 0.0001);
    }

    #[test]
    fn cjk_fallback_identity_and_missing_glyph_are_reported() {
        let result = shape_probe();
        let cjk_runs = &result.labels[3].1;
        assert!(cjk_runs.iter().any(|run| {
            run.font.postscript_name != "SourceSans3-Regular"
                && run.font.postscript_name != "<unknown>"
                && run.font.version != "<unknown>"
        }));
        assert!(
            result
                .warnings
                .iter()
                .any(|warning| warning.contains("missing-glyph"))
        );
    }
}
