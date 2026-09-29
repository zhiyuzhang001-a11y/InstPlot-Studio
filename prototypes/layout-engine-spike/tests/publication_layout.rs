use instplot_export::{FontOrigin, resolve};
use layout_engine_spike::{
    AnnotationConnector, AnnotationPosition, ArrowHead, AxisPair, Bounds, DashStyle, DataPoint,
    Formatter, HitItem, HitMap, LayoutWarning, LegendGrid, LegendPosition, LineStyle, Locator,
    MarkerShape, MarkerStyle, MeasurementArrow, MeasurementConstraint, ReferenceLine,
    ReferenceOrientation, Scale, SelectableRole, TextMeasurer, TextSize, TickDirection, layout,
    layout_with_measurer, marker_gallery_fixture, publication_fixture,
};

use instplot_render::{Color, DisplayItem, NodeId, PathVerb};
use instplot_text::Label;

#[test]
fn manual_outside_legend_uses_final_canvas_origin_without_resizing_axes() {
    for (position, origin) in [
        (LegendPosition::Above, (24.0, 15.0)),
        (LegendPosition::Right, (250.0, 18.0)),
    ] {
        let mut chart = marker_gallery_fixture();
        let legend = chart.legend.as_mut().unwrap();
        legend.position = position;
        legend.manual_position = Some(origin);
        let placed = layout(&chart).unwrap();
        let bounds = placed.legend.unwrap();
        assert_eq!((bounds.x, bounds.y), origin);
        assert!(bounds.right() <= placed.display_list.width.get());
        assert!(bounds.bottom() <= placed.display_list.height.get());
        assert_eq!(
            bounds.intersection_area(placed.axes),
            0.0,
            "{position:?}: legend={bounds:?}, axes={:?}",
            placed.axes
        );
        let mut without_legend = chart.clone();
        without_legend.legend = None;
        let base = layout(&without_legend).unwrap();
        assert_eq!(placed.axes.width, base.axes.width);
        assert_eq!(placed.axes.height, base.axes.height);
        assert!(!placed.warnings.iter().any(|warning| matches!(
            warning,
            LayoutWarning::NonConvergent { .. } | LayoutWarning::InsufficientPlotArea { .. }
        )));
    }
}

#[test]
fn manual_outside_legend_adds_only_the_space_needed_for_its_contents() {
    for (position, origin) in [
        (LegendPosition::Above, (24.0, 0.0)),
        (LegendPosition::Right, (225.0, 18.0)),
    ] {
        let mut chart = marker_gallery_fixture();
        let legend = chart.legend.as_mut().unwrap();
        legend.position = position;
        legend.manual_position = Some(origin);
        let result = layout(&chart).unwrap();
        let bounds = result.legend.unwrap();
        match position {
            LegendPosition::Above => {
                assert!((result.axes.y - bounds.bottom() - 6.0).abs() < 0.05);
                assert_eq!(result.display_list.width.get(), chart.width_pt);
            }
            LegendPosition::Right => {
                assert!((result.display_list.width.get() - bounds.right() - 3.0).abs() < 0.05);
                assert_eq!(result.display_list.height.get(), chart.height_pt);
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn right_legend_stays_outside_after_main_canvas_resizes() {
    let mut chart = marker_gallery_fixture();
    let legend = chart.legend.as_mut().unwrap();
    legend.position = LegendPosition::Right;
    legend.manual_position = Some((250.0, 18.0));
    chart.width_pt += 80.0;
    let result = layout(&chart).unwrap();
    let bounds = result.legend.unwrap();
    assert!(bounds.x >= result.axes.right() + 6.0);
    assert_eq!(bounds.intersection_area(result.axes), 0.0);
    assert!(bounds.right() <= result.display_list.width.get());
}

#[test]
fn above_and_right_legends_fit_five_and_seven_entries_without_covering_axes() {
    for count in [5, 7] {
        for position in [LegendPosition::Above, LegendPosition::Right] {
            let mut chart = marker_gallery_fixture();
            chart.series.truncate(count);
            chart.legend.as_mut().unwrap().position = position;
            let result = layout(&chart).unwrap();
            let legend = result.legend.unwrap();
            let mut without_legend = chart.clone();
            without_legend.legend = None;
            let base = layout(&without_legend).unwrap();
            assert!(!result.warnings.iter().any(|warning| matches!(
                warning,
                LayoutWarning::NonConvergent { .. } | LayoutWarning::InsufficientPlotArea { .. }
            )));
            assert!(legend.x >= 0.0 && legend.y >= 0.0);
            assert!(legend.right() <= result.display_list.width.get());
            assert!(legend.bottom() <= result.display_list.height.get());
            assert_eq!(legend.intersection_area(result.axes), 0.0);
            assert_eq!(result.axes.width, base.axes.width);
            assert_eq!(result.axes.height, base.axes.height);
            if position == LegendPosition::Above {
                assert!(legend.bottom() < result.axes.y);
                assert!(legend.height < count as f64 * 12.0 + 8.0);
                assert_eq!(result.axes.x, base.axes.x);
                assert!(
                    ((result.axes.y - base.axes.y)
                        - (result.display_list.height.get() - chart.height_pt))
                        .abs()
                        < 1e-6
                );
                assert_eq!(result.display_list.width.get(), chart.width_pt);
            } else {
                assert!(legend.x > result.axes.right());
                assert_eq!(result.axes, base.axes);
                assert_eq!(result.display_list.height.get(), chart.height_pt);
                assert!(result.display_list.width.get() > chart.width_pt);
            }
        }
    }
}

#[test]
fn explicit_legend_rows_or_columns_control_the_grid_without_shrinking_axes() {
    for count in [5_usize, 7] {
        for position in [LegendPosition::Above, LegendPosition::Right] {
            for (grid, expected_rows, expected_columns) in [
                (LegendGrid::Rows(2), 2, count.div_ceil(2)),
                (LegendGrid::Columns(2), count.div_ceil(2), 2),
            ] {
                let mut chart = marker_gallery_fixture();
                chart.series.truncate(count);
                let legend = chart.legend.as_mut().unwrap();
                legend.position = position;
                legend.grid = grid;
                let result = layout(&chart).unwrap();
                let mut without_legend = chart.clone();
                without_legend.legend = None;
                let base = layout(&without_legend).unwrap();
                assert!((result.axes.width - base.axes.width).abs() < 1e-6);
                assert!((result.axes.height - base.axes.height).abs() < 1e-6);
                let bounds = result.legend.unwrap();
                assert_eq!(bounds.height, expected_rows as f64 * 12.0 + 6.0);
                let mut xs = Vec::new();
                let mut ys = Vec::new();
                for item in &result.display_list.items {
                    if let DisplayItem::GlyphRun(run) = item
                        && chart.series.iter().any(|series| series.id == run.source)
                    {
                        xs.push(run.x.get());
                        ys.push(run.y.get());
                    }
                }
                xs.sort_by(f64::total_cmp);
                xs.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
                ys.sort_by(f64::total_cmp);
                ys.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
                assert_eq!(xs.len(), expected_columns);
                assert_eq!(ys.len(), expected_rows);
                assert_eq!(bounds.intersection_area(result.axes), 0.0);
                assert!(bounds.right() <= result.display_list.width.get());
            }
        }
    }
}

#[test]
fn tall_right_legend_extends_the_output_below_without_moving_the_main_plot() {
    let mut chart = marker_gallery_fixture();
    while chart.series.len() < 20 {
        let mut extra = chart.series[chart.series.len() % 7].clone();
        extra.id = NodeId(1000 + chart.series.len() as u64);
        extra.label = format!("Series {}", chart.series.len() + 1);
        chart.series.push(extra);
    }
    let legend = chart.legend.as_mut().unwrap();
    legend.position = LegendPosition::Right;
    legend.grid = LegendGrid::Columns(1);
    let mut without_legend = chart.clone();
    without_legend.legend = None;
    let base = layout(&without_legend).unwrap();
    let expanded = layout(&chart).unwrap();
    let legend_bounds = expanded.legend.unwrap();
    assert!(legend_bounds.bottom() <= expanded.display_list.height.get());
    assert!(expanded.display_list.height.get() > chart.height_pt);
    assert!((expanded.axes.height - base.axes.height).abs() < 1e-6);
    assert!((expanded.x_label_bounds.y - base.x_label_bounds.y).abs() < 1e-6);
}

#[test]
fn wide_inside_legend_gets_extra_canvas_without_resizing_axes() {
    let mut chart = marker_gallery_fixture();
    let legend = chart.legend.as_mut().unwrap();
    legend.position = LegendPosition::FigurePoints { x: 30.0, y: 20.0 };
    legend.grid = LegendGrid::Columns(7);
    let mut without_legend = chart.clone();
    without_legend.legend = None;
    let base = layout(&without_legend).unwrap();
    let expanded = layout(&chart).unwrap();
    assert!(expanded.legend.unwrap().right() <= expanded.display_list.width.get());
    assert!(expanded.display_list.width.get() > chart.width_pt);
    assert!((expanded.axes.width - base.axes.width).abs() < 1e-6);
    assert!((expanded.axes.height - base.axes.height).abs() < 1e-6);
}

#[test]
fn top_legend_keeps_figure_point_annotations_in_the_base_canvas() {
    let mut chart = publication_fixture();
    chart.annotations[0].position = AnnotationPosition::FigurePoints { x: 40.0, y: 35.0 };
    chart.annotations[0].offset_pt = (0.0, 0.0);
    let mut without_legend = chart.clone();
    without_legend.legend = None;
    let base = layout(&without_legend).unwrap();
    let find_annotation_y = |result: &layout_engine_spike::LayoutResult| {
        result
            .hit_map
            .items
            .iter()
            .find(|item| {
                item.node == chart.annotations[0].id && item.role == SelectableRole::Annotation
            })
            .unwrap()
            .bounds
            .y
    };
    // Figure-point annotation coordinates follow the unchanged base canvas,
    // whose origin shifts when a top legend band is added.
    chart.legend.as_mut().unwrap().position = LegendPosition::Above;
    let expanded = layout(&chart).unwrap();
    let shift = expanded.display_list.height.get() - chart.height_pt;
    assert!((find_annotation_y(&expanded) - find_annotation_y(&base) - shift).abs() < 1e-6);
    assert!((expanded.x_label_bounds.y - base.x_label_bounds.y - shift).abs() < 1e-6);
    assert!(expanded.x_label_bounds.y > expanded.axes.bottom());
}

#[test]
fn automatic_legend_moves_outside_for_five_or_seven_dense_curves() {
    for count in [5, 7] {
        let mut chart = marker_gallery_fixture();
        chart.series.truncate(count);
        chart.legend.as_mut().unwrap().position = LegendPosition::Auto;
        for (index, series) in chart.series.iter_mut().enumerate() {
            let y = -2.0 + index as f64 * 4.0 / (count - 1) as f64;
            series.points = (0..=12)
                .map(|step| DataPoint {
                    x: -3.0 + step as f64 * 0.5,
                    y,
                })
                .collect();
            series.line = Some(LineStyle {
                width: 1.0,
                dash: DashStyle::Solid,
            });
        }
        let result = layout(&chart).unwrap();
        let mut without_legend = chart.clone();
        without_legend.legend = None;
        let base = layout(&without_legend).unwrap();
        let legend = result.legend.unwrap();
        assert_eq!(legend.intersection_area(result.axes), 0.0);
        assert!(legend.bottom() < result.axes.y || legend.x > result.axes.right());
        assert_eq!(result.axes.width, base.axes.width);
        assert_eq!(result.axes.height, base.axes.height);
        assert!(
            !result
                .warnings
                .iter()
                .any(|warning| matches!(warning, LayoutWarning::NonConvergent { .. }))
        );
    }
}

#[test]
fn filled_pentagon_and_star_markers_have_distinct_closed_vector_paths() {
    for (shape, expected_verbs) in [(MarkerShape::Pentagon, 6), (MarkerShape::Star, 11)] {
        let mut chart = marker_gallery_fixture();
        chart.series.truncate(1);
        chart.series[0].marker = Some(MarkerStyle {
            shape,
            size: 5.0,
            filled: true,
            interval: 1,
        });
        let result = layout(&chart).unwrap();
        let paths = result
            .display_list
            .items
            .iter()
            .filter_map(|item| match item {
                DisplayItem::Path {
                    source,
                    path,
                    fill: Some(_),
                    ..
                } if *source == NodeId(100) => Some(path),
                _ => None,
            })
            .collect::<Vec<_>>();
        // The two data points and the legend swatch use the same vector shape.
        assert!(paths.len() >= 2);
        assert!(paths.iter().all(|path| {
            path.verbs.len() == expected_verbs && matches!(path.verbs.last(), Some(PathVerb::Close))
        }));
    }
}

#[test]
fn every_closed_marker_has_a_hollow_data_and_legend_outline() {
    for shape in [
        MarkerShape::Circle,
        MarkerShape::Square,
        MarkerShape::TriangleUp,
        MarkerShape::TriangleDown,
        MarkerShape::Diamond,
        MarkerShape::Pentagon,
        MarkerShape::Star,
    ] {
        let mut chart = marker_gallery_fixture();
        chart.series.truncate(1);
        chart.series[0].marker = Some(MarkerStyle {
            shape,
            size: 5.0,
            filled: false,
            interval: 1,
        });
        let result = layout(&chart).unwrap();
        let outlines = result
            .display_list
            .items
            .iter()
            .filter_map(|item| match item {
                DisplayItem::Path {
                    source,
                    path,
                    fill,
                    stroke: Some(stroke),
                } if *source == chart.series[0].id
                    && matches!(path.verbs.last(), Some(PathVerb::Close)) =>
                {
                    assert!(fill.is_none(), "{shape:?} must not have an interior fill");
                    Some(stroke)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(
            outlines.len() >= 2,
            "{shape:?} needs data and legend outlines"
        );
        assert!(outlines.iter().all(|stroke| stroke.width.get() >= 1.0));
    }
}

#[test]
fn overlapping_hit_candidates_keep_topmost_first() {
    let bounds = Bounds {
        x: 10.0,
        y: 10.0,
        width: 20.0,
        height: 20.0,
    };
    let map = HitMap {
        items: vec![
            HitItem {
                node: NodeId(1),
                bounds,
                z_order: 0,
                role: SelectableRole::Axes,
                data_index: None,
                tooltip: None,
                path_proximity: Vec::new(),
            },
            HitItem {
                node: NodeId(2),
                bounds,
                z_order: 5,
                role: SelectableRole::Legend,
                data_index: None,
                tooltip: None,
                path_proximity: Vec::new(),
            },
        ],
    };
    let hits = map.hit_candidates(15.0, 15.0, 0.0);
    assert_eq!(
        hits.iter().map(|hit| hit.node).collect::<Vec<_>>(),
        vec![NodeId(2), NodeId(1)]
    );
    assert_eq!(map.hit_test(15.0, 15.0, 0.0).unwrap().node, NodeId(2));
}

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
    let legend_id = chart.legend.as_ref().unwrap().id;
    assert!(!first.display_list.items.iter().any(|item| matches!(
        item,
        DisplayItem::Path {
            source,
            fill: Some(fill),
            ..
        } if *source == legend_id && fill.color == Color(255, 255, 255, 230)
    )));
    assert!(
        first
            .hit_map
            .items
            .iter()
            .any(|item| { item.node == legend_id && item.role == SelectableRole::Legend })
    );
    assert!(first.display_list.width.get() > chart.width_pt);
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
            .filter(|((_, start_y), (_, end_y))| { *start_y == first.axes.y && end_y > start_y })
            .count(),
        x_tick_count
    );
    assert_eq!(
        y_ticks
            .iter()
            .filter(|((start_x, _), (end_x, _))| { *start_x == first.axes.x && end_x > start_x })
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
        assert!(
            matches!(
                diagnostic.origin,
                FontOrigin::BundledPrimary | FontOrigin::BundledSymbol
            ),
            "{diagnostic:?}"
        );
        assert!(!diagnostic.missing_glyph, "{diagnostic:?}");
    }

    let page = layout_engine_spike::Bounds {
        x: 0.0,
        y: 0.0,
        width: first.display_list.width.get(),
        height: first.display_list.height.get(),
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
    assert!(
        legend.right() <= first.display_list.width.get()
            && legend.bottom() <= first.display_list.height.get()
    );
    assert!(
        first
            .warnings
            .contains(&LayoutWarning::LegendMovedOutside { node: chart.id })
    );
    // Data-point hit boxes may extend beyond the clipped axes at an endpoint;
    // the painted marks cannot overlap the figure-level legend.
    assert_eq!(legend.intersection_area(first.axes), 0.0);
    for role in [
        SelectableRole::Axes,
        SelectableRole::Axis,
        SelectableRole::AxisLabel,
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
    let x_label_hit = first
        .hit_map
        .hit_test(
            first.x_label_bounds.x + first.x_label_bounds.width / 2.0,
            first.x_label_bounds.y + first.x_label_bounds.height / 2.0,
            0.0,
        )
        .unwrap();
    assert_eq!(x_label_hit.node, chart.x.id);
    assert_eq!(x_label_hit.role, SelectableRole::AxisLabel);
    let x_spine_hit = first
        .hit_map
        .hit_test(
            first.axes.x + first.axes.width * 0.37,
            first.axes.bottom(),
            1.0,
        )
        .unwrap();
    assert_eq!(x_spine_hit.node, chart.x.id);
    assert_eq!(x_spine_hit.role, SelectableRole::Axis);
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
    assert!(
        !result
            .warnings
            .iter()
            .any(|warning| matches!(warning, LayoutWarning::TextOutsideFigure { .. }))
    );
}

#[test]
fn p4_size_tick_and_long_label_matrix_is_unclipped_and_axis_independent() {
    for width_mm in [85.0, 89.0] {
        for scientific in [false, true] {
            let mut chart = publication_fixture();
            chart.width_pt = width_mm / 25.4 * 72.0;
            chart.x.minimum = -12_500.0;
            chart.x.maximum = 25_000.0;
            chart.x.formatter = if scientific {
                Formatter::Scientific { precision: 2 }
            } else {
                Formatter::Decimal { precision: 1 }
            };
            chart.x.label = Label::Group(vec![
                Label::Text("Applied field ".into()),
                Label::GreekVariable('μ'),
                Label::VariableSubscript(Box::new(Label::Text("maximum".into()))),
                Label::Text(" (".into()),
                Label::Unit("mT".into()),
                Label::Text(")".into()),
            ]);
            chart.y.label = Label::Group(vec![
                Label::Text("Current density ".into()),
                Label::Variable("J".into()),
                Label::VariableSubscript(Box::new(Label::Variable("e".into()))),
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
            assert!(result.axes.width >= 20.0 && result.axes.height >= 20.0);
            assert!(!result.warnings.iter().any(|warning| matches!(
                warning,
                LayoutWarning::TextOutsideFigure { .. }
                    | LayoutWarning::InsufficientPlotArea { .. }
            )));
        }
    }

    let base = layout(&publication_fixture()).unwrap();
    let mut long_x = publication_fixture();
    long_x.x.label = Label::Text("A deliberately much longer horizontal label with unit".into());
    let long_x = layout(&long_x).unwrap();
    assert!((base.axes.x - long_x.axes.x).abs() < 1e-9);

    let mut long_y = publication_fixture();
    long_y.y.label = Label::Text("A deliberately much longer vertical label with unit".into());
    let long_y = layout(&long_y).unwrap();
    assert!((base.axes.y - long_y.axes.y).abs() < 1e-9);
}

fn axis_segments(items: &[DisplayItem], source: NodeId) -> Vec<((f64, f64), (f64, f64))> {
    items
        .iter()
        .filter_map(|item| match item {
            DisplayItem::Path {
                source: node, path, ..
            } if *node == source
                && matches!(
                    path.verbs.as_slice(),
                    [PathVerb::MoveTo(..), PathVerb::LineTo(..)]
                ) =>
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
    assert!(
        x_ticks.iter().all(|((_, start_y), (_, end_y))| {
            *start_y == result.axes.bottom() && end_y > start_y
        })
    );
    let axis_spines = axis_segments(&result.display_list.items, chart.id);
    assert!(
        !axis_spines
            .iter()
            .any(|((_, y1), (_, y2))| { *y1 == result.axes.y && *y2 == result.axes.y })
    );
    assert!(
        !axis_spines
            .iter()
            .any(|((x1, _), (x2, _))| { *x1 == result.axes.x && *x2 == result.axes.x })
    );
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

#[test]
fn secondary_axes_expand_only_the_outer_canvas_and_map_series_independently() {
    let mut baseline_chart = publication_fixture();
    baseline_chart.legend = None;
    let baseline = layout(&baseline_chart).unwrap();

    let mut dual_y = baseline_chart.clone();
    let mut y2 = dual_y.y.clone();
    y2.id = NodeId(30);
    y2.label = Label::Text("Secondary Y".into());
    y2.minimum = -100.0;
    y2.maximum = 100.0;
    dual_y.y2 = Some(y2);
    let marker_id = dual_y
        .series
        .iter()
        .find(|series| series.marker.is_some())
        .unwrap()
        .id;
    dual_y.series_axes.insert(marker_id, AxisPair::X1Y2);
    let placed = layout(&dual_y).unwrap();

    assert!((placed.axes.width - baseline.axes.width).abs() < 0.05);
    assert!((placed.axes.height - baseline.axes.height).abs() < 0.05);
    assert!(placed.display_list.width.get() > baseline.display_list.width.get());
    assert!(
        placed
            .y2_axis
            .as_ref()
            .is_some_and(|axis| !axis.major.is_empty())
    );
    assert!(placed.y2_label_bounds.is_some());
    let right_edge_owners = placed
        .hit_map
        .items
        .iter()
        .filter(|item| {
            item.role == SelectableRole::Axis
                && (item.bounds.x - placed.axes.right()).abs() < 0.01
                && item.bounds.height > 0.0
        })
        .map(|item| item.node)
        .collect::<Vec<_>>();
    assert_eq!(right_edge_owners, vec![NodeId(30)]);

    let marker_point = placed
        .hit_map
        .items
        .iter()
        .find(|item| item.node == marker_id && item.role == SelectableRole::DataPoint)
        .unwrap();
    let source_y = dual_y
        .series
        .iter()
        .find(|series| series.id == marker_id)
        .unwrap()
        .points[marker_point.data_index.unwrap()]
    .y;
    let expected_fraction = (source_y + 100.0) / 200.0;
    let expected_y = placed.axes.bottom() - expected_fraction * placed.axes.height;
    assert!((marker_point.path_proximity[0].1 - expected_y).abs() < 0.01);
    assert!(!placed.warnings.iter().any(|warning| {
        matches!(warning, LayoutWarning::TextOutsideFigure { node, .. } if *node == NodeId(30))
    }));

    let mut dual_x = baseline_chart;
    let mut x2 = dual_x.x.clone();
    x2.id = NodeId(32);
    x2.label = Label::Text("Secondary X".into());
    x2.minimum = -300.0;
    x2.maximum = 300.0;
    dual_x.x2 = Some(x2);
    dual_x.series_axes.insert(marker_id, AxisPair::X2Y1);
    let placed_x = layout(&dual_x).unwrap();
    assert!((placed_x.axes.width - baseline.axes.width).abs() < 0.05);
    assert!((placed_x.axes.height - baseline.axes.height).abs() < 0.05);
    assert!(placed_x.display_list.height.get() > baseline.display_list.height.get());
    assert!(
        placed_x
            .x2_axis
            .as_ref()
            .is_some_and(|axis| !axis.major.is_empty())
    );
    let marker_point_x = placed_x
        .hit_map
        .items
        .iter()
        .find(|item| item.node == marker_id && item.role == SelectableRole::DataPoint)
        .unwrap();
    let source_x = dual_x
        .series
        .iter()
        .find(|series| series.id == marker_id)
        .unwrap()
        .points[marker_point_x.data_index.unwrap()]
    .x;
    let expected_x = placed_x.axes.x + (source_x + 300.0) / 600.0 * placed_x.axes.width;
    assert!((marker_point_x.path_proximity[0].0 - expected_x).abs() < 0.01);
}

#[test]
fn an_enabled_secondary_axis_without_data_owns_its_edge_without_fake_ticks() {
    let mut chart = publication_fixture();
    let mut x2 = chart.x.clone();
    x2.id = NodeId(31);
    x2.label = Label::Text("Secondary X".into());
    x2.has_data = false;
    chart.x2 = Some(x2);
    let placed = layout(&chart).unwrap();

    assert!(
        placed
            .x2_axis
            .as_ref()
            .is_some_and(|axis| axis.major.is_empty())
    );
    assert!(
        !placed
            .hit_map
            .items
            .iter()
            .any(|item| item.node == NodeId(31) && item.role == SelectableRole::Tick)
    );
    let top_edge_owners = placed
        .hit_map
        .items
        .iter()
        .filter(|item| {
            item.role == SelectableRole::Axis
                && (item.bounds.y - placed.axes.y).abs() < 0.01
                && item.bounds.width > 0.0
        })
        .map(|item| item.node)
        .collect::<Vec<_>>();
    assert_eq!(top_edge_owners, vec![NodeId(31)]);
}

#[test]
fn scientific_guides_have_distinct_layers_and_hit_roles() {
    let mut chart = publication_fixture();
    chart.reference_lines.push(ReferenceLine {
        id: NodeId(700),
        orientation: ReferenceOrientation::Vertical,
        value: 0.25,
        axes: AxisPair::X1Y1,
        stroke: LineStyle {
            width: 0.9,
            dash: DashStyle::Dashed,
        },
        color: Color(0, 0, 0, 255),
    });
    chart.measurement_arrows.push(MeasurementArrow {
        id: NodeId(701),
        start: DataPoint { x: -1.0, y: 1.0 },
        end: DataPoint { x: 1.0, y: 1.0 },
        axes: AxisPair::X1Y1,
        stroke: LineStyle {
            width: 0.9,
            dash: DashStyle::Solid,
        },
        color: Color(0, 0, 0, 255),
        start_arrow: true,
        end_arrow: true,
        arrow_head: ArrowHead::Filled,
        arrow_size: 5.0,
        constraint: MeasurementConstraint::Horizontal,
        labels: vec![Label::Text("Δx".into())],
        label_offset_pt: (0.0, -8.0),
    });
    let placed = layout(&chart).unwrap();
    assert!(
        placed
            .hit_map
            .items
            .iter()
            .any(|item| { item.node == NodeId(700) && item.role == SelectableRole::ReferenceLine })
    );
    for role in [
        SelectableRole::MeasurementArrow,
        SelectableRole::MeasurementArrowStart,
        SelectableRole::MeasurementArrowEnd,
        SelectableRole::MeasurementArrowLabel,
    ] {
        assert!(
            placed
                .hit_map
                .items
                .iter()
                .any(|item| item.node == NodeId(701) && item.role == role)
        );
    }
    let reference_item = placed
        .display_list
        .items
        .iter()
        .position(|item| matches!(item, DisplayItem::Path { source, .. } if *source == NodeId(700)))
        .unwrap();
    let first_data_item = placed
        .display_list
        .items
        .iter()
        .position(|item| matches!(item, DisplayItem::Path { source, .. } if chart.series.iter().any(|series| series.id == *source)))
        .unwrap();
    let measurement_item = placed
        .display_list
        .items
        .iter()
        .position(|item| matches!(item, DisplayItem::Path { source, .. } if *source == NodeId(701)))
        .unwrap();
    assert!(reference_item < first_data_item);
    assert!(measurement_item > first_data_item);
}

#[test]
fn filled_arrow_shafts_end_at_head_bases_and_tips_stay_on_endpoints() {
    let mut chart = publication_fixture();
    chart.measurement_arrows.push(MeasurementArrow {
        id: NodeId(710),
        start: DataPoint { x: -1.0, y: 1.0 },
        end: DataPoint { x: 1.0, y: 1.0 },
        axes: AxisPair::X1Y1,
        stroke: LineStyle {
            width: 4.0,
            dash: DashStyle::Solid,
        },
        color: Color(0, 0, 0, 255),
        start_arrow: true,
        end_arrow: true,
        arrow_head: ArrowHead::Filled,
        arrow_size: 8.0,
        constraint: MeasurementConstraint::Horizontal,
        labels: Vec::new(),
        label_offset_pt: (0.0, 0.0),
    });
    chart.annotations[0].connectors.push(AnnotationConnector {
        target: DataPoint { x: 2.0, y: -1.0 },
        stroke: LineStyle {
            width: 4.0,
            dash: DashStyle::Solid,
        },
        color: Color(0, 0, 0, 255),
        start_arrow: false,
        end_arrow: true,
        arrow_head: ArrowHead::Filled,
        arrow_size: 8.0,
        axes: AxisPair::X1Y1,
    });

    let placed = layout(&chart).unwrap();
    let endpoint = |role| {
        placed
            .hit_map
            .items
            .iter()
            .find(|item| item.node == NodeId(710) && item.role == role)
            .unwrap()
            .path_proximity[0]
    };
    let start = endpoint(SelectableRole::MeasurementArrowStart);
    let end = endpoint(SelectableRole::MeasurementArrowEnd);
    let arrow_paths = placed
        .display_list
        .items
        .iter()
        .filter_map(|item| match item {
            DisplayItem::Path {
                source,
                path,
                fill,
                stroke,
            } if *source == NodeId(710) => Some((path, fill, stroke)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let (shaft, _, _) = arrow_paths
        .iter()
        .copied()
        .find(|(_, _, stroke)| stroke.is_some())
        .unwrap();
    let [
        PathVerb::MoveTo(shaft_start_x, shaft_start_y),
        PathVerb::LineTo(shaft_end_x, shaft_end_y),
    ] = shaft.verbs.as_slice()
    else {
        panic!("filled arrow shaft should contain exactly one trimmed segment")
    };
    assert!((shaft_start_x.get() - start.0 - 8.0).abs() < 0.01);
    assert!((shaft_start_y.get() - start.1).abs() < 0.01);
    assert!((shaft_end_x.get() - end.0 + 8.0).abs() < 0.01);
    assert!((shaft_end_y.get() - end.1).abs() < 0.01);
    let tips = arrow_paths
        .iter()
        .filter(|(_, fill, _)| fill.is_some())
        .filter_map(|(path, _, _)| match path.verbs.first() {
            Some(PathVerb::MoveTo(x, y)) => Some((x.get(), y.get())),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        tips.iter()
            .any(|tip| { (tip.0 - start.0).abs() < 0.01 && (tip.1 - start.1).abs() < 0.01 })
    );
    assert!(
        tips.iter()
            .any(|tip| (tip.0 - end.0).abs() < 0.01 && (tip.1 - end.1).abs() < 0.01)
    );
    let arrow_bounds = placed
        .hit_map
        .items
        .iter()
        .find(|item| item.node == NodeId(710) && item.role == SelectableRole::MeasurementArrow)
        .unwrap()
        .bounds;
    for (path, fill, _) in &arrow_paths {
        if fill.is_none() {
            continue;
        }
        for verb in &path.verbs {
            if let PathVerb::MoveTo(x, y) | PathVerb::LineTo(x, y) = verb {
                assert!((arrow_bounds.x..=arrow_bounds.x + arrow_bounds.width).contains(&x.get()));
                assert!((arrow_bounds.y..=arrow_bounds.y + arrow_bounds.height).contains(&y.get()));
            }
        }
    }

    let connector_paths = placed
        .display_list
        .items
        .iter()
        .filter_map(|item| match item {
            DisplayItem::Path {
                source,
                path,
                fill,
                stroke,
            } if *source == chart.annotations[0].id => Some((path, fill, stroke)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let connector_shaft = connector_paths
        .iter()
        .find(|(_, _, stroke)| stroke.is_some())
        .map(|(path, _, _)| *path)
        .unwrap();
    let connector_tip = connector_paths
        .iter()
        .find(|(_, fill, _)| fill.is_some())
        .and_then(|(path, _, _)| path.verbs.first())
        .and_then(|verb| match verb {
            PathVerb::MoveTo(x, y) => Some((x.get(), y.get())),
            _ => None,
        })
        .unwrap();
    let PathVerb::LineTo(connector_shaft_x, connector_shaft_y) = connector_shaft.verbs[1] else {
        unreachable!()
    };
    assert!(
        (connector_shaft_x.get() - connector_tip.0)
            .hypot(connector_shaft_y.get() - connector_tip.1)
            > 7.9
    );
    let connector_bounds = placed
        .hit_map
        .items
        .iter()
        .find(|item| {
            item.node == chart.annotations[0].id
                && item.role == SelectableRole::AnnotationConnector
                && item.data_index == Some(0)
        })
        .unwrap()
        .bounds;
    for (path, fill, _) in &connector_paths {
        if fill.is_none() {
            continue;
        }
        for verb in &path.verbs {
            if let PathVerb::MoveTo(x, y) | PathVerb::LineTo(x, y) = verb {
                assert!(
                    (connector_bounds.x..=connector_bounds.x + connector_bounds.width)
                        .contains(&x.get())
                );
                assert!(
                    (connector_bounds.y..=connector_bounds.y + connector_bounds.height)
                        .contains(&y.get())
                );
            }
        }
    }
}

#[test]
fn axis_ink_is_black_while_secondary_spines_can_use_distinct_colors() {
    let mut chart = publication_fixture();
    chart.x.appearance.spine_color = Color(0, 0, 0, 255);
    chart.y.appearance.spine_color = Color(0, 0, 0, 255);
    let mut y2 = chart.y.clone();
    y2.id = NodeId(702);
    y2.appearance.spine_color = Color(204, 0, 0, 255);
    chart.y2 = Some(y2);
    let placed = layout(&chart).unwrap();
    assert!(placed.display_list.items.iter().any(|item| matches!(
        item,
        DisplayItem::Path {
            source,
            stroke: Some(stroke),
            ..
        } if *source == NodeId(702) && stroke.color == Color(204, 0, 0, 255)
    )));
    assert!(
        placed
            .display_list
            .items
            .iter()
            .filter_map(|item| match item {
                DisplayItem::GlyphRun(run) => Some(run.color),
                _ => None,
            })
            .all(|color| color == Color(0, 0, 0, 255))
    );
}
