use studio_render_spike::{compile, fixed_figure, to_svg};

#[test]
fn display_list_snapshot_is_deterministic() {
    let first = compile(&fixed_figure()).unwrap().debug_snapshot();
    let second = compile(&fixed_figure()).unwrap().debug_snapshot();
    assert_eq!(first, second);
    assert_eq!(first, include_str!("../snapshots/fixed_figure.txt"));
}

#[test]
fn minimal_svg_backend_maps_paths_without_layout_decisions() {
    let display_list = compile(&fixed_figure()).unwrap();
    assert!(display_list.validation_errors().is_empty());
    let output = to_svg(&display_list);
    assert!(output.warnings.is_empty(), "{:?}", output.warnings);
    assert!(output.svg.contains("<path data-node=\"11\""));
    assert!(output.svg.contains("<text data-node=\"3\""));
    assert!(output.svg.contains("clip-path=\"url(#clip-0)\""));
    assert!(output.svg.contains("width=\"252.28346pt\""));
}
