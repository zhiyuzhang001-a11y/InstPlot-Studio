use eframe::egui;
pub use instplot_studio::BracketMode;
use instplot_studio::pair_bracket_edit;

/// Apply bracket pairing after egui has processed one keystroke.
/// Paste and multi-character edits stay literal, so external text is never rewritten.
pub fn auto_pair_brackets(
    ui: &egui::Ui,
    before: &str,
    text: &mut String,
    output: &mut egui::text_edit::TextEditOutput,
    mode: BracketMode,
) {
    if !output.response.changed() || !output.response.has_focus() {
        return;
    }
    let pasted = ui.input(|input| {
        input
            .events
            .iter()
            .any(|event| matches!(event, egui::Event::Paste(_)))
    });
    let Some(range) = output.cursor_range else {
        return;
    };
    let Some(cursor) = range.single() else {
        return;
    };
    let Some(edit) = pair_bracket_edit(before, text, cursor.index.into(), pasted, mode) else {
        return;
    };
    *text = edit.text;
    let range = egui::text::CCursorRange::one(egui::text::CCursor::new(edit.cursor));
    output.cursor_range = Some(range);
    output.state.cursor.set_char_range(Some(range));
    output.state.clone().store(ui.ctx(), output.response.id);
    ui.ctx().request_repaint();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn type_in_headless_field(
        initial: &str,
        typed: &str,
        mode: BracketMode,
        caret: Option<usize>,
        paste: bool,
    ) -> (String, usize) {
        let context = egui::Context::default();
        let mut text = initial.to_owned();
        let mut field_id = None;
        let mut output = context.run_ui(egui::RawInput::default(), |ui| {
            let mut field = egui::TextEdit::singleline(&mut text).show(ui);
            if let Some(index) = caret {
                let range = egui::text::CCursorRange::one(egui::text::CCursor::new(index));
                field.state.cursor.set_char_range(Some(range));
                field.state.store(ui.ctx(), field.response.id);
            }
            field_id = Some(field.response.id);
        });
        output.textures_delta.clear();
        let field_id = field_id.unwrap();
        context.memory_mut(|memory| memory.request_focus(field_id));
        let mut input = egui::RawInput::default();
        input.events.push(if paste {
            egui::Event::Paste(typed.to_owned())
        } else {
            egui::Event::Text(typed.to_owned())
        });
        let before = text.clone();
        let mut cursor = None;
        let mut output = context.run_ui(input, |ui| {
            let mut field = egui::TextEdit::singleline(&mut text).show(ui);
            auto_pair_brackets(ui, &before, &mut text, &mut field, mode);
            cursor = field.cursor_range.and_then(|range| range.single());
        });
        output.textures_delta.clear();
        (text, cursor.unwrap().index.into())
    }

    #[test]
    fn headless_text_field_pairs_ascii_and_normalizes_chinese_ime_brackets() {
        assert_eq!(
            type_in_headless_field("", "(", BracketMode::FigureText, None, false),
            ("()".to_owned(), 1)
        );
        assert_eq!(
            type_in_headless_field("", "（", BracketMode::FigureText, None, false),
            ("()".to_owned(), 1)
        );
        assert_eq!(
            type_in_headless_field("", "<=", BracketMode::FigureText, None, false),
            ("<=".to_owned(), 2)
        );
        assert_eq!(
            type_in_headless_field("()", ")", BracketMode::FigureText, Some(1), false),
            ("()".to_owned(), 2)
        );
        assert_eq!(
            type_in_headless_field("", "(", BracketMode::FigureText, None, true),
            ("(".to_owned(), 1)
        );
    }

    #[test]
    fn figure_text_does_not_auto_pair_unsupported_cjk_punctuation() {
        assert_eq!(
            type_in_headless_field("", "《", BracketMode::FigureText, None, false),
            ("《".to_owned(), 1)
        );
    }

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
