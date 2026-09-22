use std::borrow::Cow;

use parley::fontique::{Blob, FontInfoOverride};
use parley::{FontContext, FontFamily, FontStyle, FontWeight, LayoutContext, StyleProperty};
use text_shaping_spike::{Label, Span, Style};

const PRIMARY_FAMILY: &str = "InstPlot Studio TeX Gyre Heros";
const REGULAR: &[u8] =
    include_bytes!("../../../prototypes/text-shaping-spike/assets/fonts/TeXGyreHeros-Regular.otf");
const ITALIC: &[u8] =
    include_bytes!("../../../prototypes/text-shaping-spike/assets/fonts/TeXGyreHeros-Italic.otf");
const BOLD: &[u8] =
    include_bytes!("../../../prototypes/text-shaping-spike/assets/fonts/TeXGyreHeros-Bold.otf");
const BOLD_ITALIC: &[u8] = include_bytes!(
    "../../../prototypes/text-shaping-spike/assets/fonts/TeXGyreHeros-BoldItalic.otf"
);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextSize {
    pub width: f64,
    pub height: f64,
}

pub trait TextMeasurer {
    fn measure(&mut self, text: &str, size_pt: f64) -> TextSize;

    fn measure_label(&mut self, label: &Label, size_pt: f64) -> TextSize {
        self.measure(&label.normalized_text(), size_pt)
    }
}

pub struct ParleyMeasurer {
    fonts: FontContext,
    layouts: LayoutContext<[u8; 4]>,
}

impl Default for ParleyMeasurer {
    fn default() -> Self {
        let mut fonts = FontContext::new();
        let family_override = Some(FontInfoOverride {
            family_name: Some(PRIMARY_FAMILY),
            ..Default::default()
        });
        for data in [REGULAR, ITALIC, BOLD, BOLD_ITALIC] {
            fonts
                .collection
                .register_fonts(Blob::from(data.to_vec()), family_override);
        }
        Self {
            fonts,
            layouts: LayoutContext::new(),
        }
    }
}

impl TextMeasurer for ParleyMeasurer {
    fn measure(&mut self, text: &str, size_pt: f64) -> TextSize {
        self.measure_span(
            &Span {
                text: text.into(),
                style: Style::Upright,
                scale: 1.0,
                baseline_shift_em: 0.0,
                is_unit_separator: false,
            },
            size_pt,
        )
    }

    fn measure_label(&mut self, label: &Label, size_pt: f64) -> TextSize {
        let mut width = 0.0_f64;
        let mut top = 0.0_f64;
        let mut bottom = 0.0_f64;
        for span in label.spans() {
            if span.is_unit_separator {
                width += size_pt * 0.2;
                continue;
            }
            let measured = self.measure_span(&span, size_pt);
            let shift = f64::from(span.baseline_shift_em) * size_pt;
            width += measured.width;
            top = top.min(shift);
            bottom = bottom.max(shift + measured.height);
        }
        TextSize {
            width,
            height: bottom - top,
        }
    }
}

impl ParleyMeasurer {
    fn measure_span(&mut self, span: &Span, size_pt: f64) -> TextSize {
        let font_size = size_pt as f32 * span.scale;
        let mut builder = self
            .layouts
            .ranged_builder(&mut self.fonts, &span.text, 1.0, false);
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
        let mut layout = builder.build(&span.text);
        layout.break_all_lines(None);
        TextSize {
            width: layout.width() as f64,
            height: layout.height() as f64,
        }
    }
}
