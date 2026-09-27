use cosmic_text::{
    Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Style as CosmicStyle, Weight,
};
use instplot_text::{
    ProbeResult, ShapedGlyph, ShapedRun, Style, contains_missing_glyph, font_metadata,
    unsupported_v1_script, validation_labels,
};

const BASE_SIZE: f32 = 9.0;
const UPRIGHT: &[u8] = include_bytes!("../../assets/fonts/TeXGyreHeros-Regular.otf");
const ITALIC: &[u8] = include_bytes!("../../assets/fonts/TeXGyreHeros-Italic.otf");
const BOLD: &[u8] = include_bytes!("../../assets/fonts/TeXGyreHeros-Bold.otf");
const BOLD_ITALIC: &[u8] = include_bytes!("../../assets/fonts/TeXGyreHeros-BoldItalic.otf");

fn main() {
    let result = shape_probe();
    print!("{}", result.snapshot());
}

fn shape_probe() -> ProbeResult {
    // V1 deliberately exposes only the four bundled publication faces. Unsupported
    // scripts are rejected before shaping rather than resolved from system fonts.
    let mut db = cosmic_text::fontdb::Database::new();
    db.load_font_data(UPRIGHT.to_vec());
    db.load_font_data(ITALIC.to_vec());
    db.load_font_data(BOLD.to_vec());
    db.load_font_data(BOLD_ITALIC.to_vec());
    let locale = std::env::var("LANG").unwrap_or_else(|_| "en-US".into());
    let mut font_system = FontSystem::new_with_locale_and_db(locale, db);
    font_system.db_mut().set_sans_serif_family("TeX Gyre Heros");
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
            let (font_style, font_weight) = match span.style {
                Style::Upright => (CosmicStyle::Normal, Weight::NORMAL),
                Style::Italic => (CosmicStyle::Italic, Weight::NORMAL),
                Style::Bold => (CosmicStyle::Normal, Weight::BOLD),
                Style::BoldItalic => (CosmicStyle::Italic, Weight::BOLD),
            };
            let attrs = Attrs::new()
                .family(Family::Name("TeX Gyre Heros"))
                .style(font_style)
                .weight(font_weight);
            let collected = {
                let mut buffer = Buffer::new_empty(Metrics::new(font_size, font_size * 1.2));
                buffer.set_size(Some(1000.0), Some(font_size * 2.0));
                buffer.set_text(&span.text, &attrs, Shaping::Advanced, None);
                buffer.shape_until_scroll(&mut font_system, false);
                let mut collected = Vec::new();
                for line in buffer.layout_runs() {
                    for glyph in line.glyphs {
                        collected.push((
                            glyph.font_id,
                            glyph.glyph_id,
                            cursor_x + glyph.x + glyph.font_size * glyph.x_offset,
                            line.line_y - glyph.font_size * glyph.y_offset
                                + span.baseline_shift_em * BASE_SIZE,
                            glyph.w,
                            glyph.font_size,
                        ));
                    }
                }
                collected
            };
            let span_advance = collected
                .iter()
                .map(|(_, _, x, _, advance, _)| x + advance - cursor_x)
                .fold(0.0_f32, f32::max);

            let mut start = 0;
            while start < collected.len() {
                let font_id = collected[start].0;
                let mut end = start + 1;
                while end < collected.len() && collected[end].0 == font_id {
                    end += 1;
                }
                let font = font_system
                    .db()
                    .with_face_data(font_id, font_metadata)
                    .unwrap_or_else(|| font_metadata(&[], 0));
                let glyphs = collected[start..end]
                    .iter()
                    .map(|(_, glyph_id, x, y, advance, _)| ShapedGlyph {
                        id: u32::from(*glyph_id),
                        x: *x,
                        y: *y,
                        advance: *advance,
                    })
                    .collect();
                let shaped = ShapedRun {
                    source: span.text.clone(),
                    font,
                    font_size: collected[start].5,
                    style: span.style,
                    glyphs,
                };
                if contains_missing_glyph(&shaped) {
                    warnings.push(format!("{label_name}: missing glyph in {:?}", span.text));
                }
                runs.push(shaped);
                start = end;
            }
            cursor_x += span_advance;
        }
        labels.push((label_name.into(), runs));
    }

    ProbeResult {
        engine: "cosmic-text-0.19.0".into(),
        labels,
        warnings,
    }
}
