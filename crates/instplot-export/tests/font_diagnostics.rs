use instplot_export::{FontOrigin, resolve, to_pdf};
use instplot_render::{
    Color, DisplayItem, GlyphRun, NodeId, Pt, TextAnchor, compile, fixed_figure,
};
use instplot_text::Label;

#[test]
fn all_resolved_fonts_are_bundled_and_embeddable() {
    let resolved = resolve(&compile(&fixed_figure()).unwrap());
    let diagnostics = resolved.font_diagnostics();
    assert!(!diagnostics.is_empty());
    for diagnostic in diagnostics {
        assert!(
            matches!(
                diagnostic.origin,
                FontOrigin::BundledPrimary | FontOrigin::BundledSymbol
            ),
            "{diagnostic:?}"
        );
        assert!(
            diagnostic.postscript_name.starts_with("TeXGyreHeros-")
                || diagnostic.postscript_name == "STIXTwoMath-Regular"
        );
        assert!(!diagnostic.version.starts_with('<'));
        assert!(diagnostic.embedding_allowed, "{}", diagnostic.embedding);
        assert!(diagnostic.subsetting_allowed);
        assert!(!diagnostic.missing_glyph);
    }

    let pdf = to_pdf(&resolved).expect("the bundled font must be embeddable in PDF");
    assert!(pdf.starts_with(b"%PDF-"));
    assert!(
        resolved
            .font_diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.text == "≤"
                && diagnostic.origin == FontOrigin::BundledSymbol)
    );
}

#[test]
fn missing_glyph_is_a_structured_export_diagnostic() {
    let mut display = compile(&fixed_figure()).unwrap();
    display.items.push(DisplayItem::GlyphRun(GlyphRun {
        source: NodeId(99),
        label: Label::Text(char::from_u32(0x10_FFFF).unwrap().to_string()),
        x: Pt::new(10.0).unwrap(),
        y: Pt::new(10.0).unwrap(),
        size: Pt::new(9.0).unwrap(),
        color: Color(0, 0, 0, 255),
        rotation_degrees: 0.0,
        anchor: TextAnchor::Start,
    }));

    let diagnostics = resolve(&display).font_diagnostics();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.source == NodeId(99) && diagnostic.missing_glyph)
    );
}
