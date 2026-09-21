use std::borrow::Cow;

use parley::{
    FontContext, FontFamily, FontStyle, FontWeight, LayoutContext, PositionedLayoutItem,
    StyleProperty,
    fontique::{Blob, FontInfoOverride},
};
use text_shaping_spike::{
    ProbeResult, ShapedGlyph, ShapedRun, Style, contains_missing_glyph, font_metadata,
    unsupported_v1_script, validation_labels,
};

const BASE_SIZE: f32 = 9.0;
const UPRIGHT: &[u8] = include_bytes!("../../assets/fonts/TeXGyreHeros-Regular.otf");
const ITALIC: &[u8] = include_bytes!("../../assets/fonts/TeXGyreHeros-Italic.otf");
const BOLD: &[u8] = include_bytes!("../../assets/fonts/TeXGyreHeros-Bold.otf");
const BOLD_ITALIC: &[u8] = include_bytes!("../../assets/fonts/TeXGyreHeros-BoldItalic.otf");
const PRIMARY_FAMILY: &str = "InstPlot Studio TeX Gyre Heros";

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
    font_context
        .collection
        .register_fonts(Blob::from(BOLD_ITALIC.to_vec()), family_override);
    let mut layout_context: LayoutContext<[u8; 4]> = LayoutContext::new();
    let mut labels = Vec::new();
    let mut warnings = Vec::new();

    for (label_name, label) in validation_labels() {
        let spans = label.spans();
        if let Some(character) = spans
            .iter()
            .find_map(|span| unsupported_v1_script(&span.text))
        {
            warnings.push(format!(
                "{label_name}: unsupported-script U+{:04X} {character}",
                character as u32
            ));
            labels.push((label_name.into(), Vec::new()));
            continue;
        }
        let mut runs = Vec::new();
        let mut cursor_x = 0.0_f32;
        for span in spans {
            let font_size = BASE_SIZE * span.scale;
            let mut builder =
                layout_context.ranged_builder(&mut font_context, &span.text, 1.0, false);
            builder.push_default(StyleProperty::FontFamily(FontFamily::Source(
                Cow::Borrowed("'InstPlot Studio TeX Gyre Heros'"),
            )));
            builder.push_default(StyleProperty::FontSize(font_size));
            match span.style {
                Style::Upright => {}
                Style::Italic => {
                    builder.push_default(StyleProperty::FontStyle(FontStyle::Italic));
                }
                Style::Bold => {
                    builder.push_default(StyleProperty::FontWeight(FontWeight::BOLD));
                }
                Style::BoldItalic => {
                    builder.push_default(StyleProperty::FontStyle(FontStyle::Italic));
                    builder.push_default(StyleProperty::FontWeight(FontWeight::BOLD));
                }
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

    const GREEK_CORE: &str = "αβγδεζηθικλμνξοπρστυφχψωΓΔΘΛΞΠΣΦΨΩϵϑϕϖς";
    const SCIENTIFIC_SYMBOL_CORE: &str = "+−±∓×·÷=<>≤≥≠≈∞∂√∑°%Å←→↑↓Ωμ";

    #[test]
    fn bundled_primary_is_repeatable_and_not_shadowed_by_system_font() {
        let first = shape_probe();
        let second = shape_probe();
        assert_eq!(first, second);

        let plain = &first.labels[0].1[0];
        assert_eq!(plain.font.postscript_name, "TeXGyreHeros-Regular");
        assert!(plain.glyphs[0].id > 0);
        assert!(plain.glyphs[0].advance > 0.0);
    }

    #[test]
    fn unsupported_script_and_missing_glyph_are_reported_without_fallback() {
        let result = shape_probe();
        assert!(result.labels[3].1.is_empty());
        assert!(result.warnings.iter().any(|warning| {
            warning.contains("unsupported-cjk") && warning.contains("unsupported-script")
        }));
        assert!(
            result
                .warnings
                .iter()
                .any(|warning| warning.contains("missing-glyph"))
        );
    }

    #[test]
    fn all_four_real_faces_are_selected() {
        let result = shape_probe();
        assert_eq!(
            result.labels[0].1[0].font.postscript_name,
            "TeXGyreHeros-Regular"
        );
        assert!(
            result.labels[1]
                .1
                .iter()
                .any(|run| run.font.postscript_name == "TeXGyreHeros-Italic")
        );
        assert_eq!(
            result.labels[7].1[0].font.postscript_name,
            "TeXGyreHeros-Bold"
        );
        assert_eq!(
            result.labels[8].1[0].font.postscript_name,
            "TeXGyreHeros-BoldItalic"
        );
    }

    #[test]
    fn all_four_faces_cover_v1_cores_and_allow_embedding() {
        for (name, data) in [
            ("Regular", UPRIGHT),
            ("Italic", ITALIC),
            ("Bold", BOLD),
            ("Bold Italic", BOLD_ITALIC),
        ] {
            let face = ttf_parser::Face::parse(data, 0).expect("valid bundled OTF");
            for character in GREEK_CORE.chars().chain(SCIENTIFIC_SYMBOL_CORE.chars()) {
                assert!(
                    face.glyph_index(character).is_some(),
                    "{name} lacks U+{:04X} {character}",
                    character as u32
                );
            }
            let metadata = font_metadata(data, 0);
            assert!(metadata.embedding_allowed, "{name} disallows embedding");
            assert!(metadata.subsetting_allowed, "{name} disallows subsetting");
        }
    }
}
