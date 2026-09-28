#![cfg(feature = "comparison-raster")]

use std::io::Cursor;

use instplot_export::{
    Background, checked_raster_dimensions, encode_png, rasterize_direct, rasterize_via_svg, resolve,
};
use instplot_render::{compile, fixed_figure};

#[test]
fn raster_dimensions_background_alpha_and_png_metadata_are_correct() {
    let resolved = resolve(&compile(&fixed_figure()).unwrap());
    for dpi in [300, 600, 1200] {
        let expected =
            checked_raster_dimensions(resolved.width.into(), resolved.height.into(), dpi).unwrap();
        let transparent = rasterize_via_svg(&resolved, dpi, Background::Transparent).unwrap();
        assert_eq!((transparent.width, transparent.height), expected);
        assert!(
            transparent
                .rgba
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[3] == 0)
        );
        assert!(
            transparent
                .rgba
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[3] == 255)
        );

        let white = rasterize_via_svg(&resolved, dpi, Background::White).unwrap();
        assert!(
            white
                .rgba
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| pixel[3] == 255)
        );

        let png = encode_png(&white).unwrap();
        let decoder = png::Decoder::new(Cursor::new(png));
        let reader = decoder.read_info().unwrap();
        assert_eq!((reader.info().width, reader.info().height), expected);
        let dimensions = reader.info().pixel_dims.unwrap();
        assert_eq!(dimensions.unit, png::Unit::Meter);
        assert_eq!(dimensions.xppu, (dpi as f64 / 0.0254).round() as u32);
        assert!(reader.info().uncompressed_latin1_text.iter().any(|chunk| {
            chunk.keyword == "Software" && chunk.text.contains("InstPlot Studio")
        }));
    }
}

#[test]
fn direct_and_svg_routes_share_dimensions_and_are_visually_close() {
    let resolved = resolve(&compile(&fixed_figure()).unwrap());
    let direct = rasterize_direct(&resolved, 300, Background::White).unwrap();
    let via_svg = rasterize_via_svg(&resolved, 300, Background::White).unwrap();
    assert_eq!(
        (direct.width, direct.height),
        (via_svg.width, via_svg.height)
    );
    let border_offset = ((67 * direct.width + 158) * 4) as usize;
    assert!(
        direct.rgba[border_offset] < 220,
        "direct border pixel: {:?}",
        &direct.rgba[border_offset..border_offset + 4]
    );

    let differing_channels = direct
        .rgba
        .iter()
        .zip(&via_svg.rgba)
        .filter(|(left, right)| left.abs_diff(**right) > 16)
        .count();
    let ratio = differing_channels as f64 / direct.rgba.len() as f64;
    assert!(ratio < 0.08, "different channel ratio: {ratio:.4}");
}
