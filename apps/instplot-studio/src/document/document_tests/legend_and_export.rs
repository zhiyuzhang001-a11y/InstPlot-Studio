use super::*;

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
        let before = document.layout_figure().unwrap().result.legend.unwrap();
        let encoded = serde_json::to_vec(document.project()).unwrap();
        let reopened =
            FigureDocument::from_project(crate::project::decode_project(&encoded).unwrap())
                .unwrap();
        let result = reopened.layout_figure().unwrap().result;
        let bounds = result.legend.unwrap();
        assert_eq!(bounds, before);
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
