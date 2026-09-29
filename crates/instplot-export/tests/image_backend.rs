use std::collections::BTreeMap;

use instplot_export::{
    Background, RasterAsset, ResolvedBounds, rasterize_direct, resolve,
    resolve_tight_with_resources, resolve_with_resources, to_pdf, to_svg,
};
use instplot_render::{DisplayItem, DisplayList, Image, NodeId, Pt, compile, fixed_figure};

#[test]
fn rgba_image_is_embedded_in_pdf_svg_and_direct_raster() {
    let mut display_list = compile(&fixed_figure()).unwrap();
    display_list.items.push(DisplayItem::Image(Image {
        source: NodeId(20),
        resource_id: "rgba-checker".into(),
        x: Pt::new(50.0).unwrap(),
        y: Pt::new(50.0).unwrap(),
        width: Pt::new(12.0).unwrap(),
        height: Pt::new(12.0).unwrap(),
    }));
    let mut resources = BTreeMap::new();
    resources.insert(
        "rgba-checker".into(),
        RasterAsset {
            width: 2,
            height: 2,
            rgba: vec![
                255, 0, 0, 128, 0, 255, 0, 255, 0, 0, 255, 64, 255, 255, 255, 0,
            ],
        },
    );
    let resolved = resolve_with_resources(&display_list, resources);

    let svg = to_svg(&resolved);
    assert!(svg.contains("data-node=\"20\""));
    assert!(svg.contains("href=\"data:image/png;base64,"));

    let pdf = to_pdf(&resolved).unwrap();
    let without_image = to_pdf(&resolve(&compile(&fixed_figure()).unwrap())).unwrap();
    assert!(pdf.len() > without_image.len() + 100);

    let raster = rasterize_direct(&resolved, 300, Background::Transparent).unwrap();
    let sample_x = (52.0_f32 * 300.0 / 72.0) as u32;
    let sample_y = (52.0_f32 * 300.0 / 72.0) as u32;
    let offset = ((sample_y * raster.width + sample_x) * 4) as usize;
    assert!(raster.rgba[offset] > 200);
    assert!(raster.rgba[offset + 3] > 0 && raster.rgba[offset + 3] < 255);
}

#[test]
fn tight_export_retains_visible_images_and_ignores_fully_transparent_ones() {
    let pt = |value| Pt::new(value).unwrap();
    let display_list = DisplayList {
        width: pt(100.0),
        height: pt(100.0),
        items: vec![DisplayItem::Image(Image {
            source: NodeId(21),
            resource_id: "outside-image".into(),
            x: pt(100.0),
            y: pt(20.0),
            width: pt(20.0),
            height: pt(10.0),
        })],
    };
    let plot_bounds = ResolvedBounds {
        min_x: 10.0,
        min_y: 10.0,
        max_x: 90.0,
        max_y: 90.0,
    };

    let mut visible_resources = BTreeMap::new();
    visible_resources.insert(
        "outside-image".into(),
        RasterAsset {
            width: 1,
            height: 1,
            rgba: vec![255, 0, 0, 255],
        },
    );
    let visible = resolve_tight_with_resources(&display_list, visible_resources, plot_bounds, 3.0);
    assert_eq!(visible.geometry.export_bounds.max_x, 123.0);
    assert!(visible.resources.contains_key("outside-image"));
    assert!(to_svg(&visible).contains("data-node=\"21\""));

    let mut transparent_resources = BTreeMap::new();
    transparent_resources.insert(
        "outside-image".into(),
        RasterAsset {
            width: 1,
            height: 1,
            rgba: vec![255, 0, 0, 0],
        },
    );
    let transparent =
        resolve_tight_with_resources(&display_list, transparent_resources, plot_bounds, 3.0);
    assert_eq!(transparent.geometry.export_bounds.max_x, 93.0);
    assert!(transparent.resources.contains_key("outside-image"));
}
