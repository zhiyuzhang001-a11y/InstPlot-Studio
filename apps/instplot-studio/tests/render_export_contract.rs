use instplot_export::checked_raster_dimensions;
use instplot_studio::{
    FigureDocument, figure_pdf, figure_png_with_background, figure_svg, resolve_document,
    resolve_document_for_export, resolved_figure_pdf, resolved_figure_png_with_background,
    resolved_figure_svg,
};

#[test]
fn preview_keeps_its_canvas_while_exports_share_tight_geometry() {
    let document = FigureDocument::fixed();
    let preview = resolve_document(&document).unwrap();
    let export = resolve_document_for_export(&document).unwrap();

    assert_eq!(preview.display.geometry.export_translation, (0.0, 0.0));
    assert!(export.display.width <= preview.display.width);
    assert!(export.display.height <= preview.display.height);

    let pdf = resolved_figure_pdf(&export).unwrap();
    let png = resolved_figure_png_with_background(&export, 300, true).unwrap();
    let svg_bytes = resolved_figure_svg(&export);
    let svg = String::from_utf8(svg_bytes.clone()).unwrap();

    assert!(pdf.starts_with(b"%PDF-"));
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    assert!(svg.contains(&format!(
        "viewBox=\"0 0 {:.5} {:.5}\"",
        export.display.width, export.display.height
    )));

    let expected = checked_raster_dimensions(
        export.display.width.into(),
        export.display.height.into(),
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
    assert_eq!(svg_bytes, figure_svg(&document).unwrap());
}
