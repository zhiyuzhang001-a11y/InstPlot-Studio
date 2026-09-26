#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BracketEdit {
    pub text: String,
    pub cursor: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BracketMode {
    Literal,
    FigureText,
}

pub fn pair_bracket_edit(
    before: &str,
    after: &str,
    cursor: usize,
    pasted: bool,
    mode: BracketMode,
) -> Option<BracketEdit> {
    if pasted || after.chars().count() != before.chars().count() + 1 || cursor == 0 {
        return None;
    }
    let inserted = after.chars().nth(cursor - 1)?;
    let start = char_to_byte(after, cursor - 1)?;
    let end = char_to_byte(after, cursor)?;
    let mut without_inserted = after.to_owned();
    without_inserted.replace_range(start..end, "");
    if without_inserted != before {
        return None;
    }

    let normalized = if mode == BracketMode::FigureText {
        halfwidth_bracket(inserted).unwrap_or(inserted)
    } else {
        inserted
    };
    // Figure text is intentionally Latin/scientific rather than CJK. Only
    // pair the bracket forms covered by the bundled publication fonts; leave
    // unsupported CJK punctuation literal instead of auto-generating a second
    // missing glyph.
    if mode == BracketMode::FigureText && !matches!(normalized, '(' | ')' | '[' | ']' | '{' | '}') {
        return None;
    }
    let next = after.chars().nth(cursor);
    if is_closing_bracket(normalized) && (next == Some(normalized) || next == Some(inserted)) {
        return Some(BracketEdit {
            text: without_inserted,
            cursor,
        });
    }
    let mut normalized_after = after.to_owned();
    if normalized != inserted {
        normalized_after.replace_range(start..end, &normalized.to_string());
    }
    let Some(closing) = matching_closer(normalized) else {
        return (normalized != inserted).then_some(BracketEdit {
            text: normalized_after,
            cursor,
        });
    };
    if next == Some(closing) || is_escaped(after, start) {
        return (normalized != inserted).then_some(BracketEdit {
            text: normalized_after,
            cursor,
        });
    }
    let normalized_end = start + normalized.len_utf8();
    normalized_after.insert(normalized_end, closing);
    Some(BracketEdit {
        text: normalized_after,
        cursor,
    })
}

fn halfwidth_bracket(character: char) -> Option<char> {
    Some(match character {
        '（' => '(',
        '）' => ')',
        '［' => '[',
        '］' => ']',
        '｛' => '{',
        '｝' => '}',
        _ => return None,
    })
}

fn char_to_byte(text: &str, index: usize) -> Option<usize> {
    if index == text.chars().count() {
        Some(text.len())
    } else {
        text.char_indices().nth(index).map(|(byte, _)| byte)
    }
}

fn is_escaped(text: &str, opener_byte: usize) -> bool {
    text[..opener_byte]
        .chars()
        .rev()
        .take_while(|character| *character == '\\')
        .count()
        % 2
        == 1
}

fn matching_closer(opening: char) -> Option<char> {
    Some(match opening {
        '(' => ')',
        '[' => ']',
        '{' => '}',
        '（' => '）',
        '［' => '］',
        '｛' => '｝',
        '【' => '】',
        '〔' => '〕',
        '〖' => '〗',
        '「' => '」',
        '『' => '』',
        '《' => '》',
        '〈' => '〉',
        '⟨' => '⟩',
        '⌈' => '⌉',
        '⌊' => '⌋',
        // ASCII < is intentionally excluded: it starts <=, <, and other math input.
        _ => return None,
    })
}

fn is_closing_bracket(character: char) -> bool {
    matches!(
        character,
        ')' | ']'
            | '}'
            | '）'
            | '］'
            | '｝'
            | '】'
            | '〕'
            | '〗'
            | '」'
            | '』'
            | '》'
            | '〉'
            | '⟩'
            | '⌉'
            | '⌋'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairs_ascii_cjk_and_math_brackets_at_the_caret() {
        for (opening, closing) in [
            ('(', ')'),
            ('[', ']'),
            ('{', '}'),
            ('（', '）'),
            ('【', '】'),
            ('「', '」'),
            ('《', '》'),
            ('⟨', '⟩'),
        ] {
            let edit = pair_bracket_edit(
                "ab",
                &format!("a{opening}b"),
                2,
                false,
                BracketMode::Literal,
            )
            .unwrap();
            assert_eq!(edit.text, format!("a{opening}{closing}b"));
            assert_eq!(edit.cursor, 2);
        }
    }

    #[test]
    fn closing_bracket_skips_existing_match() {
        let edit = pair_bracket_edit("H_{}", "H_{}}", 4, false, BracketMode::FigureText).unwrap();
        assert_eq!(edit.text, "H_{}");
        assert_eq!(edit.cursor, 4);
    }

    #[test]
    fn pastes_comparisons_and_escaped_braces_stay_literal() {
        assert!(pair_bracket_edit("", "(", 1, true, BracketMode::FigureText).is_none());
        assert!(pair_bracket_edit("", "<", 1, false, BracketMode::FigureText).is_none());
        assert!(pair_bracket_edit("\\", "\\{", 2, false, BracketMode::FigureText).is_none());
        assert!(pair_bracket_edit("a", "a()", 2, false, BracketMode::FigureText).is_none());
    }

    #[test]
    fn preserves_unicode_cursor_positions() {
        let edit =
            pair_bracket_edit("磁场 K", "磁场（ K", 3, false, BracketMode::FigureText).unwrap();
        assert_eq!(edit.text, "磁场() K");
        assert_eq!(edit.cursor, 3);
    }

    #[test]
    fn figure_text_normalizes_fullwidth_closing_brackets() {
        let edit = pair_bracket_edit("H_{}", "H_{）}", 4, false, BracketMode::FigureText).unwrap();
        assert_eq!(edit.text, "H_{)}");
        let edit = pair_bracket_edit("()", "(）)", 2, false, BracketMode::FigureText).unwrap();
        assert_eq!(edit.text, "()");
        assert_eq!(edit.cursor, 2);
    }
}
