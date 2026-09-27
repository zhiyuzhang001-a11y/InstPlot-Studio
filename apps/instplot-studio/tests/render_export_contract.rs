use instplot_export::checked_raster_dimensions;
use instplot_studio::{
    FigureDocument, figure_pdf, figure_png_with_background, resolve_document, resolved_figure_pdf,
    resolved_figure_png_with_background, resolved_figure_svg,
};

#[test]
fn preview_pdf_png_and_svg_share_one_resolved_scene_geometry() {
    let document = FigureDocument::fixed();
    let resolved = resolve_document(&document).unwrap();

    let pdf = resolved_figure_pdf(&resolved).unwrap();
    let png = resolved_figure_png_with_background(&resolved, 300, true).unwrap();
    let svg = String::from_utf8(resolved_figure_svg(&resolved)).unwrap();

    assert!(pdf.starts_with(b"%PDF-"));
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    assert!(svg.contains(&format!(
        "viewBox=\"0 0 {:.5} {:.5}\"",
        resolved.display.width, resolved.display.height
    )));

    let expected = checked_raster_dimensions(
        resolved.display.width.into(),
        resolved.display.height.into(),
        300,
    )
    .unwrap();
    assert_eq!(
        u32::from_be_bytes(png[16..20].try_into().unwrap()),
        expected.0
    );
    assert_eq!(
        u32::from_be_bytes(png[20..24].try_into().unwrap()),
        expected.1
    );

    assert_eq!(pdf, figure_pdf(&document).unwrap());
    assert_eq!(
        png,
        figure_png_with_background(&document, 300, true).unwrap()
    );
}
