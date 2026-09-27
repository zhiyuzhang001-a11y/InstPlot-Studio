use std::path::PathBuf;

use super::*;
use instplot_core::NumericColumn;
use instplot_layout::SelectableRole;
use instplot_render::{Color, DisplayItem};

fn error_dataset() -> DataSet {
    DataSet {
        source: PathBuf::from("errors.csv"),
        label: Some("errors.csv".to_owned()),
        kind: DataSetKind::Source,
        plot_id: "errors".to_owned(),
        fit_link: None,
        encoding: "UTF-8".to_owned(),
        separator: "comma".to_owned(),
        columns: vec![
            NumericColumn {
                name: "x".to_owned(),
                values: vec![1.0, 2.0],
            },
            NumericColumn {
                name: "y".to_owned(),
                values: vec![10.0, 20.0],
            },
            NumericColumn {
                name: "xe".to_owned(),
                values: vec![0.5, 0.25],
            },
            NumericColumn {
                name: "ye".to_owned(),
                values: vec![3.0, 4.0],
            },
        ],
        row_count: 2,
        alive: vec![true; 2],
    }
}

#[test]
fn multiple_new_annotations_are_distinct_and_start_inside_small_canvases() {
    let mut document = FigureDocument::showcase();
    document.set_figure_size_mm(20.0, 20.0).unwrap();
    let first = document
        .add_annotation(vec![LabelNode::Text("A".to_owned())])
        .unwrap();
    let second = document
        .add_annotation(vec![LabelNode::Text("B".to_owned())])
        .unwrap();
    assert_ne!(first, second);
    let positions = [first, second].map(|id| {
        let artist = document.artist_record(&id).unwrap();
        let ArtistProperties::Annotation { x_pt, y_pt, .. } = artist.properties else {
            panic!("new artist is not an annotation")
        };
        assert!((12.0..=45.0).contains(&x_pt));
        assert!((12.0..=45.0).contains(&y_pt));
        (x_pt, y_pt)
    });
    assert_ne!(positions[0], positions[1]);
    assert!(positions[0].0 < 45.0 / 2.0);
    assert!(positions[0].1 > 45.0 / 2.0);
    document.project().validate().unwrap();
}

#[test]
fn multiline_annotation_and_multiple_arrow_modes_round_trip_and_render() {
    let mut document = FigureDocument::showcase();
    let annotation_id = document
        .add_annotation(vec![LabelNode::Text("First line\nSecond line".to_owned())])
        .unwrap();
    let mut annotation = document.artist_record(&annotation_id).unwrap();
    let ArtistProperties::Annotation { connectors, .. } = &mut annotation.properties else {
        panic!("new artist is not an annotation")
    };
    for (index, (start_arrow, end_arrow)) in [(false, false), (false, true), (true, true)]
        .into_iter()
        .enumerate()
    {
        connectors.push(crate::AnnotationConnectorRecord {
            target_x: -1.0 + index as f64,
            target_y: 0.5 + index as f64 * 0.2,
            stroke: StrokeStyle {
                color_id: "blue".to_owned(),
                width_pt: 0.8,
                dash_pt: if index == 0 {
                    Vec::new()
                } else {
                    vec![8.0, 3.0]
                },
            },
            start_arrow,
            end_arrow,
            arrow_size_pt: 5.0,
        });
    }
    document.set_artist_record(annotation).unwrap();
    let encoded = serde_json::to_vec(document.project()).unwrap();
    let reopened =
        FigureDocument::from_project(crate::project::decode_project(&encoded).unwrap()).unwrap();
    let layout = reopened.layout_figure().unwrap();
    let node = layout
        .project_ids
        .iter()
        .find_map(|(node, id)| (id == &annotation_id).then_some(*node))
        .unwrap();
    let text_count = layout
        .result
        .display_list
        .items
        .iter()
        .filter(|item| matches!(item, DisplayItem::GlyphRun(run) if run.source == node))
        .count();
    let connector_count = layout
        .result
        .display_list
        .items
        .iter()
        .filter(|item| matches!(item, DisplayItem::Path { source, .. } if *source == node))
        .count();
    assert_eq!(text_count, 2);
    assert_eq!(connector_count, 3);
}

#[test]
fn error_columns_drive_autoscale_reject_negative_values_and_round_trip() {
    let mut document = FigureDocument::from_datasets(&[error_dataset()]).unwrap();
    assert!(document.axis_record(AxisDimension::X).autoscale);
    assert!(document.axis_record(AxisDimension::Y).autoscale);
    let series_id = document
        .series()
        .into_iter()
        .find(|series| matches!(series.kind, SeriesKind::Line | SeriesKind::Scatter))
        .unwrap()
        .id;
    document
        .set_series_error_columns(&series_id, Some("xe"), Some("ye"))
        .unwrap();
    let ranges = document.axis_ranges();
    assert!(ranges.x_min < 0.5 && ranges.x_max > 2.25);
    assert!(ranges.y_min < 7.0 && ranges.y_max > 24.0);
    let encoded = serde_json::to_vec(document.project()).unwrap();
    let reopened =
        FigureDocument::from_project(crate::project::decode_project(&encoded).unwrap()).unwrap();
    assert!(
        reopened
            .project()
            .figure
            .artists
            .iter()
            .any(|artist| matches!(
                &artist.properties,
                ArtistProperties::ErrorBar { x_error_column, y_error_column, .. }
                    if x_error_column.as_deref() == Some("xe") && y_error_column == "ye"
            ))
    );

    let mut invalid = error_dataset();
    invalid.columns[3].values[0] = -0.1;
    let mut invalid_document = FigureDocument::from_datasets(&[invalid]).unwrap();
    let invalid_series = invalid_document
        .series()
        .into_iter()
        .find(|series| matches!(series.kind, SeriesKind::Line | SeriesKind::Scatter))
        .unwrap()
        .id;
    let before = invalid_document.clone();
    assert!(
        invalid_document
            .set_series_error_columns(&invalid_series, None, Some("ye"))
            .unwrap_err()
            .contains("invalid value")
    );
    assert_eq!(invalid_document, before);
}

#[test]
fn adding_error_columns_immediately_expands_both_automatic_axes_past_every_cap() {
    let mut document = FigureDocument::from_datasets(&[error_dataset()]).unwrap();
    let series_id = document
        .series()
        .into_iter()
        .find(|series| matches!(series.kind, SeriesKind::Line | SeriesKind::Scatter))
        .unwrap()
        .id;

    let before = document.axis_ranges();
    assert!(before.x_min > 0.5 && before.x_max < 2.25);
    assert!(before.y_min > 7.0 && before.y_max < 24.0);

    document
        .set_series_error_columns(&series_id, Some("xe"), Some("ye"))
        .unwrap();

    let after = document.axis_ranges();
    assert!(
        after.x_min < 0.5,
        "left X error cap must have visual padding"
    );
    assert!(
        after.x_max > 2.25,
        "right X error cap must have visual padding"
    );
    assert!(
        after.y_min < 7.0,
        "lower Y error cap must have visual padding"
    );
    assert!(
        after.y_max > 24.0,
        "upper Y error cap must have visual padding"
    );
}

#[test]
fn new_curve_and_error_bar_defaults_use_one_point_strokes() {
    let mut document = FigureDocument::from_datasets(&[error_dataset()]).unwrap();
    let series_id = document
        .series()
        .into_iter()
        .find(|series| matches!(series.kind, SeriesKind::Line | SeriesKind::Scatter))
        .unwrap()
        .id;
    document
        .set_series_style(&series_id, SeriesCreationStyle::Line)
        .unwrap();
    document
        .set_series_error_columns(&series_id, None, Some("ye"))
        .unwrap();

    let widths = document
        .project()
        .figure
        .artists
        .iter()
        .filter_map(|artist| match &artist.properties {
            ArtistProperties::Line { stroke, .. } | ArtistProperties::ErrorBar { stroke, .. } => {
                Some(stroke.width_pt)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(widths.contains(&DEFAULT_CURVE_WIDTH_PT));
    assert!(widths.contains(&DEFAULT_ERROR_BAR_WIDTH_PT));
    assert!(
        widths
            .iter()
            .all(|width| (*width - 1.0).abs() < f64::EPSILON)
    );
}

#[test]
fn legend_error_key_uses_the_real_error_style_and_marker_relative_height() {
    let mut document = FigureDocument::from_datasets(&[error_dataset()]).unwrap();
    let series_id = document
        .series()
        .into_iter()
        .find(|series| matches!(series.kind, SeriesKind::Line | SeriesKind::Scatter))
        .unwrap()
        .id;
    document
        .set_series_error_columns(&series_id, Some("xe"), Some("ye"))
        .unwrap();
    let mut error = document
        .project()
        .figure
        .artists
        .iter()
        .find(|artist| artist.kind == ArtistKind::ErrorBar)
        .cloned()
        .unwrap();
    let ArtistProperties::ErrorBar {
        cap_width_pt,
        stroke,
        ..
    } = &mut error.properties
    else {
        unreachable!()
    };
    *cap_width_pt = 8.0;
    stroke.color_id = "object-purple".to_owned();
    stroke.width_pt = 1.7;
    stroke.dash_pt = vec![6.0, 2.0, 0.8, 2.0, 0.8, 2.0];
    document.set_artist_record(error).unwrap();
    let purple = palette_color("object-purple", &document.project().palette).unwrap();
    let layout = document.layout_figure().unwrap();
    let series_node = layout
        .project_ids
        .iter()
        .find_map(|(node, id)| (id == &series_id).then_some(*node))
        .unwrap();
    let legend_error = layout
        .result
        .display_list
        .items
        .iter()
        .find_map(|item| match item {
            DisplayItem::Path {
                source,
                path,
                stroke: Some(stroke),
                ..
            } if *source == series_node
                && stroke.color == purple
                && (stroke.width.get() - 1.7).abs() < 1.0e-9
                && path.verbs.len() == 6 =>
            {
                Some((path, stroke))
            }
            _ => None,
        })
        .expect("legend should contain the actual error style");
    assert_eq!(
        legend_error
            .1
            .dash
            .iter()
            .map(|value| value.get())
            .collect::<Vec<_>>(),
        vec![6.0, 2.0, 0.8, 2.0, 0.8, 2.0]
    );
    let instplot_render::PathVerb::MoveTo(_, top) = legend_error.0.verbs[0] else {
        unreachable!()
    };
    let instplot_render::PathVerb::LineTo(_, bottom) = legend_error.0.verbs[1] else {
        unreachable!()
    };
    assert!((9.0..=10.5).contains(&(bottom.get() - top.get()).abs()));
}

#[test]
fn all_seven_dash_patterns_map_to_distinct_layout_styles() {
    let patterns = [
        (Vec::new(), DashStyle::Solid),
        (vec![4.0, 2.4], DashStyle::Dashed),
        (vec![0.8, 1.8], DashStyle::Dotted),
        (vec![4.0, 2.0, 0.8, 2.0], DashStyle::DashDot),
        (vec![8.0, 3.0], DashStyle::LongDash),
        (vec![8.0, 2.0, 2.0, 2.0], DashStyle::LongShortDash),
        (vec![6.0, 2.0, 0.8, 2.0, 0.8, 2.0], DashStyle::DashDotDot),
    ];
    for (dash_pt, expected) in patterns {
        assert_eq!(
            dash_style(&StrokeStyle {
                color_id: "object-blue".to_owned(),
                width_pt: 0.8,
                dash_pt,
            }),
            expected
        );
    }
}

#[test]
fn clearing_last_source_resets_only_data_dependent_axis_state() {
    let mut document = FigureDocument::from_datasets(&[error_dataset()]).unwrap();
    document
        .set_axis_label(AxisDimension::X, vec![LabelNode::Text("old x".to_owned())])
        .unwrap();
    document
        .set_axis_label(AxisDimension::Y, vec![LabelNode::Text("old y".to_owned())])
        .unwrap();
    document
        .set_axis_ranges(AxisRanges {
            x_min: -8.0,
            x_max: 12.0,
            y_min: -3.0,
            y_max: 30.0,
        })
        .unwrap();
    for dimension in [AxisDimension::X, AxisDimension::Y] {
        let mut axis = document.axis_record(dimension);
        axis.locator = LocatorSpec::Interval { step: 1.0 };
        axis.minor_interval = Some(0.25);
        document.set_axis_record(dimension, axis).unwrap();
    }
    let x_appearance = document.project().figure.axes[0].x.appearance;
    let y_appearance = document.project().figure.axes[0].y.appearance;
    document
        .delete_data_sources(&["errors".to_owned()])
        .unwrap();
    assert_eq!(
        document.axis_ranges(),
        AxisRanges {
            x_min: 0.0,
            x_max: 1.0,
            y_min: 0.0,
            y_max: 1.0
        }
    );
    for dimension in [AxisDimension::X, AxisDimension::Y] {
        let axis = document.axis_record(dimension);
        assert!(matches!(axis.locator, LocatorSpec::Auto { .. }));
        assert_eq!(axis.minor_interval, None);
        assert_eq!(
            document.axis_label(dimension),
            [LabelNode::Text(String::new())]
        );
    }
    assert_eq!(document.project().figure.axes[0].x.appearance, x_appearance);
    assert_eq!(document.project().figure.axes[0].y.appearance, y_appearance);
}

#[test]
fn fixed_document_compiles_to_one_deterministic_display_list() {
    let document = FigureDocument::fixed();
    let display = document.compile().unwrap();
    assert!(display.validation_errors().is_empty());
    assert_eq!(document.series().len(), 6);
    assert_eq!(document.series()[1].kind, SeriesKind::Line);
    assert_eq!(document.series()[1].id, "node-11");
}

#[test]
fn axis_edits_mutate_document_state_and_recompile() {
    let mut document = FigureDocument::fixed();
    let ranges = AxisRanges {
        x_min: -4.0,
        x_max: 4.0,
        y_min: -3.0,
        y_max: 3.0,
    };
    document.set_axis_ranges(ranges).unwrap();
    assert_eq!(document.axis_ranges(), ranges);
    assert!(document.compile().is_ok());
}

#[test]
fn invalid_axis_edits_do_not_change_the_document() {
    let mut document = FigureDocument::fixed();
    let before = document.axis_ranges();
    let invalid = AxisRanges {
        x_min: 2.0,
        x_max: 1.0,
        ..before
    };
    assert!(document.set_axis_ranges(invalid).is_err());
    assert_eq!(document.axis_ranges(), before);
}

#[test]
fn empty_or_entirely_missing_handoff_data_fail_with_specific_errors() {
    let empty = DataSet {
        source: PathBuf::from("empty.txt"),
        label: Some("Empty".to_owned()),
        kind: DataSetKind::Source,
        plot_id: "empty-source".to_owned(),
        fit_link: None,
        encoding: "UTF-8".to_owned(),
        separator: "tab".to_owned(),
        columns: vec![
            NumericColumn {
                name: "x".to_owned(),
                values: Vec::new(),
            },
            NumericColumn {
                name: "y".to_owned(),
                values: Vec::new(),
            },
        ],
        row_count: 0,
        alive: Vec::new(),
    };
    assert!(
        FigureDocument::from_datasets(&[])
            .unwrap_err()
            .to_string()
            .contains("no datasets")
    );
    assert!(
        FigureDocument::from_datasets(std::slice::from_ref(&empty))
            .unwrap_err()
            .to_string()
            .contains("no alive plotted rows")
    );

    let mut non_finite = empty;
    non_finite.row_count = 1;
    non_finite.alive = vec![true];
    non_finite.columns[0].values = vec![f64::NAN];
    non_finite.columns[1].values = vec![1.0];
    assert!(
        FigureDocument::from_datasets(&[non_finite])
            .unwrap_err()
            .to_string()
            .contains("no alive plotted rows")
    );
}

#[test]
fn missing_cells_are_preserved_and_omitted_only_from_affected_series() {
    let dataset = DataSet {
        source: PathBuf::from("missing.csv"),
        label: Some("Missing values".to_owned()),
        kind: DataSetKind::Source,
        plot_id: "missing-source".to_owned(),
        fit_link: None,
        encoding: "UTF-8".to_owned(),
        separator: "comma".to_owned(),
        columns: vec![
            NumericColumn {
                name: "x".to_owned(),
                values: vec![1.0, 2.0, 3.0],
            },
            NumericColumn {
                name: "y".to_owned(),
                values: vec![2.0, f64::NAN, 6.0],
            },
            NumericColumn {
                name: "complete".to_owned(),
                values: vec![10.0, 20.0, 30.0],
            },
        ],
        row_count: 3,
        alive: vec![true; 3],
    };
    let mut document = FigureDocument::from_datasets(&[dataset]).unwrap();
    let source = &document.project().data_sources[0];
    let DataSourcePayload::Embedded { columns, .. } = &source.payload else {
        panic!("imported data must be embedded");
    };
    assert_eq!(columns[1].values, [2.0, 0.0, 6.0]);
    assert_eq!(columns[1].valid, [true, false, true]);
    assert_eq!(
        bound_points(
            document.series()[0].binding.as_ref().unwrap(),
            document.project(),
        )
        .unwrap()
        .len(),
        2
    );

    let series = document.series()[0].clone();
    document
        .rebind_series(&series.id, "missing-source", "x", "complete", None)
        .unwrap();
    assert_eq!(
        bound_points(
            document.series()[0].binding.as_ref().unwrap(),
            document.project(),
        )
        .unwrap()
        .len(),
        3
    );
}

#[test]
fn extreme_finite_scientific_ranges_compile_without_changing_precision() {
    let mut document = FigureDocument::fixed();
    let mut x = document.axis_record(AxisDimension::X);
    x.minimum = -1.0e12;
    x.maximum = 1.0e12;
    x.formatter = FormatterSpec::Scientific { precision: 6 };
    document
        .set_axis_record(AxisDimension::X, x.clone())
        .unwrap();

    let mut y = document.axis_record(AxisDimension::Y);
    y.minimum = -1.0e-9;
    y.maximum = 1.0e-9;
    y.formatter = FormatterSpec::Scientific { precision: 8 };
    document
        .set_axis_record(AxisDimension::Y, y.clone())
        .unwrap();

    let display = document.compile().unwrap();
    assert!(display.validation_errors().is_empty());
    assert_eq!(document.axis_record(AxisDimension::X), x);
    assert_eq!(document.axis_record(AxisDimension::Y), y);
}

#[test]
fn p4_axis_size_and_semantic_label_settings_round_trip() {
    let mut document = FigureDocument::fixed();
    document.set_figure_size_mm(89.0, 65.0).unwrap();

    let mut x_axis = document.axis_record(AxisDimension::X);
    x_axis.autoscale = false;
    x_axis.minimum = -2.0;
    x_axis.maximum = 2.0;
    x_axis.locator = LocatorSpec::Fixed {
        values: vec![-2.0, 0.0, 2.0],
    };
    x_axis.formatter = FormatterSpec::Scientific { precision: 3 };
    x_axis.appearance.tick_direction = crate::TickDirection::InOut;
    x_axis.appearance.far_tick_labels = true;
    x_axis.appearance.grid_minor = true;
    document
        .set_axis_record(AxisDimension::X, x_axis.clone())
        .unwrap();
    let label = vec![
        LabelNode::GreekVariable('μ'),
        LabelNode::VariableSubscript(vec![LabelNode::Text("eff".to_owned())]),
        LabelNode::UnitSeparator,
        LabelNode::Unit("mA cm^-2".to_owned()),
    ];
    document
        .set_axis_label(AxisDimension::X, label.clone())
        .unwrap();

    let encoded = serde_json::to_vec(document.project()).unwrap();
    let reopened =
        FigureDocument::from_project(crate::project::decode_project(&encoded).unwrap()).unwrap();
    assert_eq!(reopened.figure_size_mm(), (89.0, 65.0));
    assert_eq!(reopened.axis_record(AxisDimension::X), x_axis);
    assert_eq!(reopened.axis_label(AxisDimension::X), label.as_slice());
    assert_eq!(document.compile().unwrap(), reopened.compile().unwrap());
}

#[test]
fn canvas_first_axis_contract_hides_legacy_grid_and_uses_inward_ticks() {
    let mut document = FigureDocument::fixed();
    let mut x = document.axis_record(AxisDimension::X);
    x.appearance.grid_major = true;
    x.appearance.grid_minor = true;
    x.appearance.tick_direction = crate::TickDirection::Out;
    x.locator = LocatorSpec::Interval { step: 0.5 };
    document.set_axis_record(AxisDimension::X, x).unwrap();

    let spec = axis_spec(
        &document.axis_record(AxisDimension::X),
        100.0,
        &document.project().semantic_registry,
        &mut BTreeMap::new(),
    )
    .unwrap();
    assert!(!spec.grid.major && !spec.grid.minor);
    assert_eq!(spec.appearance.tick_direction, LayoutTickDirection::In);

    let layout = document.layout_axes().unwrap();
    assert!(layout.result.x_axis.major.len() >= 3);
    assert!(
        !layout.result.display_list.items.iter().any(|item| matches!(
            item,
            DisplayItem::Path {
                stroke: Some(stroke),
                ..
            } if stroke.color == Color(218, 221, 224, 255)
        ))
    );
    let encoded = serde_json::to_vec(document.project()).unwrap();
    let reopened =
        FigureDocument::from_project(crate::project::decode_project(&encoded).unwrap()).unwrap();
    assert_eq!(
        layout.result.snapshot(),
        reopened.layout_axes().unwrap().result.snapshot()
    );
}

#[test]
fn custom_minor_interval_reaches_both_axis_layouts() {
    let mut document = FigureDocument::fixed();
    let mut x = document.axis_record(AxisDimension::X);
    x.locator = LocatorSpec::Fixed {
        values: vec![-2.0, 0.0, 2.0],
    };
    x.minor_interval = Some(0.5);
    document.set_axis_record(AxisDimension::X, x).unwrap();
    let mut y = document.axis_record(AxisDimension::Y);
    y.locator = LocatorSpec::Fixed {
        values: vec![-2.0, 0.0, 2.0],
    };
    y.minor_interval = Some(0.5);
    document.set_axis_record(AxisDimension::Y, y).unwrap();
    let layout = document.layout_axes().unwrap();
    assert_eq!(
        layout.result.x_axis.minor,
        vec![-2.5, -1.5, -1.0, -0.5, 0.5, 1.0, 1.5, 2.5]
    );
    assert_eq!(
        layout.result.y_axis.minor,
        vec![-1.5, -1.0, -0.5, 0.5, 1.0, 1.5]
    );
}

#[test]
fn p4_autoscale_is_stable_and_failed_log_autoscale_is_atomic() {
    let mut document = FigureDocument::fixed();
    let mut x_axis = document.axis_record(AxisDimension::X);
    x_axis.autoscale = true;
    document.set_axis_record(AxisDimension::X, x_axis).unwrap();
    let autoscaled = document.axis_record(AxisDimension::X);
    assert!(autoscaled.minimum < autoscaled.maximum);

    let before = document.project().clone();
    let mut y_axis = document.axis_record(AxisDimension::Y);
    y_axis.autoscale = true;
    y_axis.scale = AxisScale::Log10;
    assert!(document.set_axis_record(AxisDimension::Y, y_axis).is_err());
    assert_eq!(document.project(), &before);
}

#[test]
fn p5_all_artist_properties_and_legend_entries_round_trip() {
    let mut document = FigureDocument::fixed();

    let mut line = document.artist_record("node-11").unwrap();
    line.role = ArtistRole::Theory;
    let ArtistProperties::Line { stroke, .. } = &mut line.properties else {
        panic!("node-11 must be a line")
    };
    stroke.color_id = "gray".to_owned();
    stroke.width_pt = 1.4;
    stroke.dash_pt = vec![4.0, 2.4];
    document.set_artist_record(line).unwrap();

    let mut scatter = document.artist_record("node-13").unwrap();
    let ArtistProperties::Scatter { marker, .. } = &mut scatter.properties else {
        panic!("node-13 must be a scatter")
    };
    marker.shape = MarkerShape::Diamond;
    marker.size_pt = 5.5;
    document.set_artist_record(scatter).unwrap();

    let mut errors = document.artist_record("node-12").unwrap();
    let ArtistProperties::ErrorBar {
        cap_width_pt,
        stroke,
        ..
    } = &mut errors.properties
    else {
        panic!("node-12 must be an error bar")
    };
    *cap_width_pt = 3.0;
    stroke.width_pt = 0.8;
    document.set_artist_record(errors).unwrap();

    let mut reference = document.artist_record("node-10").unwrap();
    let ArtistProperties::ReferenceLine {
        orientation, value, ..
    } = &mut reference.properties
    else {
        panic!("node-10 must be a reference line")
    };
    *orientation = ReferenceOrientation::Vertical;
    *value = 1.0;
    document.set_artist_record(reference).unwrap();

    let mut annotation = document.artist_record("node-14").unwrap();
    let ArtistProperties::Annotation { x_pt, y_pt, .. } = &mut annotation.properties else {
        panic!("node-14 must be an annotation")
    };
    *x_pt = 55.0;
    *y_pt = 22.0;
    document.set_artist_record(annotation).unwrap();
    document
        .set_semantic_label_nodes(
            "label-temperature",
            vec![
                LabelNode::Variable("T".to_owned()),
                LabelNode::Operator("=".to_owned()),
                LabelNode::Number("250".to_owned()),
                LabelNode::Unit("K".to_owned()),
            ],
        )
        .unwrap();

    let mut legend = document.artist_record("node-15").unwrap();
    let ArtistProperties::Legend {
        entries,
        x_pt,
        y_pt,
        ..
    } = &mut legend.properties
    else {
        panic!("node-15 must be a legend")
    };
    entries.reverse();
    entries[0].visible = false;
    *x_pt = 150.0;
    *y_pt = 25.0;
    document.set_artist_record(legend).unwrap();

    let resolved = document.layout_figure().unwrap();
    assert!(resolved.result.display_list.validation_errors().is_empty());
    assert_eq!(
        document
            .project()
            .overrides
            .iter()
            .filter(|record| record.property == "artist_style")
            .count(),
        6
    );
    let encoded = serde_json::to_vec(document.project()).unwrap();
    let reopened =
        FigureDocument::from_project(crate::project::decode_project(&encoded).unwrap()).unwrap();
    assert_eq!(document.project(), reopened.project());
    assert_eq!(document.compile().unwrap(), reopened.compile().unwrap());
}

#[test]
fn showcase_legend_placements_round_trip_into_layout() {
    for placement in [LegendPlacement::Above, LegendPlacement::Right] {
        let mut document = FigureDocument::showcase();
        let mut record = document.artist_record("node-15").unwrap();
        record.visible = true;
        let ArtistProperties::Legend {
            placement: current, ..
        } = &mut record.properties
        else {
            panic!("node-15 must be a legend")
        };
        *current = placement;
        document.set_artist_record(record).unwrap();
        let serialized = serde_json::to_vec(document.project()).unwrap();
        let reopened =
            FigureDocument::from_project(crate::project::decode_project(&serialized).unwrap())
                .unwrap();
        let resolved = reopened.layout_figure().unwrap();
        let legend = resolved.result.legend.unwrap();
        assert_eq!(legend.intersection_area(resolved.result.axes), 0.0);
        let (base_width_mm, base_height_mm) = reopened.figure_size_mm();
        assert_eq!((base_width_mm, base_height_mm), (85.0, 65.0));
        let base_width_pt = base_width_mm * 72.0 / 25.4;
        let base_height_pt = base_height_mm * 72.0 / 25.4;
        match placement {
            LegendPlacement::Above => {
                assert!(legend.bottom() < resolved.result.axes.y);
                assert!(resolved.result.display_list.height.get() > base_height_pt);
                assert!((resolved.result.display_list.width.get() - base_width_pt).abs() < 1e-6);
            }
            LegendPlacement::Right => {
                assert!(legend.x > resolved.result.axes.right());
                assert!(resolved.result.display_list.width.get() > base_width_pt);
                assert!((resolved.result.display_list.height.get() - base_height_pt).abs() < 1e-6);
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn outside_legend_manual_coordinates_round_trip_to_export_layout() {
    for (placement, origin) in [
        (LegendPlacement::Above, (24.0, 15.0)),
        (LegendPlacement::Right, (250.0, 18.0)),
    ] {
        let mut document = FigureDocument::showcase();
        let mut record = document.artist_record("node-15").unwrap();
        record.visible = true;
        let ArtistProperties::Legend {
            x_pt,
            y_pt,
            placement: current,
            position_custom,
            ..
        } = &mut record.properties
        else {
            panic!("node-15 must be a legend")
        };
        (*x_pt, *y_pt) = origin;
        *current = placement;
        *position_custom = true;
        document.set_artist_record(record).unwrap();
        let encoded = serde_json::to_vec(document.project()).unwrap();
        let reopened =
            FigureDocument::from_project(crate::project::decode_project(&encoded).unwrap())
                .unwrap();
        let result = reopened.layout_figure().unwrap().result;
        let bounds = result.legend.unwrap();
        assert_eq!((bounds.x, bounds.y), origin);
        assert_eq!(bounds.intersection_area(result.axes), 0.0);
        assert!(bounds.right() <= result.display_list.width.get());
        assert!(bounds.bottom() <= result.display_list.height.get());
    }
}

#[test]
fn showcase_legend_grid_round_trips_and_controls_export_layout() {
    for grid in [LegendGrid::Rows(2), LegendGrid::Columns(2)] {
        let mut document = FigureDocument::showcase();
        let mut record = document.artist_record("node-15").unwrap();
        record.visible = true;
        let ArtistProperties::Legend {
            grid: current,
            placement,
            ..
        } = &mut record.properties
        else {
            panic!("node-15 must be a legend")
        };
        *current = grid;
        *placement = LegendPlacement::Above;
        document.set_artist_record(record).unwrap();
        let serialized = serde_json::to_vec(document.project()).unwrap();
        let reopened =
            FigureDocument::from_project(crate::project::decode_project(&serialized).unwrap())
                .unwrap();
        let ArtistProperties::Legend { grid: saved, .. } =
            reopened.artist_record("node-15").unwrap().properties
        else {
            panic!("node-15 must be a legend")
        };
        assert_eq!(saved, grid);
        let resolved = reopened.layout_figure().unwrap();
        let visible = 7_usize;
        let expected_rows = match grid {
            LegendGrid::Rows(rows) => usize::from(rows),
            LegendGrid::Columns(columns) => visible.div_ceil(usize::from(columns)),
            LegendGrid::Auto => unreachable!(),
        };
        assert_eq!(
            resolved.result.legend.unwrap().height,
            expected_rows as f64 * 12.0 + 6.0
        );
        assert!(resolved.result.display_list.height.get() > 65.0 * 72.0 / 25.4);
    }
}

#[test]
fn p7_export_preferences_round_trip_and_drive_publication_dpi() {
    let mut document = FigureDocument::fixed();
    let mut preferences = document.export_preferences().clone();
    preferences.selected_raster_dpi = 600;
    preferences.transparent_background = true;
    document
        .set_export_preferences(preferences.clone())
        .unwrap();

    let encoded = serde_json::to_vec(document.project()).unwrap();
    let reopened =
        FigureDocument::from_project(crate::project::decode_project(&encoded).unwrap()).unwrap();
    assert_eq!(reopened.export_preferences(), &preferences);
    let resolved = crate::resolve_document(&reopened).unwrap();
    let report = crate::check_publication(
        &reopened,
        &resolved,
        reopened.export_preferences().selected_raster_dpi,
    );
    assert_eq!(report.raster_dpi, 600);
    assert!(
        report
            .findings
            .iter()
            .all(|finding| { !finding.impact.is_empty() && !finding.remediation.is_empty() })
    );
}

#[test]
fn project_round_trip_preserves_the_resolved_display_list() {
    let document = FigureDocument::fixed();
    let encoded = serde_json::to_vec(document.project()).unwrap();
    let decoded = crate::project::decode_project(&encoded).unwrap();
    let reopened = FigureDocument::from_project(decoded).unwrap();
    assert_eq!(document.compile().unwrap(), reopened.compile().unwrap());
}

#[test]
fn formal_axes_layout_is_deterministic_and_retains_project_identity() {
    let document = FigureDocument::fixed();
    let first = document.layout_axes().unwrap();
    let second = document.layout_axes().unwrap();

    assert_eq!(first.result.snapshot(), second.result.snapshot());
    assert_eq!(first.data_clip, first.result.axes);
    assert!(first.result.display_list.validation_errors().is_empty());
    assert!(first.result.warnings.is_empty());
    assert_eq!(first.project_ids.get(&NodeId(2)).unwrap(), "node-2");
    assert_eq!(first.project_ids.get(&NodeId(3)).unwrap(), "node-3");
    assert_eq!(first.project_ids.get(&NodeId(4)).unwrap(), "node-4");

    let grid_count = first
        .result
        .display_list
        .items
        .iter()
        .filter(|item| match item {
            DisplayItem::Path {
                stroke: Some(stroke),
                ..
            } => stroke.color == Color(218, 221, 224, 255),
            _ => false,
        })
        .count();
    assert_eq!(grid_count, 0);

    let semantic_runs: Vec<_> = first
        .result
        .display_list
        .items
        .iter()
        .filter_map(|item| match item {
            DisplayItem::GlyphRun(run) if run.source == NodeId(3) || run.source == NodeId(4) => {
                Some((
                    run.source,
                    run.label.normalized_text(),
                    run.rotation_degrees,
                ))
            }
            _ => None,
        })
        .collect();
    assert!(semantic_runs.iter().any(|(source, text, rotation)| {
        *source == NodeId(3) && text.contains("μ0HDL") && *rotation == 0.0
    }));
    assert!(semantic_runs.iter().any(|(source, text, rotation)| {
        *source == NodeId(4) && text.contains("Current density") && *rotation == -90.0
    }));
}

#[test]
fn formal_axes_layout_honors_ranges_locators_and_formatters() {
    let mut project = ProjectDocument::fixed_fixture();
    let axes = &mut project.figure.axes[0];
    axes.x.minimum = -1.0;
    axes.x.maximum = 1.0;
    axes.x.locator = LocatorSpec::Fixed {
        values: vec![-1.0, 0.0, 1.0],
    };
    axes.x.formatter = FormatterSpec::Decimal { precision: 2 };
    let document = FigureDocument::from_project(project).unwrap();
    let output = document.layout_axes().unwrap();

    assert_eq!(
        output
            .result
            .x_axis
            .major
            .iter()
            .map(|tick| (tick.value, tick.label.as_str()))
            .collect::<Vec<_>>(),
        vec![(-1.0, "−1"), (0.0, "0"), (1.0, "1")]
    );
}

#[test]
fn non_numeric_project_ids_have_a_stable_reverse_mapping() {
    let mut project = ProjectDocument::fixed_fixture();
    project.figure.axes[0].id = "axes-primary".to_owned();
    project.figure.axes[0].x.id = "axis-horizontal".to_owned();
    project.figure.axes[0].y.id = "axis-vertical".to_owned();
    let document = FigureDocument::from_project(project).unwrap();
    let first = document.layout_axes().unwrap();
    let second = document.layout_axes().unwrap();

    assert_eq!(first.project_ids, second.project_ids);
    assert!(first.project_ids.values().any(|id| id == "axes-primary"));
    assert!(first.project_ids.values().any(|id| id == "axis-horizontal"));
    assert!(first.project_ids.values().any(|id| id == "axis-vertical"));
}

#[test]
fn formal_figure_layout_resolves_basic_line_and_scatter_artists() {
    let output = FigureDocument::fixed().layout_figure().unwrap();

    assert_eq!(output.project_ids.get(&NodeId(11)).unwrap(), "node-11");
    assert_eq!(output.project_ids.get(&NodeId(13)).unwrap(), "node-13");
    assert_eq!(output.project_ids.get(&NodeId(12)).unwrap(), "node-12");
    assert!(
        output
            .result
            .hit_map
            .items
            .iter()
            .any(|item| { item.node == NodeId(11) && item.role == SelectableRole::Series })
    );
    assert_eq!(
        output
            .result
            .hit_map
            .items
            .iter()
            .filter(|item| { item.node == NodeId(13) && item.role == SelectableRole::DataPoint })
            .count(),
        3
    );
    assert!(output.result.display_list.validation_errors().is_empty());
}

#[test]
fn matching_line_and_scatter_bindings_form_a_combined_visual_series() {
    let mut project = ProjectDocument::fixed_fixture();
    let scatter = project
        .figure
        .artists
        .iter_mut()
        .find(|artist| artist.id == "node-13")
        .unwrap();
    let ArtistProperties::Scatter { binding, .. } = &mut scatter.properties else {
        panic!("fixed node-13 must remain a scatter artist")
    };
    binding.x_column = "line_x".to_owned();
    binding.y_column = "line_y".to_owned();

    let output = FigureDocument::from_project(project)
        .unwrap()
        .layout_figure()
        .unwrap();
    let line_points = output
        .result
        .hit_map
        .items
        .iter()
        .find(|item| item.node == NodeId(11) && item.role == SelectableRole::Series)
        .unwrap()
        .path_proximity
        .clone();
    let marker_points: Vec<_> = output
        .result
        .hit_map
        .items
        .iter()
        .filter(|item| item.node == NodeId(13) && item.role == SelectableRole::DataPoint)
        .flat_map(|item| item.path_proximity.iter().copied())
        .collect();
    assert_eq!(line_points, marker_points);
}

#[test]
fn marker_interval_skips_only_markers_and_keeps_the_full_line() {
    let mut document = FigureDocument::fixed();
    let mut record = document.artist_record("node-13").unwrap();
    let ArtistProperties::Scatter { marker, .. } = &mut record.properties else {
        panic!("node-13 must remain a scatter artist")
    };
    marker.interval = 2;
    document.set_artist_record(record).unwrap();
    let output = document.layout_figure().unwrap();
    assert_eq!(
        output
            .result
            .hit_map
            .items
            .iter()
            .filter(|item| item.node == NodeId(13) && item.role == SelectableRole::DataPoint)
            .count(),
        2
    );
    assert_eq!(
        output
            .result
            .hit_map
            .items
            .iter()
            .find(|item| item.node == NodeId(11) && item.role == SelectableRole::Series)
            .unwrap()
            .path_proximity
            .len(),
        3
    );
}

#[test]
fn separately_imported_sources_receive_distinct_default_markers() {
    let dataset = |id: &str| DataSet {
        source: PathBuf::from(format!("{id}.csv")),
        label: Some(id.to_owned()),
        kind: DataSetKind::Source,
        plot_id: id.to_owned(),
        fit_link: None,
        encoding: "UTF-8".to_owned(),
        separator: "comma".to_owned(),
        columns: vec![
            NumericColumn {
                name: "x".to_owned(),
                values: vec![0.0, 1.0],
            },
            NumericColumn {
                name: "y".to_owned(),
                values: vec![1.0, 2.0],
            },
        ],
        row_count: 2,
        alive: vec![true; 2],
    };
    let mut document = FigureDocument::fixed();
    for id in ["later-a", "later-b"] {
        document.sync_datasets(&[dataset(id)]).unwrap();
        document
            .create_series(id, "x", "y", SeriesCreationStyle::Scatter)
            .unwrap();
    }
    let shapes = ["later-a", "later-b"].map(|id| {
        document
            .project()
            .figure
            .artists
            .iter()
            .find_map(|artist| match &artist.properties {
                ArtistProperties::Scatter { binding, marker } if binding.data_source_id == id => {
                    Some(marker.shape)
                }
                _ => None,
            })
            .unwrap()
    });
    assert_ne!(shapes[0], shapes[1]);
}

#[test]
fn marker_density_can_be_applied_atomically_to_all_scatter_series() {
    let mut document = FigureDocument::showcase();
    document.set_all_marker_density(3.25, 7).unwrap();
    let markers = document
        .project()
        .figure
        .artists
        .iter()
        .filter_map(|artist| match &artist.properties {
            ArtistProperties::Scatter { marker, .. } => Some(marker),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(!markers.is_empty());
    assert!(
        markers
            .iter()
            .all(|marker| marker.size_pt == 3.25 && marker.interval == 7)
    );
}

#[test]
fn marker_size_and_interval_can_be_applied_independently() {
    let mut document = FigureDocument::showcase();
    document.set_all_marker_density(3.25, 7).unwrap();
    document.set_all_marker_sizes(5.5).unwrap();
    let markers = document
        .project()
        .figure
        .artists
        .iter()
        .filter_map(|artist| match &artist.properties {
            ArtistProperties::Scatter { marker, .. } => Some(marker),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(markers.iter().all(|marker| marker.size_pt == 5.5));
    assert!(markers.iter().all(|marker| marker.interval == 7));
    document.set_all_marker_intervals(11).unwrap();
    let markers = document
        .project()
        .figure
        .artists
        .iter()
        .filter_map(|artist| match &artist.properties {
            ArtistProperties::Scatter { marker, .. } => Some(marker),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(markers.iter().all(|marker| marker.size_pt == 5.5));
    assert!(markers.iter().all(|marker| marker.interval == 11));
}

#[test]
fn marker_fill_can_be_applied_to_all_without_changing_each_shape() {
    let mut document = FigureDocument::showcase();
    let before = document
        .project()
        .figure
        .artists
        .iter()
        .filter_map(|artist| match &artist.properties {
            ArtistProperties::Scatter { marker, .. } => Some((
                artist.id.clone(),
                marker.shape,
                marker.color_id.clone(),
                marker.size_pt,
                marker.interval,
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(before.len() > 1);

    document.set_all_marker_filled(false).unwrap();

    let after = document
        .project()
        .figure
        .artists
        .iter()
        .filter_map(|artist| match &artist.properties {
            ArtistProperties::Scatter { marker, .. } => {
                assert!(!marker.filled);
                Some((
                    artist.id.clone(),
                    marker.shape,
                    marker.color_id.clone(),
                    marker.size_pt,
                    marker.interval,
                ))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(after, before);
}

#[test]
fn series_plot_type_switch_preserves_binding_legend_and_error_bars() {
    let dataset = DataSet {
        source: PathBuf::from("style-switch.csv"),
        label: Some("style-switch.csv".to_owned()),
        kind: DataSetKind::Source,
        plot_id: "style-switch".to_owned(),
        fit_link: None,
        encoding: "UTF-8".to_owned(),
        separator: "comma".to_owned(),
        columns: vec![
            NumericColumn {
                name: "x".to_owned(),
                values: vec![0.0, 1.0, 2.0],
            },
            NumericColumn {
                name: "y".to_owned(),
                values: vec![1.0, 2.0, 4.0],
            },
            NumericColumn {
                name: "error".to_owned(),
                values: vec![0.1, 0.2, 0.3],
            },
        ],
        row_count: 3,
        alive: vec![true; 3],
    };
    let mut document = FigureDocument::fixed();
    document.sync_datasets(&[dataset]).unwrap();
    let series_id = document
        .create_series("style-switch", "x", "y", SeriesCreationStyle::Scatter)
        .unwrap()
        .remove(0);
    let error_id = document
        .create_error_bars("style-switch", "x", "y", "error", None)
        .unwrap();
    document
        .set_series_legend_label(
            &series_id,
            vec![LabelNode::VariableSubscript(vec![LabelNode::Text(
                "switch".to_owned(),
            )])],
        )
        .unwrap();

    document
        .set_series_style(&series_id, SeriesCreationStyle::Line)
        .unwrap();
    assert_eq!(
        document.series_style(&series_id),
        Some(SeriesCreationStyle::Line)
    );
    assert!(document.artist_record(&error_id).is_some());

    document
        .set_series_style(&series_id, SeriesCreationStyle::LineAndMarker)
        .unwrap();
    assert_eq!(
        document.series_style(&series_id),
        Some(SeriesCreationStyle::LineAndMarker)
    );
    let bound_visuals = document
        .project()
        .figure
        .artists
        .iter()
        .filter(|artist| {
            matches!(artist.kind, ArtistKind::Line | ArtistKind::Scatter)
                && artist_binding(artist).is_some_and(|binding| {
                    binding.data_source_id == "style-switch"
                        && binding.x_column == "x"
                        && binding.y_column == "y"
                })
        })
        .count();
    assert_eq!(bound_visuals, 2);
    assert!(document.artist_record(&error_id).is_some());

    document
        .set_series_style(&series_id, SeriesCreationStyle::Scatter)
        .unwrap();
    assert_eq!(
        document.series_style(&series_id),
        Some(SeriesCreationStyle::Scatter)
    );
    assert!(document.artist_record(&error_id).is_some());
    document.project().validate().unwrap();
    document.layout_figure().unwrap();
    let encoded = serde_json::to_vec(document.project()).unwrap();
    let reopened =
        FigureDocument::from_project(crate::project::decode_project(&encoded).unwrap()).unwrap();
    assert_eq!(reopened.project(), document.project());
}

#[test]
fn legend_preserves_semantic_label_styles_in_the_display_list() {
    let mut document = FigureDocument::fixed();
    let (artist_id, label_id) = document
        .project()
        .figure
        .artists
        .iter()
        .find_map(|artist| match &artist.properties {
            ArtistProperties::Legend { entries, .. } => entries
                .first()
                .map(|entry| (entry.artist_id.clone(), entry.label_id.clone())),
            _ => None,
        })
        .unwrap();
    document
        .set_semantic_label_nodes(
            &label_id,
            vec![
                LabelNode::Variable("I".to_owned()),
                LabelNode::VariableSubscript(vec![LabelNode::Variable("c".to_owned())]),
            ],
        )
        .unwrap();
    let layout = document.layout_figure().unwrap();
    let node = layout
        .project_ids
        .iter()
        .find_map(|(node, id)| (id == &artist_id).then_some(*node))
        .unwrap();
    let legend_run = layout
        .result
        .display_list
        .items
        .iter()
        .find_map(|item| match item {
            DisplayItem::GlyphRun(run)
                if run.source == node && run.label.normalized_text() == "Ic" =>
            {
                Some(run)
            }
            _ => None,
        })
        .unwrap();
    let spans = legend_run.label.spans();
    assert_eq!(spans[0].style, instplot_text::Style::Italic);
    assert!(spans[1].baseline_shift_em > 0.0);
}

#[test]
fn switching_palette_recolors_series_and_keeps_the_document_valid() {
    for (palette_id, expected_colors) in [
        ("tol-bright-v1", 7),
        ("tol-high-contrast-v1", 3),
        ("okabe-ito-v1", 7),
        ("batlow-v1", 7),
        ("viridis-v1", 7),
        ("cividis-v1", 7),
        ("tol-burd-v1", 7),
        ("sciplot-neutral-v1", 2),
    ] {
        let mut document = FigureDocument::showcase();
        document.set_palette(palette_id).unwrap();

        assert_eq!(document.palette_id(), palette_id);
        let available = document
            .palette_colors()
            .iter()
            .map(|color| color.id.as_str())
            .collect::<BTreeSet<_>>();
        let series_colors = document
            .project()
            .figure
            .artists
            .iter()
            .filter_map(|artist| match &artist.properties {
                ArtistProperties::Line { stroke, .. }
                | ArtistProperties::ErrorBar { stroke, .. } => Some(stroke.color_id.as_str()),
                ArtistProperties::Scatter { marker, .. } => Some(marker.color_id.as_str()),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(series_colors.len(), expected_colors, "{palette_id}");
        assert!(series_colors.iter().all(|color| available.contains(color)));
        for index in 0..7 {
            let line_id = format!("node-{}", 16 + index * 2);
            let marker_id = format!("node-{}", 17 + index * 2);
            let line = document.artist_record(&line_id).unwrap();
            let marker = document.artist_record(&marker_id).unwrap();
            let ArtistProperties::Line { stroke, .. } = line.properties else {
                panic!("{line_id} must be a line")
            };
            let ArtistProperties::Scatter { marker, .. } = marker.properties else {
                panic!("{marker_id} must be a marker series")
            };
            assert_eq!(
                stroke.color_id, marker.color_id,
                "{palette_id} series index {index}"
            );
        }
        document.project().validate().unwrap();
        document.layout_figure().unwrap();
    }
}

#[test]
fn series_management_is_valid_deterministic_and_round_trips() {
    let mut document = FigureDocument::fixed();
    let created = document
        .create_series(
            "fixture-data",
            "line_x",
            "line_y",
            SeriesCreationStyle::LineAndMarker,
        )
        .unwrap();
    assert_eq!(created, ["series-1", "series-2"]);
    document.project().validate().unwrap();
    document.layout_figure().unwrap();

    document.set_series_visible(&created[0], false).unwrap();
    assert!(
        !document
            .series()
            .iter()
            .find(|item| item.id == created[0])
            .unwrap()
            .visible
    );
    let duplicate = document.duplicate_series(&created[1]).unwrap();
    assert_eq!(duplicate, "series-3");
    document
        .move_series(&duplicate, MoveDirection::Earlier)
        .unwrap();
    document
        .rebind_series(&duplicate, "fixture-data", "scatter_x", "scatter_y", None)
        .unwrap();
    document.delete_series(&created[1]).unwrap();
    document.project().validate().unwrap();

    let encoded = serde_json::to_vec(document.project()).unwrap();
    let reopened =
        FigureDocument::from_project(crate::project::decode_project(&encoded).unwrap()).unwrap();
    assert_eq!(reopened.project(), document.project());
    assert!(
        !reopened
            .series()
            .iter()
            .find(|item| item.id == created[0])
            .unwrap()
            .visible
    );
}

#[test]
fn rebind_updates_only_automatic_axis_labels() {
    let mut document = FigureDocument::fixed();
    let created = document
        .create_series(
            "fixture-data",
            "line_x",
            "line_y",
            SeriesCreationStyle::Line,
        )
        .unwrap();
    document
        .set_axis_label(
            AxisDimension::X,
            vec![LabelNode::Variable("line_x".to_owned())],
        )
        .unwrap();
    document
        .set_axis_label(
            AxisDimension::Y,
            vec![LabelNode::Text("Measured signal".to_owned())],
        )
        .unwrap();

    document
        .rebind_series(&created[0], "fixture-data", "scatter_x", "scatter_y", None)
        .unwrap();

    assert_eq!(
        document.axis_label(AxisDimension::X),
        [LabelNode::Text("scatter_x".to_owned())]
    );
    assert_eq!(
        document.axis_label(AxisDimension::Y),
        [LabelNode::Text("Measured signal".to_owned())]
    );
}

#[test]
fn data_source_deletion_requires_explicit_cascade_and_cleans_dependencies() {
    let mut document = FigureDocument::fixed();
    let before = document.clone();
    assert!(document.delete_data_source("fixture-data", false).is_err());
    assert_eq!(document, before);

    document.delete_data_source("fixture-data", true).unwrap();
    assert!(document.project().data_sources.is_empty());
    assert!(document.project().figure.artists.iter().all(|artist| {
        !matches!(
            artist.kind,
            ArtistKind::Line | ArtistKind::Scatter | ArtistKind::ErrorBar | ArtistKind::Legend
        )
    }));
    document.project().validate().unwrap();
    document.layout_figure().unwrap();
}

#[test]
fn ordinary_import_is_embedded_before_series_creation() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("smoke.csv");
    let datasets = instplot_io::read_data_file(&fixture).unwrap();
    let imported_id = datasets[0].plot_id.clone();
    let mut document = FigureDocument::fixed();
    document.sync_datasets(&datasets).unwrap();
    let source = document
        .project()
        .data_sources
        .iter()
        .find(|source| source.id == imported_id)
        .unwrap();
    assert!(matches!(source.payload, DataSourcePayload::Embedded { .. }));
    document
        .create_series(
            &imported_id,
            "field",
            "response",
            SeriesCreationStyle::Scatter,
        )
        .unwrap();
    document.project().validate().unwrap();
    document.layout_figure().unwrap();
}

#[test]
fn formal_figure_layout_includes_remaining_single_axes_artists() {
    let output = FigureDocument::fixed().layout_figure().unwrap();
    let legend = output.result.legend.unwrap();
    assert!(legend.right() <= output.result.display_list.width.get());
    assert!(legend.bottom() <= output.result.display_list.height.get());
    let legend_text_order: Vec<_> = output
        .result
        .display_list
        .items
        .iter()
        .filter_map(|item| match item {
            DisplayItem::GlyphRun(run) if run.source == NodeId(13) || run.source == NodeId(11) => {
                Some(run.source)
            }
            _ => None,
        })
        .collect();
    assert_eq!(legend_text_order, vec![NodeId(13), NodeId(11)]);

    assert!(
        output
            .result
            .hit_map
            .items
            .iter()
            .any(|item| { item.node == NodeId(10) && item.role == SelectableRole::Series })
    );
    assert_eq!(
        output
            .result
            .hit_map
            .items
            .iter()
            .filter(|item| item.node == NodeId(12) && item.role == SelectableRole::ErrorBar)
            .count(),
        3
    );
    assert!(
        output
            .result
            .hit_map
            .items
            .iter()
            .any(|item| { item.node == NodeId(14) && item.role == SelectableRole::Annotation })
    );
    assert!(
        output
            .result
            .hit_map
            .items
            .iter()
            .any(|item| { item.node == NodeId(15) && item.role == SelectableRole::Legend })
    );
    assert_eq!(output.project_ids.get(&NodeId(14)).unwrap(), "node-14");
    assert_eq!(output.project_ids.get(&NodeId(15)).unwrap(), "node-15");

    let labels: Vec<_> = output
        .result
        .display_list
        .items
        .iter()
        .filter_map(|item| match item {
            DisplayItem::GlyphRun(run) => Some((run.source, run.label.normalized_text())),
            _ => None,
        })
        .collect();
    assert!(labels.contains(&(NodeId(14), "T ≤ 300 K".to_owned())));
    assert!(labels.contains(&(NodeId(13), "Experiment".to_owned())));
    assert!(labels.contains(&(NodeId(11), "Fit".to_owned())));
}

#[test]
fn formal_figure_layout_supports_positive_log_data() {
    let mut document = FigureDocument::fixed();
    let axes = &mut document.project.figure.axes[0];
    axes.x.minimum = 0.1;
    axes.x.maximum = 10.0;
    axes.x.scale = AxisScale::Log10;
    axes.y.minimum = 0.1;
    axes.y.maximum = 10.0;
    axes.y.scale = AxisScale::Log10;
    if let ArtistProperties::ReferenceLine { value, .. } =
        &mut document.project.figure.artists[0].properties
    {
        *value = 1.0;
    }
    let DataSourcePayload::Embedded { columns, .. } = &mut document.project.data_sources[0].payload
    else {
        unreachable!()
    };
    for column in columns {
        match column.name.as_str() {
            "line_x" | "scatter_x" => column.values = vec![0.1, 1.0, 10.0],
            "line_y" | "scatter_y" => column.values = vec![0.2, 2.0, 8.0],
            _ => {}
        }
    }

    let output = document.layout_figure().unwrap();
    assert_eq!(
        output
            .result
            .x_axis
            .major
            .iter()
            .map(|tick| tick.value)
            .collect::<Vec<_>>(),
        vec![0.1, 1.0, 10.0]
    );
    assert!(output.result.display_list.validation_errors().is_empty());
}
