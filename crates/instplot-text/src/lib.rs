#[derive(Clone, Debug, PartialEq)]
pub enum Label {
    Text(String),
    Variable(String),
    Upright(String),
    GreekVariable(char),
    Number(String),
    DescriptiveSubscript(Box<Label>),
    VariableSubscript(Box<Label>),
    Superscript(Box<Label>),
    Unit(String),
    UnitSeparator,
    Operator(String),
    Emphasis(String),
    BoldVariable(String),
    Group(Vec<Label>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Upright,
    Italic,
    Bold,
    BoldItalic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpanFlow {
    Inline,
    Subscript(u32),
    Superscript(u32),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    pub text: String,
    pub style: Style,
    pub scale: f32,
    pub baseline_shift_em: f32,
    pub is_unit_separator: bool,
    pub flow: SpanFlow,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FontMetadata {
    pub full_name: String,
    pub postscript_name: String,
    pub version: String,
    pub index: u32,
    pub embedding: String,
    pub embedding_allowed: bool,
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
                    "RUN source={:?} font={:?} ps={:?} version={:?} index={} size={:.3} style={:?} embedding={} allowed={} subset={}",
                    run.source,
                    run.font.full_name,
                    run.font.postscript_name,
                    run.font.version,
                    run.font.index,
                    run.font_size,
                    run.style,
                    run.font.embedding,
                    run.font.embedding_allowed,
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
            embedding_allowed: false,
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
    let permissions = face.permissions();
    FontMetadata {
        full_name: name(name_id::FULL_NAME),
        postscript_name: name(name_id::POST_SCRIPT_NAME),
        version: name(name_id::VERSION),
        index,
        embedding: format!("{permissions:?}"),
        embedding_allowed: !matches!(permissions, Some(ttf_parser::Permissions::Restricted)),
        subsetting_allowed: face.is_subsetting_allowed(),
    }
}

pub fn contains_missing_glyph(run: &ShapedRun) -> bool {
    run.source
        .chars()
        .any(|character| !character.is_whitespace())
        && run.glyphs.iter().any(|glyph| glyph.id == 0)
}

pub fn unsupported_v1_script(text: &str) -> Option<char> {
    text.chars().find(|character| {
        matches!(
            *character as u32,
            0x3040..=0x30FF
                | 0x3400..=0x4DBF
                | 0x4E00..=0x9FFF
                | 0xAC00..=0xD7AF
                | 0xF900..=0xFAFF
                | 0x20000..=0x2FA1F
        )
    })
}

impl Label {
    pub fn normalized_text(&self) -> String {
        self.spans().into_iter().map(|span| span.text).collect()
    }

    pub fn spans(&self) -> Vec<Span> {
        let mut spans = Vec::new();
        self.push_spans(&mut spans, None, 1.0, 0.0);
        mark_combined_scripts(&mut spans);
        spans
    }

    fn push_spans(
        &self,
        spans: &mut Vec<Span>,
        style_override: Option<Style>,
        scale: f32,
        shift: f32,
    ) {
        match self {
            Self::Text(text) | Self::Upright(text) | Self::Operator(text) => push(
                spans,
                text,
                style_override.unwrap_or(Style::Upright),
                scale,
                shift,
            ),
            Self::Number(text) => push(spans, text, Style::Upright, scale, shift),
            Self::Variable(text) => push(
                spans,
                text,
                style_override.unwrap_or(Style::Italic),
                scale,
                shift,
            ),
            Self::GreekVariable(character) => push(
                spans,
                &character.to_string(),
                style_override.unwrap_or(Style::Italic),
                scale,
                shift,
            ),
            Self::Unit(text) => push(
                spans,
                &text.replace('\u{00B5}', "μ"),
                style_override.unwrap_or(Style::Upright),
                scale,
                shift,
            ),
            Self::UnitSeparator => spans.push(Span {
                text: "\u{202F}".into(),
                style: style_override.unwrap_or(Style::Upright),
                scale,
                baseline_shift_em: shift,
                is_unit_separator: true,
                flow: SpanFlow::Inline,
            }),
            Self::DescriptiveSubscript(body) => {
                body.push_spans(spans, Some(Style::Upright), scale * 0.72, shift + 0.22)
            }
            Self::VariableSubscript(body) => {
                body.push_spans(spans, Some(Style::Italic), scale * 0.72, shift + 0.22)
            }
            Self::Superscript(body) => {
                body.push_spans(spans, style_override, scale * 0.72, shift - 0.38)
            }
            Self::Emphasis(text) => push(
                spans,
                text,
                style_override.unwrap_or(Style::Bold),
                scale,
                shift,
            ),
            Self::BoldVariable(text) => push(
                spans,
                text,
                style_override.unwrap_or(Style::BoldItalic),
                scale,
                shift,
            ),
            Self::Group(children) => {
                for child in children {
                    child.push_spans(spans, style_override, scale, shift);
                }
            }
        }
    }
}

fn push(spans: &mut Vec<Span>, text: &str, style: Style, scale: f32, baseline_shift_em: f32) {
    let mut start = 0;
    for (offset, character) in text.char_indices() {
        if matches!(character, '≤' | '≥') {
            push_run(spans, &text[start..offset], style, scale, baseline_shift_em);
            push_run(
                spans,
                &text[offset..offset + character.len_utf8()],
                style,
                scale,
                baseline_shift_em,
            );
            start = offset + character.len_utf8();
        }
    }
    push_run(spans, &text[start..], style, scale, baseline_shift_em);
}

fn push_run(spans: &mut Vec<Span>, text: &str, style: Style, scale: f32, baseline_shift_em: f32) {
    if text.is_empty() {
        return;
    }
    let next = Span {
        text: text.into(),
        style,
        scale,
        baseline_shift_em,
        is_unit_separator: false,
        flow: SpanFlow::Inline,
    };
    if let Some(previous) = spans.last_mut()
        && !previous.is_unit_separator
        && previous.style == next.style
        && previous.scale == next.scale
        && previous.baseline_shift_em == next.baseline_shift_em
        && !matches!(previous.text.as_str(), "≤" | "≥")
        && !matches!(next.text.as_str(), "≤" | "≥")
    {
        previous.text.push_str(&next.text);
    } else {
        spans.push(next);
    }
}

fn mark_combined_scripts(spans: &mut [Span]) {
    let mut at = 0;
    let mut group = 0_u32;
    while at < spans.len() {
        if spans[at].baseline_shift_em == 0.0 {
            at += 1;
            continue;
        }
        let start = at;
        while at < spans.len() && spans[at].baseline_shift_em != 0.0 {
            at += 1;
        }
        let block = &mut spans[start..at];
        let has_subscript = block.iter().any(|span| span.baseline_shift_em > 0.0);
        let has_superscript = block.iter().any(|span| span.baseline_shift_em < 0.0);
        if has_subscript && has_superscript {
            for span in block {
                span.flow = if span.baseline_shift_em > 0.0 {
                    SpanFlow::Subscript(group)
                } else {
                    SpanFlow::Superscript(group)
                };
            }
            group += 1;
        }
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
                Label::VariableSubscript(Box::new(Label::Variable("e".into()))),
                Label::Text(" (".into()),
                Label::Unit("A".into()),
                Label::UnitSeparator,
                Label::Unit("m".into()),
                Label::Superscript(Box::new(Label::Number("−2".into()))),
                Label::Text(")".into()),
            ]),
        ),
        (
            "greek-and-math-subscript",
            Label::Group(vec![
                Label::GreekVariable('μ'),
                Label::VariableSubscript(Box::new(Label::Number("0".into()))),
                Label::Variable("H".into()),
                Label::DescriptiveSubscript(Box::new(Label::Upright("DL".into()))),
                Label::Text(" (".into()),
                Label::Unit("mT".into()),
                Label::Text(")".into()),
            ]),
        ),
        ("unsupported-cjk", Label::Text("温度 T (K)".into())),
        ("missing-glyph", Label::Text("missing: \u{10FFFF}".into())),
        (
            "rotated-y-label",
            Label::Group(vec![
                Label::Text("Current density ".into()),
                Label::Variable("J".into()),
                Label::VariableSubscript(Box::new(Label::Variable("e".into()))),
                Label::Text(" (".into()),
                Label::Unit("A".into()),
                Label::UnitSeparator,
                Label::Unit("m".into()),
                Label::Superscript(Box::new(Label::Number("−2".into()))),
                Label::Text(")".into()),
            ]),
        ),
        ("legend", Label::Text("Experiment / Fit / Theory".into())),
        ("panel-label", Label::Emphasis("(a)".into())),
        ("bold-variable", Label::BoldVariable("H".into())),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_ranges_do_not_collapse_style() {
        let spans = validation_labels()[1].1.spans();
        assert_eq!(spans[0].style, Style::Italic);
        assert_eq!(spans[1].style, Style::Italic);
        assert!(spans[1].baseline_shift_em > 0.0);
        assert!(spans.iter().any(|span| span.baseline_shift_em < 0.0));
    }

    #[test]
    fn numeric_subscript_stays_upright_while_variable_subscript_is_italic() {
        let spans = Label::Group(vec![
            Label::GreekVariable('μ'),
            Label::VariableSubscript(Box::new(Label::Number("0".into()))),
            Label::Variable("H".into()),
            Label::VariableSubscript(Box::new(Label::Variable("z".into()))),
        ])
        .spans();
        assert_eq!(spans[0].style, Style::Italic);
        assert_eq!(spans[1].style, Style::Upright);
        assert!(spans[1].baseline_shift_em > 0.0);
        assert_eq!(spans[3].style, Style::Italic);
        assert!(spans[3].baseline_shift_em > 0.0);
    }

    #[test]
    fn adjacent_subscript_and_superscript_share_one_horizontal_column() {
        let label = Label::Group(vec![
            Label::Variable("R".into()),
            Label::VariableSubscript(Box::new(Label::Variable("x".into()))),
            Label::Superscript(Box::new(Label::Variable("y".into()))),
            Label::Text(" next".into()),
        ]);
        let spans = label.spans();
        assert_eq!(spans[0].flow, SpanFlow::Inline);
        assert_eq!(spans[1].flow, SpanFlow::Subscript(0));
        assert_eq!(spans[2].flow, SpanFlow::Superscript(0));
        assert_eq!(spans[3].flow, SpanFlow::Inline);
    }

    #[test]
    fn greek_remains_unicode() {
        let spans = validation_labels()[2].1.spans();
        assert!(spans.iter().any(|span| span.text.contains('μ')));
        assert_eq!(spans[0].style, Style::Italic);
        assert_eq!(spans[1].text, "0");
        assert_eq!(spans[1].style, Style::Upright);
    }

    #[test]
    fn legacy_micro_sign_is_normalized_in_units() {
        let spans = Label::Unit("µm".into()).spans();
        assert_eq!(spans[0].text, "μm");
        assert_eq!(spans[0].style, Style::Upright);
    }

    #[test]
    fn relation_symbols_are_isolated_without_changing_variable_style() {
        let spans = Label::Group(vec![
            Label::Variable("T".into()),
            Label::Text(" ≤ 300 K and ≥ 2 K".into()),
        ])
        .spans();
        assert_eq!(spans[0].style, Style::Italic);
        assert_eq!(spans[2].text, "≤");
        assert_eq!(spans[2].style, Style::Upright);
        assert_eq!(spans[4].text, "≥");
        assert_eq!(spans[4].style, Style::Upright);
    }

    #[test]
    fn cjk_is_an_explicitly_unsupported_v1_script() {
        assert_eq!(unsupported_v1_script("温度 T (K)"), Some('温'));
        assert_eq!(unsupported_v1_script("μ₀H_DL (mT)"), None);
    }
}
