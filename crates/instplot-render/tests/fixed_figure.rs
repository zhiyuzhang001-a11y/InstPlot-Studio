use instplot_render::{DisplayItem, NodeId, compile, fixed_figure, to_svg};
use instplot_text::Style;

#[test]
fn display_list_snapshot_is_deterministic() {
    let first = compile(&fixed_figure()).unwrap().debug_snapshot();
    let second = compile(&fixed_figure()).unwrap().debug_snapshot();
    assert_eq!(first, second);
    assert_eq!(first, include_str!("../snapshots/fixed_figure.txt"));

    let display = compile(&fixed_figure()).unwrap();
    let x_label = display
        .items
        .iter()
        .find_map(|item| match item {
            DisplayItem::GlyphRun(run) if run.source == NodeId(3) => Some(run),
            _ => None,
        })
        .unwrap();
    let spans = x_label.label.spans();
    assert_eq!(spans[0].style, Style::Italic);
    assert!(spans.iter().any(|span| span.baseline_shift_em > 0.0));
    assert!(
        !x_label
            .label
            .normalized_text()
            .chars()
            .any(|character| matches!(character, '₀' | '⁻' | '²'))
    );
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
