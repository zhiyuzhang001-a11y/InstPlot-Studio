use export_backend_spike::{FontOrigin, resolve};
use layout_engine_spike::{
    LayoutWarning, Locator, Scale, SelectableRole, TextMeasurer, TextSize, layout,
    layout_with_measurer, marker_gallery_fixture, publication_fixture,
};
use studio_render_spike::DisplayItem;

#[test]
fn publication_fixture_is_deterministic_and_unclipped() {
    let chart = publication_fixture();
    let first = layout(&chart).unwrap();
    let second = layout(&chart).unwrap();
    assert_eq!(first.snapshot(), second.snapshot());
    assert_eq!(
        first.snapshot(),
        include_str!("../snapshots/publication_layout.txt")
    );
    assert_eq!(first.display_list.width.get(), chart.width_pt);
    assert_eq!(first.display_list.height.get(), chart.height_pt);
    let clip_rectangles: Vec<_> = first
        .display_list
        .items
        .iter()
        .filter_map(|item| match item {
            DisplayItem::ClipPush {
                x,
                y,
                width,
                height,
                ..
            } => Some((x.get(), y.get(), width.get(), height.get())),
            _ => None,
        })
        .collect();
    assert_eq!(
        clip_rectangles,
        vec![(
            first.axes.x,
            first.axes.y,
            first.axes.width,
            first.axes.height
        )]
    );
    assert_eq!(
        first
            .display_list
            .items
            .iter()
            .filter(|item| matches!(item, DisplayItem::ClipPop { .. }))
            .count(),
        1
    );
    assert!(first.display_list.validation_errors().is_empty());
    assert!(first.iterations <= 4);
    assert!(!first.warnings.iter().any(|warning| matches!(
        warning,
        LayoutWarning::NonConvergent { .. } | LayoutWarning::TextOutsideFigure { .. }
    )));
    for diagnostic in resolve(&first.display_list).font_diagnostics() {
        assert_eq!(
            diagnostic.origin,
            FontOrigin::BundledPrimary,
            "{diagnostic:?}"
        );
        assert!(!diagnostic.missing_glyph, "{diagnostic:?}");
    }

    let page = layout_engine_spike::Bounds {
        x: 0.0,
        y: 0.0,
        width: chart.width_pt,
        height: chart.height_pt,
    };
    for tick in first.x_axis.major.iter().chain(first.y_axis.major.iter()) {
        assert!(page.contains(tick.label_bounds));
    }

    for pair in first.x_axis.major.windows(2) {
        assert_eq!(
            pair[0].label_bounds.intersection_area(pair[1].label_bounds),
            0.0
        );
    }
    let legend = first.legend.unwrap();
    assert!(legend.right() <= chart.width_pt && legend.bottom() <= chart.height_pt);
    assert!(
        first
            .warnings
            .contains(&LayoutWarning::LegendMovedOutside { node: chart.id })
    );
    for item in &first.hit_map.items {
        if matches!(
            item.role,
            SelectableRole::DataPoint | SelectableRole::ErrorBar | SelectableRole::Annotation
        ) {
            assert_eq!(legend.intersection_area(item.bounds), 0.0);
        }
    }
    for role in [
        SelectableRole::Axes,
        SelectableRole::Tick,
        SelectableRole::Series,
        SelectableRole::DataPoint,
        SelectableRole::ErrorBar,
        SelectableRole::Annotation,
        SelectableRole::Legend,
    ] {
        assert!(first.hit_map.items.iter().any(|item| item.role == role));
    }
    let legend_hit = first
        .hit_map
        .hit_test(
            legend.x + legend.width / 2.0,
            legend.y + legend.height / 2.0,
            2.0,
        )
        .unwrap();
    assert_eq!(legend_hit.role, SelectableRole::Legend);
    let series = first
        .hit_map
        .items
        .iter()
        .find(|item| item.role == SelectableRole::Series)
        .unwrap();
    let point = series.path_proximity[2];
    assert_eq!(
        first.hit_map.hit_test(point.0, point.1, 2.0).unwrap().node,
        series.node
    );
}

#[test]
fn all_marker_and_dash_contracts_reach_the_display_list() {
    let chart = marker_gallery_fixture();
    let result = layout(&chart).unwrap();
    let point_hits = result
        .hit_map
        .items
        .iter()
        .filter(|item| item.role == SelectableRole::DataPoint)
        .count();
    assert_eq!(point_hits, 14);
    assert_eq!(
        result
            .hit_map
            .items
            .iter()
            .filter(|item| item.role == SelectableRole::ErrorBar)
            .count(),
        2
    );

    let dash_patterns: Vec<Vec<f64>> = result
        .display_list
        .items
        .iter()
        .filter_map(|item| match item {
            DisplayItem::Path {
                stroke: Some(stroke),
                ..
            } => Some(stroke.dash.iter().map(|value| value.get()).collect()),
            _ => None,
        })
        .collect();
    for expected in [
        vec![],
        vec![4.0, 2.4],
        vec![0.8, 1.8],
        vec![4.0, 2.0, 0.8, 2.0],
    ] {
        assert!(dash_patterns.contains(&expected));
    }
}

#[test]
fn log_axes_layout_uses_decades_and_minor_ticks() {
    let mut chart = publication_fixture();
    chart.x.minimum = 1e-3;
    chart.x.maximum = 1e3;
    chart.x.scale = Scale::Log10;
    chart.x.locator = Locator::Auto {
        target_spacing_pt: 30.0,
    };
    chart.series.clear();
    chart.annotations.clear();
    let result = layout(&chart).unwrap();
    assert_eq!(
        result
            .x_axis
            .major
            .iter()
            .map(|tick| tick.value)
            .collect::<Vec<_>>(),
        vec![1e-3, 1e-2, 1e-1, 1.0, 10.0, 100.0, 1000.0]
    );
    assert!(!result.x_axis.minor.is_empty());
}

#[test]
fn layout_reports_non_convergence_from_unstable_metrics() {
    struct Unstable {
        wide: bool,
    }
    impl TextMeasurer for Unstable {
        fn measure(&mut self, text: &str, size_pt: f64) -> TextSize {
            self.wide = !self.wide;
            TextSize {
                width: text.chars().count() as f64 * size_pt * if self.wide { 1.2 } else { 0.3 },
                height: size_pt * if self.wide { 1.3 } else { 0.8 },
            }
        }
    }
    let result =
        layout_with_measurer(&publication_fixture(), &mut Unstable { wide: false }).unwrap();
    assert!(result.warnings.contains(&LayoutWarning::NonConvergent {
        node: publication_fixture().id,
        iterations: 4,
    }));
}
