#[derive(Clone, Debug, PartialEq)]
pub enum Label {
    Text(String),
    Variable(String),
    Upright(String),
    Greek(char),
    Subscript { body: Box<Label>, descriptive: bool },
    Superscript(Box<Label>),
    Unit(String),
    Operator(String),
    Group(Vec<Label>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Upright,
    Italic,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    pub text: String,
    pub style: Style,
    pub scale: f32,
    pub baseline_shift_em: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FontMetadata {
    pub full_name: String,
    pub postscript_name: String,
    pub version: String,
    pub index: u32,
    pub embedding: String,
    pub subsetting_allowed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShapedGlyph {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub advance: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShapedRun {
    pub source: String,
    pub font: FontMetadata,
    pub font_size: f32,
    pub style: Style,
    pub glyphs: Vec<ShapedGlyph>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProbeResult {
    pub engine: String,
    pub labels: Vec<(String, Vec<ShapedRun>)>,
    pub warnings: Vec<String>,
}

impl ProbeResult {
    pub fn snapshot(&self) -> String {
        use core::fmt::Write;

        let mut output = String::new();
        writeln!(output, "ENGINE {}", self.engine).unwrap();
        for (label, runs) in &self.labels {
            writeln!(output, "LABEL {label}").unwrap();
            for run in runs {
                writeln!(
                    output,
                    "RUN source={:?} font={:?} ps={:?} version={:?} index={} size={:.3} style={:?} embedding={} subset={}",
                    run.source,
                    run.font.full_name,
                    run.font.postscript_name,
                    run.font.version,
                    run.font.index,
                    run.font_size,
                    run.style,
                    run.font.embedding,
                    run.font.subsetting_allowed
                )
                .unwrap();
                for glyph in &run.glyphs {
                    writeln!(
                        output,
                        "  GLYPH id={} x={:.3} y={:.3} advance={:.3}",
                        glyph.id, glyph.x, glyph.y, glyph.advance
                    )
                    .unwrap();
                }
            }
        }
        for warning in &self.warnings {
            writeln!(output, "WARNING {warning}").unwrap();
        }
        output
    }
}

pub fn font_metadata(data: &[u8], index: u32) -> FontMetadata {
    use ttf_parser::{Face, name_id};

    let Ok(face) = Face::parse(data, index) else {
        return FontMetadata {
            full_name: "<unparseable>".into(),
            postscript_name: "<unparseable>".into(),
            version: "<unparseable>".into(),
            index,
            embedding: "unknown".into(),
            subsetting_allowed: false,
        };
    };
    let name = |id| {
        face.names()
            .into_iter()
            .filter(|name| name.name_id == id)
            .find_map(|name| name.to_string())
            .unwrap_or_else(|| "<unknown>".into())
    };
    FontMetadata {
        full_name: name(name_id::FULL_NAME),
        postscript_name: name(name_id::POST_SCRIPT_NAME),
        version: name(name_id::VERSION),
        index,
        embedding: format!("{:?}", face.permissions()),
        subsetting_allowed: face.is_subsetting_allowed(),
    }
}

pub fn contains_missing_glyph(run: &ShapedRun) -> bool {
    run.source
        .chars()
        .any(|character| !character.is_whitespace())
        && run.glyphs.iter().any(|glyph| glyph.id == 0)
}

impl Label {
    pub fn spans(&self) -> Vec<Span> {
        let mut spans = Vec::new();
        self.push_spans(&mut spans, Style::Upright, 1.0, 0.0);
        spans
    }

    fn push_spans(&self, spans: &mut Vec<Span>, inherited: Style, scale: f32, shift: f32) {
        match self {
            Self::Text(text) | Self::Upright(text) | Self::Unit(text) | Self::Operator(text) => {
                push(spans, text, Style::Upright, scale, shift)
            }
            Self::Variable(text) => push(spans, text, Style::Italic, scale, shift),
            Self::Greek(character) => push(spans, &character.to_string(), inherited, scale, shift),
            Self::Subscript { body, descriptive } => body.push_spans(
                spans,
                if *descriptive {
                    Style::Upright
                } else {
                    Style::Italic
                },
                scale * 0.72,
                shift + 0.22,
            ),
            Self::Superscript(body) => {
                body.push_spans(spans, inherited, scale * 0.72, shift - 0.38)
            }
            Self::Group(children) => {
                for child in children {
                    child.push_spans(spans, inherited, scale, shift);
                }
            }
        }
    }
}

fn push(spans: &mut Vec<Span>, text: &str, style: Style, scale: f32, baseline_shift_em: f32) {
    if text.is_empty() {
        return;
    }
    let next = Span {
        text: text.into(),
        style,
        scale,
        baseline_shift_em,
    };
    if let Some(previous) = spans.last_mut()
        && previous.style == next.style
        && previous.scale == next.scale
        && previous.baseline_shift_em == next.baseline_shift_em
    {
        previous.text.push_str(&next.text);
    } else {
        spans.push(next);
    }
}

pub fn validation_labels() -> Vec<(&'static str, Label)> {
    vec![
        (
            "plain-latin",
            Label::Text("Experiment / Fit / Theory".into()),
        ),
        (
            "italic-variable-upright-unit",
            Label::Group(vec![
                Label::Variable("J".into()),
                Label::Subscript {
                    body: Box::new(Label::Upright("e".into())),
                    descriptive: true,
                },
                Label::Text(" (".into()),
                Label::Unit("A m".into()),
                Label::Superscript(Box::new(Label::Operator("−2".into()))),
                Label::Text(")".into()),
            ]),
        ),
        (
            "greek-and-math-subscript",
            Label::Group(vec![
                Label::Greek('μ'),
                Label::Subscript {
                    body: Box::new(Label::Variable("0".into())),
                    descriptive: false,
                },
                Label::Variable("H".into()),
                Label::Subscript {
                    body: Box::new(Label::Upright("DL".into())),
                    descriptive: true,
                },
                Label::Text(" (".into()),
                Label::Unit("mT".into()),
                Label::Text(")".into()),
            ]),
        ),
        ("mixed-cjk-latin", Label::Text("温度 T (K)".into())),
        ("missing-glyph", Label::Text("missing: \u{10FFFF}".into())),
        (
            "rotated-y-label",
            Label::Text("Current density J_e (A m⁻²)".into()),
        ),
        ("legend", Label::Text("Experiment / Fit / Theory".into())),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_ranges_do_not_collapse_style() {
        let spans = validation_labels()[1].1.spans();
        assert_eq!(spans[0].style, Style::Italic);
        assert_eq!(spans[1].style, Style::Upright);
        assert!(spans[1].baseline_shift_em > 0.0);
        assert!(spans.iter().any(|span| span.baseline_shift_em < 0.0));
    }

    #[test]
    fn greek_remains_unicode() {
        let spans = validation_labels()[2].1.spans();
        assert!(spans.iter().any(|span| span.text.contains('μ')));
    }
}
