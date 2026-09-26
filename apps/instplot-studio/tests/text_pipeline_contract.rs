use instplot_studio::{AxisDimension, BracketMode, FigureDocument, figure_pdf, label_input};

#[test]
fn one_label_semantic_tree_drives_preview_and_pdf_export() {
    let source = r"$v_{sk}$ <= 300 K";
    let nodes = label_input::parse(source).unwrap();
    label_input::validate_glyphs(&nodes).unwrap();
    assert_eq!(
        label_input::format(&nodes).as_deref(),
        Some(r"$v_{sk}$ ≤ 300 K")
    );

    let mut document = FigureDocument::fixed();
    document
        .set_axis_label(AxisDimension::X, nodes.clone())
        .unwrap();
    let resolved = instplot_studio::resolve_document(&document).unwrap();
    assert!(!resolved.display.items.is_empty());
    let pdf = figure_pdf(&document).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
    assert_eq!(document.axis_label(AxisDimension::X), nodes);
}

#[test]
fn label_rules_distinguish_math_variables_units_and_literal_brackets() {
    let math = label_input::parse(r"$v_{sk}$").unwrap();
    let descriptive = label_input::parse(r"$v$_{sk}").unwrap();
    assert_ne!(math, descriptive);

    let comparison = label_input::parse("T <= 300 K").unwrap();
    assert_eq!(label_input::display_text(&comparison), "T ≤ 300 K");

    let edit =
        instplot_studio::pair_bracket_edit("H_", "H_{", 3, false, BracketMode::FigureText).unwrap();
    assert_eq!(edit.text, "H_{}");
    assert_eq!(edit.cursor, 3);
}
