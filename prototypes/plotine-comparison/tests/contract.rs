use std::io::Cursor;

#[test]
fn vector_and_raster_outputs_expose_contract_gaps() {
    let figure = plotine_comparison::fixture(72.0);
    let svg = figure.render_svg().unwrap();
    let xml = roxmltree::Document::parse(&svg).unwrap();
    let root = xml.root_element();
    assert_eq!(root.attribute("width"), Some("252"));
    assert_eq!(root.attribute("height"), Some("184"));
    assert!(svg.contains("<text"));
    assert!(svg.contains("font-family=\"DejaVu Sans, Arial, sans-serif\""));
    assert!(svg.contains("font-style=\"italic\""));
    let clipped_math_label = xml.descendants().any(|node| {
        node.has_tag_name("text")
            && node
                .attribute("y")
                .and_then(|value| value.parse::<f64>().ok())
                .is_some_and(|y| y > 184.0)
    });
    assert!(clipped_math_label);

    let expected_width_pt = 89.0_f64 / 25.4 * 72.0;
    let expected_height_pt = 65.0_f64 / 25.4 * 72.0;
    assert!((expected_width_pt - 252.0).abs() > 0.01);
    assert!((expected_height_pt - 184.0).abs() > 0.01);

    let pdf = figure.render_pdf().unwrap();
    let document = lopdf::Document::load_mem(&pdf).unwrap();
    assert_eq!(document.get_pages().len(), 1);
    let extracted = pdf_extract::extract_text_from_mem(&pdf).unwrap_or_default();
    assert!(extracted.contains("E x p e r i m e n t   A"));
    assert!(!extracted.contains("Experiment A"));
    assert!(!extracted.contains('温'));

    let png = plotine_comparison::fixture(300.0).render_png().unwrap();
    let decoder = png::Decoder::new(Cursor::new(png));
    let reader = decoder.read_info().unwrap();
    assert_eq!((reader.info().width, reader.info().height), (1051, 768));
}
