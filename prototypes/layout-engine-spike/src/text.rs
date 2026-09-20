use std::borrow::Cow;

use parley::fontique::{Blob, FontInfoOverride};
use parley::{FontContext, FontFamily, LayoutContext, StyleProperty};

const PRIMARY_FAMILY: &str = "SciPlot Source Sans 3";
const REGULAR: &[u8] =
    include_bytes!("../../text-shaping-spike/assets/fonts/SourceSans3-Regular.otf");

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextSize {
    pub width: f64,
    pub height: f64,
}

pub trait TextMeasurer {
    fn measure(&mut self, text: &str, size_pt: f64) -> TextSize;
}

pub struct ParleyMeasurer {
    fonts: FontContext,
    layouts: LayoutContext<[u8; 4]>,
}

impl Default for ParleyMeasurer {
    fn default() -> Self {
        let mut fonts = FontContext::new();
        fonts.collection.register_fonts(
            Blob::from(REGULAR.to_vec()),
            Some(FontInfoOverride {
                family_name: Some(PRIMARY_FAMILY),
                ..Default::default()
            }),
        );
        Self {
            fonts,
            layouts: LayoutContext::new(),
        }
    }
}

impl TextMeasurer for ParleyMeasurer {
    fn measure(&mut self, text: &str, size_pt: f64) -> TextSize {
        let mut builder = self
            .layouts
            .ranged_builder(&mut self.fonts, text, 1.0, false);
        builder.push_default(StyleProperty::FontFamily(FontFamily::Source(
            Cow::Borrowed("'SciPlot Source Sans 3', sans-serif"),
        )));
        builder.push_default(StyleProperty::FontSize(size_pt as f32));
        let mut layout = builder.build(text);
        layout.break_all_lines(None);
        TextSize {
            width: layout.width() as f64,
            height: layout.height() as f64,
        }
    }
}
