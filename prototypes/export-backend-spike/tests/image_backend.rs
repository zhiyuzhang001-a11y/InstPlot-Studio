use std::collections::BTreeMap;

use export_backend_spike::{
    Background, RasterAsset, rasterize_direct, resolve, resolve_with_resources, to_pdf, to_svg,
};
use studio_render_spike::{DisplayItem, Image, NodeId, Pt, compile, fixed_figure};

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

    let raster = rasterize_direct(&resolved, 300, Background::Transparent);
    let sample_x = (52.0_f32 * 300.0 / 72.0) as u32;
    let sample_y = (52.0_f32 * 300.0 / 72.0) as u32;
    let offset = ((sample_y * raster.width + sample_x) * 4) as usize;
    assert!(raster.rgba[offset] > 200);
    assert!(raster.rgba[offset + 3] > 0 && raster.rgba[offset + 3] < 255);
}
