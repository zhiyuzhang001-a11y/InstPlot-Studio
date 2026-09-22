use export_backend_spike::{FontOrigin, resolve};
use layout_engine_spike::{
    LayoutWarning, Locator, Scale, SelectableRole, TextMeasurer, TextSize, TickDirection, layout,
    layout_with_measurer, marker_gallery_fixture, publication_fixture,
};
use studio_render_spike::{DisplayItem, NodeId, PathVerb};
use text_shaping_spike::Label;

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
    let x_ticks = axis_segments(&first.display_list.items, chart.x.id);
    let y_ticks = axis_segments(&first.display_list.items, chart.y.id);
    let x_tick_count = first.x_axis.major.len() + first.x_axis.minor.len();
    let y_tick_count = first.y_axis.major.len() + first.y_axis.minor.len();
    assert_eq!(x_ticks.len(), x_tick_count * 2);
    assert_eq!(y_ticks.len(), y_tick_count * 2);
    assert_eq!(
        x_ticks
            .iter()
            .filter(|((_, start_y), (_, end_y))| {
                *start_y == first.axes.bottom() && end_y < start_y
            })
            .count(),
        x_tick_count
    );
    assert_eq!(
        x_ticks
            .iter()
            .filter(|((_, start_y), (_, end_y))| {
                *start_y == first.axes.y && end_y > start_y
            })
            .count(),
        x_tick_count
    );
    assert_eq!(
        y_ticks
            .iter()
            .filter(|((start_x, _), (end_x, _))| {
                *start_x == first.axes.x && end_x > start_x
            })
            .count(),
        y_tick_count
    );
    assert_eq!(
        y_ticks
            .iter()
            .filter(|((start_x, _), (end_x, _))| {
                *start_x == first.axes.right() && end_x < start_x
            })
            .count(),
        y_tick_count
    );
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
    assert!(page.contains(first.x_label_bounds));
    assert!(page.contains(first.y_label_bounds));
    let x_tick_bottom = first
        .x_axis
        .major
        .iter()
        .map(|tick| tick.label_bounds.bottom())
        .fold(first.axes.bottom(), f64::max);
    let y_tick_left = first
        .y_axis
        .major
        .iter()
        .map(|tick| tick.label_bounds.x)
        .fold(first.axes.x, f64::min);
    assert!((first.x_label_bounds.y - x_tick_bottom - 4.0).abs() < 1e-9);
    assert!((y_tick_left - first.y_label_bounds.right() - 4.0).abs() < 1e-9);
    assert!((chart.height_pt - first.x_label_bounds.bottom() - 6.0).abs() < 1e-9);
    assert!((first.y_label_bounds.x - 6.0).abs() < 1e-9);

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
fn large_greek_and_scripted_labels_keep_balanced_clearance() {
    let mut chart = publication_fixture();
    chart.width_pt = 85.0 / 25.4 * 72.0;
    chart.x.label = Label::Group(vec![
        Label::GreekVariable('Δ'),
        Label::VariableSubscript(Box::new(Label::Text("maximum".into()))),
        Label::Text(" (".into()),
        Label::Unit("rad".into()),
        Label::Text(")".into()),
    ]);
    chart.y.label = Label::Group(vec![
        Label::GreekVariable('Ω'),
        Label::Superscript(Box::new(Label::Number("2".into()))),
        Label::VariableSubscript(Box::new(Label::GreekVariable('μ'))),
        Label::Text(" (".into()),
        Label::Unit("A".into()),
        Label::UnitSeparator,
        Label::Unit("m".into()),
        Label::Superscript(Box::new(Label::Number("−2".into()))),
        Label::Text(")".into()),
    ]);

    let result = layout(&chart).unwrap();
    let x_tick_bottom = result
        .x_axis
        .major
        .iter()
        .map(|tick| tick.label_bounds.bottom())
        .fold(result.axes.bottom(), f64::max);
    let y_tick_left = result
        .y_axis
        .major
        .iter()
        .map(|tick| tick.label_bounds.x)
        .fold(result.axes.x, f64::min);
    assert!((result.x_label_bounds.y - x_tick_bottom - 4.0).abs() < 1e-9);
    assert!((y_tick_left - result.y_label_bounds.right() - 4.0).abs() < 1e-9);
    assert!((chart.height_pt - result.x_label_bounds.bottom() - 6.0).abs() < 1e-9);
    assert!((result.y_label_bounds.x - 6.0).abs() < 1e-9);
    assert!(!result.warnings.iter().any(|warning| matches!(
        warning,
        LayoutWarning::TextOutsideFigure { .. }
    )));
}

fn axis_segments(items: &[DisplayItem], source: NodeId) -> Vec<((f64, f64), (f64, f64))> {
    items
        .iter()
        .filter_map(|item| match item {
            DisplayItem::Path { source: node, path, .. }
                if *node == source
                    && matches!(path.verbs.as_slice(), [PathVerb::MoveTo(..), PathVerb::LineTo(..)]) =>
            {
                let [PathVerb::MoveTo(x1, y1), PathVerb::LineTo(x2, y2)] = path.verbs.as_slice()
                else {
                    unreachable!()
                };
                Some(((x1.get(), y1.get()), (x2.get(), y2.get())))
            }
            _ => None,
        })
        .collect()
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
fn axis_appearance_controls_spines_ticks_labels_and_direction() {
    let mut chart = publication_fixture();
    chart.x.appearance.far_spine = false;
    chart.x.appearance.far_ticks = false;
    chart.x.appearance.minor_ticks = false;
    chart.x.appearance.tick_direction = TickDirection::Out;
    chart.x.appearance.far_tick_labels = true;
    chart.y.appearance.near_spine = false;
    chart.y.appearance.near_ticks = false;
    let result = layout(&chart).unwrap();

    assert!(!result.x_axis.minor.is_empty());
    let x_ticks = axis_segments(&result.display_list.items, chart.x.id);
    assert_eq!(x_ticks.len(), result.x_axis.major.len());
    assert!(x_ticks.iter().all(|((_, start_y), (_, end_y))| {
        *start_y == result.axes.bottom() && end_y > start_y
    }));
    let axis_spines = axis_segments(&result.display_list.items, chart.id);
    assert!(!axis_spines.iter().any(|((_, y1), (_, y2))| {
        *y1 == result.axes.y && *y2 == result.axes.y
    }));
    assert!(!axis_spines.iter().any(|((x1, _), (x2, _))| {
        *x1 == result.axes.x && *x2 == result.axes.x
    }));
    assert!(
        result
            .hit_map
            .items
            .iter()
            .filter(|item| item.node == chart.x.id && item.role == SelectableRole::Tick)
            .count()
            > result.x_axis.major.len()
    );
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
                ascent: size_pt * if self.wide { 1.0 } else { 0.6 },
                descent: size_pt * if self.wide { 0.3 } else { 0.2 },
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
