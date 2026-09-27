//! Small, deliberately bounded label syntax. This is not a TeX interpreter.

use crate::LabelNode;
use std::sync::OnceLock;

const REGULAR: &[u8] =
    include_bytes!("../../../crates/instplot-text/assets/fonts/TeXGyreHeros-Regular.otf");
const ITALIC: &[u8] =
    include_bytes!("../../../crates/instplot-text/assets/fonts/TeXGyreHeros-Italic.otf");
const BOLD: &[u8] =
    include_bytes!("../../../crates/instplot-text/assets/fonts/TeXGyreHeros-Bold.otf");
const BOLD_ITALIC: &[u8] =
    include_bytes!("../../../crates/instplot-text/assets/fonts/TeXGyreHeros-BoldItalic.otf");
const RELATION_FONT: &[u8] =
    include_bytes!("../../../crates/instplot-text/assets/fonts/STIXTwoMath-Regular.otf");

pub const SYMBOLS: &[(&str, &str)] = &[
    ("α", "\\alpha"),
    ("β", "\\beta"),
    ("γ", "\\gamma"),
    ("δ", "\\delta"),
    ("ε", "\\epsilon"),
    ("ζ", "\\zeta"),
    ("η", "\\eta"),
    ("θ", "\\theta"),
    ("ι", "\\iota"),
    ("κ", "\\kappa"),
    ("λ", "\\lambda"),
    ("μ", "\\mu"),
    ("ν", "\\nu"),
    ("ξ", "\\xi"),
    ("ο", "\\omicron"),
    ("π", "\\pi"),
    ("ρ", "\\rho"),
    ("σ", "\\sigma"),
    ("τ", "\\tau"),
    ("υ", "\\upsilon"),
    ("φ", "\\phi"),
    ("χ", "\\chi"),
    ("ψ", "\\psi"),
    ("ω", "\\omega"),
    ("ϵ", "\\varepsilon"),
    ("ϑ", "\\vartheta"),
    ("ϕ", "\\varphi"),
    ("ϖ", "\\varpi"),
    ("ς", "\\varsigma"),
    ("Γ", "\\Gamma"),
    ("Δ", "\\Delta"),
    ("Θ", "\\Theta"),
    ("Λ", "\\Lambda"),
    ("Ξ", "\\Xi"),
    ("Π", "\\Pi"),
    ("Σ", "\\Sigma"),
    ("Φ", "\\Phi"),
    ("Ψ", "\\Psi"),
    ("Ω", "\\Omega"),
    ("+", "\\plus"),
    ("−", "\\minus"),
    ("±", "\\pm"),
    ("∓", "\\mp"),
    ("×", "\\times"),
    ("·", "\\cdot"),
    ("÷", "\\div"),
    ("=", "\\equals"),
    ("<", "\\lt"),
    (">", "\\gt"),
    ("≤", "\\leq"),
    ("≥", "\\geq"),
    ("≠", "\\neq"),
    ("≈", "\\approx"),
    ("∑", "\\sum"),
    ("∞", "\\infty"),
    ("∂", "\\partial"),
    ("√", "\\sqrt"),
    ("°", "\\degree"),
    ("%", "\\percent"),
    ("Å", "\\angstrom"),
    ("←", "\\leftarrow"),
    ("→", "\\rightarrow"),
    ("↑", "\\uparrow"),
    ("↓", "\\downarrow"),
];

pub const GREEK_LOWER_END: usize = 29;
pub const GREEK_UPPER_END: usize = 39;
pub const MATH_RELATIONS_END: usize = 53;

const SYMBOL_ALIASES: &[(&str, &str)] = &[
    ("le", "≤"),
    ("ge", "≥"),
    ("ne", "≠"),
    ("Sigma", "Σ"),
    ("Pi", "Π"),
    ("Gamma", "Γ"),
    ("Phi", "Φ"),
    ("Psi", "Ψ"),
    ("Lambda", "Λ"),
    ("Theta", "Θ"),
    ("Xi", "Ξ"),
];

pub fn parse(input: &str) -> Result<Vec<LabelNode>, String> {
    if input.is_empty() {
        return Err("标签不能为空".to_owned());
    }
    let normalized = input.replace('µ', "μ");
    if let Some((prefix, units)) = trailing_parenthesized_units(&normalized)
        && let Some(unit_nodes) = parse_unit_expression(units)
    {
        let mut nodes = parse_sequence(prefix)?;
        nodes.push(LabelNode::Text(" (".to_owned()));
        nodes.extend(unit_nodes);
        nodes.push(LabelNode::Text(")".to_owned()));
        return Ok(nodes);
    }
    let mut nodes = parse_sequence(&normalized)?;
    normalize_plain_number_after_operator(&mut nodes);
    normalize_trailing_unit(&mut nodes);
    Ok(nodes)
}

// A comparison such as "$T$ <= 300 K" does not need math delimiters around
// the number or its unit. Keep unrelated prose (for example "sample 300 K")
// untouched so that existing label semantics remain stable.
fn normalize_plain_number_after_operator(nodes: &mut Vec<LabelNode>) {
    let mut index = 0;
    while index + 1 < nodes.len() {
        if !matches!(nodes[index], LabelNode::Operator(_)) {
            index += 1;
            continue;
        }
        let Some(LabelNode::Text(text)) = nodes.get(index + 1) else {
            index += 1;
            continue;
        };
        let Some(rest) = text.strip_prefix(' ') else {
            index += 1;
            continue;
        };
        let number_len = rest
            .char_indices()
            .take_while(|(_, c)| c.is_ascii_digit() || *c == '.')
            .last()
            .map(|(offset, c)| offset + c.len_utf8())
            .unwrap_or(0);
        if number_len == 0 {
            index += 1;
            continue;
        }
        let (number, tail) = rest.split_at(number_len);
        let replacement = if tail.is_empty() {
            Some(vec![
                LabelNode::Text(" ".into()),
                LabelNode::Number(number.into()),
            ])
        } else if let Some(unit) = tail.strip_prefix(' ')
            && !unit.is_empty()
            && unit.chars().all(char::is_alphabetic)
        {
            Some(vec![
                LabelNode::Text(" ".into()),
                LabelNode::Number(number.into()),
                LabelNode::Text(" ".into()),
                LabelNode::Unit(unit.into()),
            ])
        } else {
            None
        };
        if let Some(replacement) = replacement {
            nodes.splice(index + 1..index + 2, replacement);
        }
        index += 1;
    }
}

fn parse_sequence(input: &str) -> Result<Vec<LabelNode>, String> {
    let mut parser = Parser {
        chars: input.chars().collect(),
        at: 0,
    };
    let nodes = parser.sequence(false, None)?;
    if nodes.is_empty() {
        return Err("标签不能为空".to_owned());
    }
    Ok(nodes)
}

fn trailing_parenthesized_units(input: &str) -> Option<(&str, &str)> {
    let start = input.rfind(" (")?;
    let units = input.get(start + 2..input.len().checked_sub(1)?)?;
    (input.ends_with(')') && !units.is_empty() && !units.contains(['(', ')']))
        .then_some((&input[..start], units))
}

fn parse_unit_expression(input: &str) -> Option<Vec<LabelNode>> {
    let mut nodes = Vec::new();
    for factor in input.split_whitespace() {
        if factor.contains(['\\', '{', '}']) && !factor.contains("^{") {
            return None;
        }
        if !nodes.is_empty() {
            nodes.push(LabelNode::UnitSeparator);
        }
        let (unit, exponent) = if let Some((unit, exponent)) = factor.split_once("^{") {
            (unit, Some(exponent.strip_suffix('}')?.to_owned()))
        } else if let Some((unit, exponent)) = factor.split_once('^') {
            (unit, Some(exponent.to_owned()))
        } else if let Some((index, _)) = factor
            .char_indices()
            .find(|(_, c)| unicode_superscript(*c).is_some())
        {
            let exponent = factor[index..]
                .chars()
                .map(unicode_superscript)
                .collect::<Option<String>>()?;
            (factor.get(..index)?, Some(exponent))
        } else {
            (factor, None)
        };
        if unit.is_empty() || unit.contains(['^', '_', '\\', '{', '}']) {
            return None;
        }
        nodes.push(LabelNode::Unit(unit.to_owned()));
        if let Some(exponent) = exponent {
            if exponent.is_empty()
                || !exponent
                    .chars()
                    .all(|c| c.is_ascii_digit() || matches!(c, '-' | '−' | '+'))
            {
                return None;
            }
            nodes.push(LabelNode::Superscript(vec![LabelNode::Number(
                exponent.replace('-', "−"),
            )]));
        }
    }
    (!nodes.is_empty()).then_some(nodes)
}

fn normalize_trailing_unit(nodes: &mut Vec<LabelNode>) {
    if nodes.len() < 2 || !matches!(nodes[nodes.len() - 2], LabelNode::Number(_)) {
        return;
    }
    let Some(LabelNode::Text(text)) = nodes.last() else {
        return;
    };
    let Some(unit) = text.strip_prefix(' ') else {
        return;
    };
    if unit.is_empty() || unit.contains(' ') || unit.contains(['\\', '_', '^', '{', '}']) {
        return;
    }
    let unit = unit.to_owned();
    nodes.pop();
    nodes.push(LabelNode::Text(" ".to_owned()));
    nodes.push(LabelNode::Unit(unit));
}

pub fn display_text(nodes: &[LabelNode]) -> String {
    fn append(nodes: &[LabelNode], out: &mut String) {
        for node in nodes {
            match node {
                LabelNode::Text(s)
                | LabelNode::Variable(s)
                | LabelNode::Upright(s)
                | LabelNode::Number(s)
                | LabelNode::Unit(s)
                | LabelNode::Operator(s)
                | LabelNode::Emphasis(s)
                | LabelNode::BoldVariable(s) => out.push_str(s),
                LabelNode::GreekVariable(c) => out.push(*c),
                LabelNode::DescriptiveSubscript(inner)
                | LabelNode::VariableSubscript(inner)
                | LabelNode::Superscript(inner) => append(inner, out),
                LabelNode::UnitSeparator => out.push(' '),
            }
        }
    }
    let mut result = String::new();
    append(nodes, &mut result);
    result
}

/// Refuse a glyph that the bundled, semantically selected face cannot render.
pub fn validate_glyphs(nodes: &[LabelNode]) -> Result<(), String> {
    static FACES: OnceLock<Result<Vec<ttf_parser::Face<'static>>, String>> = OnceLock::new();
    let faces = FACES
        .get_or_init(|| {
            [REGULAR, ITALIC, BOLD, BOLD_ITALIC, RELATION_FONT]
                .map(|bytes| {
                    ttf_parser::Face::parse(bytes, 0).map_err(|_| "内置字体无法读取".to_owned())
                })
                .into_iter()
                .collect()
        })
        .as_ref()
        .map_err(Clone::clone)?;
    let label = crate::layout_label_from_nodes(nodes);
    for span in label.spans() {
        if span.is_unit_separator {
            continue;
        }
        let face = if matches!(span.text.as_str(), "≤" | "≥") {
            4
        } else {
            match span.style {
                instplot_text::Style::Upright => 0,
                instplot_text::Style::Italic => 1,
                instplot_text::Style::Bold => 2,
                instplot_text::Style::BoldItalic => 3,
            }
        };
        for c in span.text.chars() {
            if matches!(c, '\n' | '\r') {
                continue;
            }
            if faces[face].glyph_index(c).is_none() {
                return Err(format!("内置字体缺少符号 {c} (U+{:04X})", c as u32));
            }
        }
    }
    Ok(())
}

/// Return readable markup only when parsing it preserves the semantic label.
/// Units are shown as ordinary text, with their unit semantics restored by the parser.
pub fn format(nodes: &[LabelNode]) -> Option<String> {
    let minimal = format_with_mode(nodes, true)?;
    if parse(&minimal).ok().as_deref() == Some(nodes) {
        return Some(minimal);
    }
    let explicit = format_with_mode(nodes, false)?;
    (parse(&explicit).ok().as_deref() == Some(nodes)).then_some(explicit)
}

fn format_with_mode(nodes: &[LabelNode], minimal: bool) -> Option<String> {
    let mut out = String::new();
    let mut math = false;
    let mut previous_is_unit = false;
    for node in nodes {
        let is_unit_exponent = previous_is_unit && matches!(node, LabelNode::Superscript(_));
        let wants_math = !is_unit_exponent
            && if minimal {
                matches!(
                    node,
                    LabelNode::Variable(_)
                        | LabelNode::Upright(_)
                        | LabelNode::GreekVariable(_)
                        | LabelNode::VariableSubscript(_)
                        | LabelNode::BoldVariable(_)
                ) || matches!(node, LabelNode::Superscript(inner) if script_has_variable(inner))
            } else {
                matches!(
                    node,
                    LabelNode::Variable(_)
                        | LabelNode::Upright(_)
                        | LabelNode::GreekVariable(_)
                        | LabelNode::Number(_)
                        | LabelNode::DescriptiveSubscript(_)
                        | LabelNode::VariableSubscript(_)
                        | LabelNode::Superscript(_)
                        | LabelNode::Operator(_)
                        | LabelNode::BoldVariable(_)
                )
            };
        if wants_math != math {
            out.push('$');
            math = wants_math;
        }
        match node {
            LabelNode::Text(s) => out.push_str(&escape(s)),
            LabelNode::Variable(s) => out.push_str(&escape(s)),
            LabelNode::Upright(s) => out.push_str(&format!("\\mathrm{{{}}}", escape(s))),
            LabelNode::GreekVariable(c) => {
                out.push(*c);
            }
            LabelNode::Number(s) => out.push_str(&escape(s)),
            LabelNode::DescriptiveSubscript(inner) => {
                out.push_str("_{");
                out.push_str(&escape(&display_text(inner)));
                out.push('}');
            }
            LabelNode::VariableSubscript(inner) => {
                out.push_str("_{");
                out.push_str(&format_script(inner)?);
                out.push('}');
            }
            LabelNode::Superscript(inner) => {
                out.push_str("^{");
                if math
                    || inner
                        .iter()
                        .any(|node| matches!(node, LabelNode::Upright(_)))
                {
                    out.push_str(&format_script(inner)?);
                } else {
                    out.push_str(&escape(&display_text(inner)));
                }
                out.push('}');
            }
            LabelNode::Unit(s) => out.push_str(&escape(s)),
            LabelNode::UnitSeparator => out.push(' '),
            LabelNode::Operator(s) => out.push_str(&escape(s)),
            LabelNode::Emphasis(s) => out.push_str(&format!("\\emph{{{}}}", escape(s))),
            LabelNode::BoldVariable(s) => out.push_str(&format!("\\mathbf{{{}}}", escape(s))),
        }
        previous_is_unit = matches!(node, LabelNode::Unit(_));
    }
    if math {
        out.push('$');
    }
    Some(out)
}

fn script_has_variable(nodes: &[LabelNode]) -> bool {
    nodes.iter().any(|node| {
        matches!(
            node,
            LabelNode::Variable(_) | LabelNode::GreekVariable(_) | LabelNode::BoldVariable(_)
        )
    })
}

fn format_script(nodes: &[LabelNode]) -> Option<String> {
    let mut out = String::new();
    for node in nodes {
        match node {
            LabelNode::Variable(s) | LabelNode::Number(s) => out.push_str(&escape(s)),
            LabelNode::GreekVariable(c) => out.push(*c),
            LabelNode::Text(s) | LabelNode::Upright(s) => {
                out.push_str(&format!("\\mathrm{{{}}}", escape(s)));
            }
            _ => return None,
        }
    }
    Some(out)
}

fn escape(input: &str) -> String {
    let mut out = String::new();
    for c in input.chars() {
        if matches!(c, '$' | '\\' | '_' | '^' | '{' | '}' | '*') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

struct Parser {
    chars: Vec<char>,
    at: usize,
}

struct ParsedScript {
    nodes: Vec<LabelNode>,
    explicit_variable: bool,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.at).copied()
    }
    fn take(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.at += 1;
        Some(c)
    }

    fn sequence(&mut self, mut math: bool, stop: Option<char>) -> Result<Vec<LabelNode>, String> {
        let mut nodes = Vec::new();
        while let Some(c) = self.peek() {
            if Some(c) == stop {
                self.take();
                return Ok(merge_text(nodes));
            }
            match c {
                '$' => {
                    self.take();
                    math = !math;
                }
                '\\' => nodes.push(self.command(math)?),
                '*' => return Err("星号需要写成 \\*".to_owned()),
                '_' | '^' => {
                    self.take();
                    let script = self.script(math)?;
                    let inner = script.nodes;
                    if c == '^' {
                        nodes.push(LabelNode::Superscript(inner));
                    } else if inner
                        .iter()
                        .all(|n| matches!(n, LabelNode::Upright(_) | LabelNode::Text(_)))
                    {
                        nodes.push(LabelNode::DescriptiveSubscript(
                            inner
                                .into_iter()
                                .map(|n| match n {
                                    LabelNode::Upright(s) => LabelNode::Text(s),
                                    other => other,
                                })
                                .collect(),
                        ));
                    } else if math || script.explicit_variable {
                        // Inside explicit math delimiters, follow TeX semantics:
                        // Latin and Greek script variables remain italic regardless
                        // of how many letters the script contains.
                        nodes.push(LabelNode::VariableSubscript(inner));
                    } else {
                        nodes.push(LabelNode::DescriptiveSubscript(inner));
                    }
                }
                c if unicode_subscript(c).is_some() => {
                    let mut value = String::new();
                    while let Some(mapped) = self.peek().and_then(unicode_subscript) {
                        self.take();
                        value.push(mapped);
                    }
                    let inner = vec![LabelNode::Number(value)];
                    nodes.push(if math {
                        LabelNode::VariableSubscript(inner)
                    } else {
                        LabelNode::DescriptiveSubscript(inner)
                    });
                }
                c if unicode_superscript(c).is_some() => {
                    let mut value = String::new();
                    while let Some(mapped) = self.peek().and_then(unicode_superscript) {
                        self.take();
                        value.push(mapped);
                    }
                    nodes.push(LabelNode::Superscript(vec![LabelNode::Number(value)]));
                }
                '<' if self.chars.get(self.at + 1) == Some(&'=') => {
                    self.at += 2;
                    nodes.push(LabelNode::Operator("≤".to_owned()));
                }
                '>' if self.chars.get(self.at + 1) == Some(&'=') => {
                    self.at += 2;
                    nodes.push(LabelNode::Operator("≥".to_owned()));
                }
                '!' if self.chars.get(self.at + 1) == Some(&'=') => {
                    self.at += 2;
                    nodes.push(LabelNode::Operator("≠".to_owned()));
                }
                _ => nodes.push(self.atom(math)),
            }
        }
        if stop.is_some() {
            return Err("缺少右花括号 }".to_owned());
        }
        if math {
            return Err("缺少结束的 $".to_owned());
        }
        Ok(merge_text(nodes))
    }

    fn atom(&mut self, math: bool) -> LabelNode {
        let first = self.take().expect("atom needs a character");
        if !math {
            if first == '-' && self.peek().is_some_and(|c| c.is_ascii_digit()) {
                let mut value = "−".to_owned();
                while self.peek().is_some_and(|c| c.is_ascii_digit() || c == '.') {
                    value.push(self.take().unwrap());
                }
                return LabelNode::Number(value);
            }
            if is_operator(first) && first != '-' {
                return LabelNode::Operator(first.to_string());
            }
            return LabelNode::Text(first.to_string());
        }
        if first.is_ascii_alphabetic() {
            let mut value = first.to_string();
            while self.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
                value.push(self.take().unwrap());
            }
            return LabelNode::Variable(value);
        }
        if first.is_ascii_digit()
            || first == '−'
            || (first == '-' && self.peek().is_some_and(|c| c.is_ascii_digit()))
        {
            let mut value = if first == '-' {
                "−".to_owned()
            } else {
                first.to_string()
            };
            while self.peek().is_some_and(|c| c.is_ascii_digit() || c == '.') {
                value.push(self.take().unwrap());
            }
            return LabelNode::Number(value);
        }
        if first == '-' {
            return LabelNode::Operator("−".to_owned());
        }
        if is_greek(first) {
            return LabelNode::GreekVariable(first);
        }
        if is_operator(first) {
            return LabelNode::Operator(first.to_string());
        }
        LabelNode::Text(first.to_string())
    }

    fn command(&mut self, math: bool) -> Result<LabelNode, String> {
        self.take(); // backslash
        let Some(c) = self.peek() else {
            return Err("反斜杠后缺少命令".to_owned());
        };
        if !c.is_ascii_alphabetic() {
            self.take();
            return Ok(LabelNode::Text(c.to_string()));
        }
        let mut name = String::new();
        while self.peek().is_some_and(|ch| ch.is_ascii_alphabetic()) {
            name.push(self.take().unwrap());
        }
        if let Some((symbol, _)) = SYMBOLS
            .iter()
            .find(|(_, command)| command.trim_start_matches('\\') == name)
        {
            let c = symbol.chars().next().unwrap();
            return Ok(if is_greek(c) && math {
                LabelNode::GreekVariable(c)
            } else if is_greek(c) {
                LabelNode::Text(symbol.to_string())
            } else {
                LabelNode::Operator(symbol.to_string())
            });
        }
        let alias = SYMBOL_ALIASES
            .iter()
            .find(|(alias, _)| *alias == name)
            .map(|(_, symbol)| *symbol);
        if let Some(symbol) = alias {
            let c = symbol.chars().next().unwrap();
            return Ok(if is_greek(c) && math {
                LabelNode::GreekVariable(c)
            } else if is_greek(c) {
                LabelNode::Text(symbol.to_owned())
            } else {
                LabelNode::Operator(symbol.to_owned())
            });
        }
        if name == "unitsep" {
            if self.peek() == Some(' ') {
                self.take();
            }
            return Ok(LabelNode::UnitSeparator);
        }
        if !matches!(
            name.as_str(),
            "mathrm"
                | "upright"
                | "mathit"
                | "var"
                | "mathbf"
                | "boldvar"
                | "text"
                | "num"
                | "unit"
                | "op"
                | "emph"
        ) {
            return Err(format!("不支持的命令 \\{name}"));
        }
        let value = self.braced_literal()?;
        Ok(match name.as_str() {
            "mathrm" | "upright" => LabelNode::Upright(value),
            "mathit" | "var" => LabelNode::Variable(value),
            "mathbf" | "boldvar" => LabelNode::BoldVariable(value),
            "text" => LabelNode::Text(value),
            "num" => LabelNode::Number(value),
            "unit" => LabelNode::Unit(value),
            "op" => LabelNode::Operator(value),
            "emph" => LabelNode::Emphasis(value),
            _ => return Err(format!("不支持的命令 \\{name}")),
        })
    }

    fn braced_literal(&mut self) -> Result<String, String> {
        if self.take() != Some('{') {
            return Err("命令后需要 {内容}".to_owned());
        }
        let mut value = String::new();
        while let Some(c) = self.take() {
            if c == '}' {
                return Ok(value);
            }
            if c == '\\' {
                let next = self.take().ok_or("反斜杠后缺少字符")?;
                if !matches!(next, '$' | '\\' | '_' | '^' | '{' | '}' | '*') {
                    return Err("花括号内只允许普通文字；请把符号写在外面".to_owned());
                }
                value.push(next);
            } else {
                value.push(c);
            }
        }
        Err("缺少右花括号 }".to_owned())
    }

    fn script(&mut self, math: bool) -> Result<ParsedScript, String> {
        let explicit_variable = self.script_starts_with_explicit_variable_command();
        let nodes = if self.peek() == Some('{') {
            self.take();
            let nodes = self.sequence(math, Some('}'))?;
            if nodes.is_empty() {
                return Err("上下标不能为空".to_owned());
            }
            nodes
        } else if self.peek() == Some('\\') {
            vec![self.command(math)?]
        } else if !math && self.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
            let mut value = String::new();
            while self.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
                value.push(self.take().unwrap());
            }
            vec![LabelNode::Text(value)]
        } else if !math && self.peek().is_some_and(|c| c.is_ascii_digit()) {
            let mut value = String::new();
            while self.peek().is_some_and(|c| c.is_ascii_digit() || c == '.') {
                value.push(self.take().unwrap());
            }
            vec![LabelNode::Number(value)]
        } else if self.peek().is_some() {
            vec![self.atom(math)]
        } else {
            return Err("上下标缺少内容".to_owned());
        };
        Ok(ParsedScript {
            nodes,
            explicit_variable,
        })
    }

    fn script_starts_with_explicit_variable_command(&self) -> bool {
        let mut index = self.at;
        if self.chars.get(index) == Some(&'{') {
            index += 1;
        }
        if self.chars.get(index) != Some(&'\\') {
            return false;
        }
        index += 1;
        let name = self.chars[index..]
            .iter()
            .take_while(|c| c.is_ascii_alphabetic())
            .collect::<String>();
        matches!(name.as_str(), "mathit" | "var" | "mathbf" | "boldvar")
    }
}

fn is_greek(c: char) -> bool {
    matches!(c as u32, 0x0370..=0x03ff | 0x1f00..=0x1fff)
}

fn unicode_subscript(c: char) -> Option<char> {
    Some(match c {
        '₀' => '0',
        '₁' => '1',
        '₂' => '2',
        '₃' => '3',
        '₄' => '4',
        '₅' => '5',
        '₆' => '6',
        '₇' => '7',
        '₈' => '8',
        '₉' => '9',
        '₊' => '+',
        '₋' => '−',
        _ => return None,
    })
}

fn unicode_superscript(c: char) -> Option<char> {
    Some(match c {
        '⁰' => '0',
        '¹' => '1',
        '²' => '2',
        '³' => '3',
        '⁴' => '4',
        '⁵' => '5',
        '⁶' => '6',
        '⁷' => '7',
        '⁸' => '8',
        '⁹' => '9',
        '⁺' => '+',
        '⁻' => '−',
        _ => return None,
    })
}
fn merge_text(nodes: Vec<LabelNode>) -> Vec<LabelNode> {
    let mut merged = Vec::with_capacity(nodes.len());
    for node in nodes {
        if let LabelNode::Text(value) = node {
            if let Some(LabelNode::Text(previous)) = merged.last_mut() {
                previous.push_str(&value);
            } else {
                merged.push(LabelNode::Text(value));
            }
        } else {
            merged.push(node);
        }
    }
    merged
}

fn is_operator(c: char) -> bool {
    matches!(
        c,
        '+' | '−'
            | '-'
            | '±'
            | '∓'
            | '×'
            | '·'
            | '÷'
            | '='
            | '<'
            | '>'
            | '≤'
            | '≥'
            | '≠'
            | '≈'
            | '∞'
            | '∂'
            | '√'
            | '∑'
            | '°'
            | '%'
            | 'Å'
            | '←'
            | '→'
            | '↑'
            | '↓'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_math_inputs_keep_semantics() {
        assert_eq!(
            parse(r"$\sigma_0 H_z \sum <= 3$").unwrap(),
            vec![
                LabelNode::GreekVariable('σ'),
                LabelNode::VariableSubscript(vec![LabelNode::Number("0".into())]),
                LabelNode::Text(" ".into()),
                LabelNode::Variable("H".into()),
                LabelNode::VariableSubscript(vec![LabelNode::Variable("z".into())]),
                LabelNode::Text(" ".into()),
                LabelNode::Operator("∑".into()),
                LabelNode::Text(" ".into()),
                LabelNode::Operator("≤".into()),
                LabelNode::Text(" ".into()),
                LabelNode::Number("3".into()),
            ]
        );
    }

    #[test]
    fn descriptive_subscript_is_upright() {
        assert_eq!(
            parse(r"$H_{\mathrm{DL}}$").unwrap(),
            vec![
                LabelNode::Variable("H".into()),
                LabelNode::DescriptiveSubscript(vec![LabelNode::Text("DL".into())]),
            ]
        );
        assert_eq!(parse("H_{DL}").unwrap(), parse(r"H_{\mathrm{DL}}").unwrap());
    }

    #[test]
    fn explicit_math_delimiters_control_multi_letter_subscript_semantics() {
        let math = parse(r"$v_{sk}$").unwrap();
        assert_eq!(
            math,
            vec![
                LabelNode::Variable("v".into()),
                LabelNode::VariableSubscript(vec![LabelNode::Variable("sk".into())]),
            ]
        );
        let descriptive = parse(r"$v$_{sk}").unwrap();
        assert_eq!(
            descriptive,
            vec![
                LabelNode::Variable("v".into()),
                LabelNode::DescriptiveSubscript(vec![LabelNode::Text("sk".into())]),
            ]
        );
        assert_eq!(
            parse(r"$v_{\mathrm{sk}}$").unwrap(),
            vec![
                LabelNode::Variable("v".into()),
                LabelNode::DescriptiveSubscript(vec![LabelNode::Text("sk".into())]),
            ]
        );
        assert_eq!(parse(r"$v_sk$").unwrap(), math);
        assert_eq!(parse(r"$v$_sk").unwrap(), descriptive);
        assert_eq!(
            parse(r"v_{\mathit{sk}}").unwrap(),
            vec![
                LabelNode::Text("v".into()),
                LabelNode::VariableSubscript(vec![LabelNode::Variable("sk".into())]),
            ]
        );
        assert_eq!(format(&math).as_deref(), Some("$v_{sk}$"));
        assert_eq!(format(&descriptive).as_deref(), Some("$v$_{sk}"));
    }

    #[test]
    fn scripts_work_without_math_delimiters() {
        assert_eq!(
            parse("H_0").unwrap(),
            vec![
                LabelNode::Text("H".into()),
                LabelNode::DescriptiveSubscript(vec![LabelNode::Number("0".into())]),
            ]
        );
        assert_eq!(
            parse(r"H_{\mathrm{DL}}").unwrap(),
            vec![
                LabelNode::Text("H".into()),
                LabelNode::DescriptiveSubscript(vec![LabelNode::Text("DL".into())]),
            ]
        );
        let combined = parse("R_x^y").unwrap();
        assert_eq!(
            combined,
            vec![
                LabelNode::Text("R".into()),
                LabelNode::DescriptiveSubscript(vec![LabelNode::Text("x".into())]),
                LabelNode::Superscript(vec![LabelNode::Text("y".into())]),
            ]
        );
        assert_eq!(format(&combined).as_deref(), Some("R_{x}^{y}"));
        let math = parse("$R_x^y$").unwrap();
        assert_eq!(
            math,
            vec![
                LabelNode::Variable("R".into()),
                LabelNode::VariableSubscript(vec![LabelNode::Variable("x".into())]),
                LabelNode::Superscript(vec![LabelNode::Variable("y".into())]),
            ]
        );
        assert_eq!(format(&math).as_deref(), Some("$R_{x}^{y}$"));
        assert_eq!(parse("α").unwrap(), vec![LabelNode::Text("α".into())]);
        assert_eq!(parse("$α$").unwrap(), vec![LabelNode::GreekVariable('α')]);
    }

    #[test]
    fn plain_labels_units_and_pasted_symbols_need_no_extra_commands() {
        let plain = parse("Kerr signal (a.u.)").unwrap();
        assert_eq!(display_text(&plain), "Kerr signal (a.u.)");
        let scientific = parse("H_{DL} (mT) σ_0 ≤ ∑").unwrap();
        assert_eq!(display_text(&scientific), "HDL (mT) σ0 ≤ ∑");
        assert!(
            scientific
                .iter()
                .any(|node| matches!(node, LabelNode::DescriptiveSubscript(_)))
        );
        validate_glyphs(&scientific).unwrap();
        let unit_power = parse("Current density (A m^{-2})").unwrap();
        let spans = crate::layout_label_from_nodes(&unit_power).spans();
        assert!(spans.iter().any(|span| {
            span.text.ends_with("m") && span.style == instplot_text::Style::Upright
        }));
        let pasted = parse("µ₀H (A m⁻²)").unwrap();
        assert_eq!(pasted[0], LabelNode::Text("μ".to_owned()));
        assert_eq!(
            pasted[1],
            LabelNode::DescriptiveSubscript(vec![LabelNode::Number("0".to_owned())])
        );
        assert!(pasted.iter().any(|node| {
            matches!(node, LabelNode::Superscript(inner) if inner == &vec![LabelNode::Number("−2".to_owned())])
        }));
        validate_glyphs(&pasted).unwrap();
    }

    #[test]
    fn filenames_and_imported_column_titles_stay_plain_and_editable() {
        for value in ["E-dsk_1_5V.csv", "Diameter (nm)", "0.00083"] {
            let nodes = vec![LabelNode::Text(value.to_owned())];
            let editable = format(&nodes).unwrap_or_else(|| display_text(&nodes));
            assert_eq!(display_text(&parse(&editable).unwrap()), value);
        }
        assert_eq!(
            parse("-1").unwrap(),
            vec![LabelNode::Number("−1".to_owned())]
        );
    }

    #[test]
    fn comparison_number_operator_and_unit_need_no_math_delimiters() {
        let nodes = vec![
            LabelNode::Variable("T".into()),
            LabelNode::Text(" ".into()),
            LabelNode::Operator("≤".into()),
            LabelNode::Text(" ".into()),
            LabelNode::Number("300".into()),
            LabelNode::Text(" ".into()),
            LabelNode::Unit("K".into()),
        ];
        assert_eq!(format(&nodes).as_deref(), Some("$T$ ≤ 300 K"));
        assert_eq!(parse("$T$ <= 300 K").unwrap(), nodes);
        assert_eq!(parse("$T$ ≤ 300 K").unwrap(), nodes);
    }

    #[test]
    fn unsupported_commands_are_visible_errors() {
        assert!(parse(r"$\frac{1}{2}$").unwrap_err().contains("\\frac"));
        assert!(parse(r"$H_{$").is_err());
    }

    #[test]
    fn literal_star_requires_an_explicit_escape_and_round_trips() {
        assert_eq!(parse("*").unwrap_err(), "星号需要写成 \\*");
        let nodes = parse(r"\*").unwrap();
        assert_eq!(nodes, vec![LabelNode::Text("*".to_owned())]);
        assert_eq!(format(&nodes).as_deref(), Some(r"\*"));
        validate_glyphs(&nodes).unwrap();
    }

    #[test]
    fn hard_line_breaks_round_trip_for_annotations() {
        let nodes = parse("First line\n$H_{DL}$ <= 300 K").unwrap();
        assert_eq!(display_text(&nodes), "First line\nHDL ≤ 300 K");
        validate_glyphs(&nodes).unwrap();
        let editable = format(&nodes).expect("multiline label remains editable");
        assert!(editable.contains('\n'));
        assert_eq!(parse(&editable).unwrap(), nodes);
    }

    #[test]
    fn existing_unit_labels_use_plain_editable_text() {
        let nodes = vec![
            LabelNode::GreekVariable('μ'),
            LabelNode::VariableSubscript(vec![LabelNode::Number("0".into())]),
            LabelNode::Variable("H".into()),
            LabelNode::DescriptiveSubscript(vec![LabelNode::Text("DL".into())]),
            LabelNode::Text(" (".into()),
            LabelNode::Unit("mT".into()),
            LabelNode::Text(")".into()),
        ];
        let formatted = format(&nodes).unwrap();
        assert!(!formatted.contains("\\unit"));
        assert!(formatted.ends_with(" (mT)"));
        let parsed = parse(&formatted).unwrap();
        assert_eq!(display_text(&parsed), display_text(&nodes));
        validate_glyphs(&parsed).unwrap();
    }

    #[test]
    fn bundled_fonts_cover_requested_symbols_and_report_missing_glyphs() {
        validate_glyphs(&parse(r"$\sigma \sum <=$").unwrap()).unwrap();
        assert!(
            validate_glyphs(&parse("🪐").unwrap())
                .unwrap_err()
                .contains("U+1FA90")
        );
    }

    #[test]
    fn every_symbol_picker_item_has_a_valid_command_and_bundled_glyph() {
        assert_eq!(SYMBOLS[GREEK_LOWER_END - 1].0, "ς");
        assert_eq!(SYMBOLS[GREEK_UPPER_END - 1].0, "Ω");
        assert_eq!(SYMBOLS[MATH_RELATIONS_END - 1].0, "≈");
        for (symbol, command) in SYMBOLS {
            let from_command = parse(command).unwrap();
            let from_picker = parse(symbol).unwrap();
            assert_eq!(from_command, from_picker, "{command}");
            validate_glyphs(&from_picker).unwrap();
        }
    }

    #[test]
    fn every_manual_symbol_alias_has_the_expected_supported_glyph() {
        for (alias, symbol) in SYMBOL_ALIASES {
            let nodes = parse(&format!("\\{alias}")).unwrap();
            assert_eq!(nodes, parse(symbol).unwrap(), "{alias}");
            validate_glyphs(&nodes).unwrap();
        }
    }

    #[test]
    fn complete_v1_greek_and_scientific_cores_exist_in_all_four_faces() {
        use instplot_export::ResolvedItem;
        let core = concat!(
            "α β γ δ ε ζ η θ ι κ λ μ ν ξ ο π ρ σ τ υ φ χ ψ ω ",
            "Γ Δ Θ Λ Ξ Π Σ Φ Ψ Ω ϵ ϑ ϕ ϖ ς ",
            "+ − ± ∓ × · ÷ = < > ≤ ≥ ≠ ≈ ∞ ∂ √ ∑ ° % Å ← → ↑ ↓",
        );
        for (name, bytes) in [
            ("Regular", REGULAR),
            ("Italic", ITALIC),
            ("Bold", BOLD),
            ("BoldItalic", BOLD_ITALIC),
        ] {
            let face = ttf_parser::Face::parse(bytes, 0).unwrap();
            for symbol in core.chars().filter(|c| !c.is_whitespace()) {
                assert!(
                    face.glyph_index(symbol).is_some(),
                    "{name} lacks U+{:04X}",
                    symbol as u32
                );
            }
        }
        for symbol in core.chars().filter(|c| !c.is_whitespace()) {
            let nodes = parse(&symbol.to_string()).unwrap();
            validate_glyphs(&nodes).unwrap();
            let mut document = crate::FigureDocument::fixed();
            document
                .set_axis_label(crate::AxisDimension::X, nodes)
                .unwrap();
            let resolved = crate::resolve_document(&document).unwrap();
            let text = resolved
                .display
                .items
                .iter()
                .find_map(|item| {
                    let ResolvedItem::Text(text) = item else {
                        return None;
                    };
                    (text.text == symbol.to_string()).then_some(text)
                })
                .unwrap_or_else(|| panic!("symbol {symbol} did not resolve"));
            assert!(
                text.runs
                    .iter()
                    .flat_map(|run| &run.glyphs)
                    .all(|glyph| glyph.id != 0),
                "{symbol}"
            );
        }
    }

    #[test]
    fn every_symbol_picker_item_shapes_and_exports_without_missing_glyphs() {
        use instplot_export::ResolvedItem;
        for (symbol, command) in SYMBOLS {
            let mut document = crate::FigureDocument::fixed();
            let nodes = parse(command).unwrap();
            document
                .set_axis_label(crate::AxisDimension::X, nodes)
                .unwrap();
            let resolved = crate::resolve_document(&document).unwrap();
            let text = resolved
                .display
                .items
                .iter()
                .find_map(|item| {
                    let ResolvedItem::Text(text) = item else {
                        return None;
                    };
                    (text.text == *symbol).then_some(text)
                })
                .unwrap_or_else(|| panic!("missing resolved label for {command}"));
            assert_eq!(&text.text, symbol, "{command}");
            assert!(!text.runs.is_empty(), "{command}");
            for run in &text.runs {
                if matches!(*symbol, "≤" | "≥") {
                    assert_eq!(run.font.postscript_name, "STIXTwoMath-Regular", "{command}");
                } else {
                    assert!(
                        run.font.postscript_name.starts_with("TeXGyreHeros-"),
                        "{command}"
                    );
                }
                assert!(run.glyphs.iter().all(|glyph| glyph.id != 0), "{command}");
            }
            let pdf = crate::figure_pdf(&document).unwrap();
            assert!(
                pdf.windows(9).any(|bytes| bytes == b"/FontFile"),
                "{command}"
            );
        }
    }

    #[test]
    fn manual_latex_styles_scripts_and_ascii_relations_are_semantic() {
        use instplot_text::Style;
        let cases = [
            (r"$H$", "H", Style::Italic, 1.0, 0),
            (r"\mathrm{H}", "H", Style::Upright, 1.0, 0),
            (r"\sigma", "σ", Style::Upright, 1.0, 0),
            (r"$\sigma$", "σ", Style::Italic, 1.0, 0),
            (r"H_0", "0", Style::Upright, 0.72, 1),
            (r"H_z", "z", Style::Upright, 0.72, 1),
            (r"$H_z$", "z", Style::Italic, 0.72, 1),
            (r"H_{\mathrm{DL}}", "DL", Style::Upright, 0.72, 1),
            (r"$v_sk$", "sk", Style::Italic, 0.72, 1),
            (r"$v$_sk", "sk", Style::Upright, 0.72, 1),
            (r"v_{\mathit{sk}}", "sk", Style::Italic, 0.72, 1),
            (r"10^{-3}", "−3", Style::Upright, 0.72, -1),
            (r"\unit{μΩ}", "μΩ", Style::Upright, 1.0, 0),
            (r"\mathbf{H}", "H", Style::BoldItalic, 1.0, 0),
            (r"\emph{Fit}", "Fit", Style::Bold, 1.0, 0),
            ("<=", "≤", Style::Upright, 1.0, 0),
            (">=", "≥", Style::Upright, 1.0, 0),
            ("!=", "≠", Style::Upright, 1.0, 0),
        ];
        for (input, expected, style, scale, shift) in cases {
            let nodes = parse(input).unwrap();
            validate_glyphs(&nodes).unwrap();
            let label = crate::layout_label_from_nodes(&nodes);
            let span = label
                .spans()
                .into_iter()
                .find(|span| span.text == expected)
                .unwrap_or_else(|| panic!("missing {expected} for {input}"));
            assert_eq!(span.style, style, "{input}");
            assert!((span.scale - scale).abs() < 0.001, "{input}");
            if shift == 0 {
                assert!(span.baseline_shift_em.abs() < 0.001, "{input}");
            } else {
                assert_eq!(span.baseline_shift_em.signum() as i32, shift, "{input}");
            }
        }
    }

    #[test]
    fn fixed_document_labels_have_simple_valid_input() {
        let document = crate::FigureDocument::fixed();
        for label in &document.project().semantic_registry {
            let markup =
                format(&label.nodes).unwrap_or_else(|| panic!("cannot format {}", label.id));
            assert!(!markup.contains("\\unit"), "{}", label.id);
            let parsed = parse(&markup).unwrap();
            assert_eq!(
                display_text(&parsed),
                display_text(&label.nodes),
                "{}",
                label.id
            );
            validate_glyphs(&parsed).unwrap();
        }
    }

    #[test]
    fn typed_label_uses_existing_pdf_pipeline() {
        let mut document = crate::FigureDocument::fixed();
        let nodes = parse(r"$\sigma_0 H_z \sum <= 3$ \*").unwrap();
        assert!(display_text(&nodes).ends_with(" *"));
        validate_glyphs(&nodes).unwrap();
        document
            .set_axis_label(crate::AxisDimension::X, nodes)
            .unwrap();
        let pdf = crate::figure_pdf(&document).unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
        assert!(pdf.windows(9).any(|bytes| bytes == b"/FontFile"));
    }

    #[test]
    #[ignore = "run with INSTPLOT_LABEL_QA_PDF to write a temporary visual-check PDF"]
    fn write_manual_label_visual_qa_pdf() {
        let path = std::env::var_os("INSTPLOT_LABEL_QA_PDF").expect("set output path");
        let mut document = crate::FigureDocument::fixed();
        document
            .set_axis_label(
                crate::AxisDimension::X,
                parse(r"$\sigma_0 H_{\mathrm{DL}} \sum <= 3$").unwrap(),
            )
            .unwrap();
        crate::save_figure_pdf(&document, std::path::Path::new(&path)).unwrap();
    }
}
