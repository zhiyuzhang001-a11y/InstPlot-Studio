#![cfg(feature = "comparison-raster")]

use instplot_export::{resolve, to_svg};
use instplot_render::{compile, fixed_figure};

#[test]
fn svg_is_physical_self_contained_parseable_and_keeps_text() {
    let resolved = resolve(&compile(&fixed_figure()).unwrap());
    let svg = to_svg(&resolved);
    let xml = roxmltree::Document::parse(&svg).unwrap();
    let root = xml.root_element();
    assert_eq!(root.attribute("width"), Some("252.28346pt"));
    assert_eq!(root.attribute("height"), Some("184.25197pt"));
    assert_eq!(root.attribute("viewBox"), Some("0 0 252.28346 184.25197"));
    assert!(svg.contains("<path data-node=\"11\""));
    assert!(svg.contains("clip-path=\"url(#clip-0)\""));
    assert!(svg.contains("data-source=\"Experiment\""));
    assert!(svg.contains("data-glyph-ids="));
    assert!(svg.contains("T ≤ 300 K"));
    assert!(svg.contains("rotate(-90.000"));
    assert!(svg.contains("data:font/otf;base64,"));
    assert!(!svg.contains("file://"));
    assert!(!svg.contains("href=\"/"));

    let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default()).unwrap();
    assert!(!tree.root().children().is_empty());
    assert_eq!(tree.size().width(), 336.37796);
    assert_eq!(tree.size().height(), 245.6693);
}
