use super::*;

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

fn add_secondary_dataset(document: &mut FigureDocument) -> String {
    document
        .project
        .upsert_embedded_source(
            "secondary-data",
            "Secondary data",
            vec![
                EmbeddedColumn {
                    name: "x2".to_owned(),
                    values: vec![100.0, 200.0],
                    valid: Vec::new(),
                },
                EmbeddedColumn {
                    name: "y2".to_owned(),
                    values: vec![1_000.0, 2_000.0],
                    valid: Vec::new(),
                },
                EmbeddedColumn {
                    name: "xe2".to_owned(),
                    values: vec![10.0, 20.0],
                    valid: Vec::new(),
                },
                EmbeddedColumn {
                    name: "ye2".to_owned(),
                    values: vec![100.0, 200.0],
                    valid: Vec::new(),
                },
            ],
            vec![true; 2],
            DataSourceKind::Source,
            None,
        )
        .unwrap();
    document
        .create_series(
            "secondary-data",
            "x2",
            "y2",
            SeriesCreationStyle::LineAndMarker,
        )
        .unwrap()[0]
        .clone()
}

#[test]
fn dual_y_autoscale_includes_error_caps_and_mode_switch_suspends_without_hiding() {
    let mut document = FigureDocument::fixed();
    let secondary = add_secondary_dataset(&mut document);
    let error = document
        .create_error_bars("secondary-data", "x2", "y2", "ye2", Some("xe2"))
        .unwrap();
    document.set_axis_mode(AxisMode::DualY).unwrap();
    document
        .set_series_axis_binding(
            &secondary,
            AxisBinding {
                x: XAxisSlot::X1,
                y: YAxisSlot::Y2,
            },
        )
        .unwrap();

    let x1 = compute_axis_data_bounds(
        document.project(),
        AxisIdentity::X1,
        AutoscalePolicy::default(),
    )
    .unwrap();
    let y1 = compute_axis_data_bounds(
        document.project(),
        AxisIdentity::Y1,
        AutoscalePolicy::default(),
    )
    .unwrap();
    let y2 = compute_axis_data_bounds(
        document.project(),
        AxisIdentity::Y2,
        AutoscalePolicy::default(),
    )
    .unwrap();
    assert!(x1.maximum >= 220.0 && x1.minimum <= -3.2);
    assert!(y1.maximum < 10.0);
    assert_eq!(
        y2,
        DataBounds {
            minimum: 900.0,
            maximum: 2_200.0
        }
    );
    assert!(document.project().artist_id_effectively_visible(&error));

    document.set_axis_mode(AxisMode::Single).unwrap();
    let descriptor = document
        .series()
        .into_iter()
        .find(|series| series.id == secondary)
        .unwrap();
    assert!(descriptor.visible);
    assert!(!descriptor.effective_visible);
    assert!(
        compute_axis_data_bounds(
            document.project(),
            AxisIdentity::Y2,
            AutoscalePolicy::default()
        )
        .is_err()
    );
    assert!(!document.project().artist_id_effectively_visible(&error));

    document.set_axis_mode(AxisMode::DualY).unwrap();
    assert!(document.project().artist_id_effectively_visible(&secondary));
    assert_eq!(
        document.series_axis_binding(&secondary).unwrap().y,
        YAxisSlot::Y2
    );
}

#[test]
fn dual_x_autoscale_is_independent_and_last_secondary_series_enters_empty_state() {
    let mut document = FigureDocument::fixed();
    let secondary = add_secondary_dataset(&mut document);
    let error = document
        .create_error_bars("secondary-data", "x2", "y2", "ye2", Some("xe2"))
        .unwrap();
    document.set_axis_mode(AxisMode::DualX).unwrap();
    document
        .set_series_axis_binding(
            &secondary,
            AxisBinding {
                x: XAxisSlot::X2,
                y: YAxisSlot::Y1,
            },
        )
        .unwrap();
    let x2 = compute_axis_data_bounds(
        document.project(),
        AxisIdentity::X2,
        AutoscalePolicy::default(),
    )
    .unwrap();
    assert_eq!(
        x2,
        DataBounds {
            minimum: 90.0,
            maximum: 220.0
        }
    );
    assert!(document.project().artist_id_effectively_visible(&error));
    assert!(
        document
            .axis_record_by_identity(AxisIdentity::X2)
            .unwrap()
            .minimum
            < 100.0
    );

    document.delete_data_source("secondary-data", true).unwrap();
    let x2_axis = document.axis_record_by_identity(AxisIdentity::X2).unwrap();
    assert_eq!((x2_axis.minimum, x2_axis.maximum), (0.0, 1.0));
    assert!(
        compute_axis_data_bounds(
            document.project(),
            AxisIdentity::X2,
            AutoscalePolicy::default()
        )
        .is_err()
    );
}

#[test]
fn user_hidden_secondary_series_stays_hidden_across_mode_switch_and_round_trip() {
    let mut document = FigureDocument::fixed();
    let secondary = add_secondary_dataset(&mut document);
    document.set_axis_mode(AxisMode::DualY).unwrap();
    document
        .set_series_axis_binding(
            &secondary,
            AxisBinding {
                x: XAxisSlot::X1,
                y: YAxisSlot::Y2,
            },
        )
        .unwrap();
    document.set_series_visible(&secondary, false).unwrap();
    document.set_axis_mode(AxisMode::Single).unwrap();

    let project_path = std::env::temp_dir().join(format!(
        "instplot-dual-axis-dormant-{}.instplot",
        std::process::id()
    ));
    document.save(&project_path).unwrap();
    let (mut reopened, _) = FigureDocument::open(&project_path).unwrap();
    std::fs::remove_file(project_path).unwrap();
    assert_eq!(reopened.project().figure.axes[0].mode, AxisMode::Single);
    assert_eq!(
        reopened.series_axis_binding(&secondary),
        Some(AxisBinding {
            x: XAxisSlot::X1,
            y: YAxisSlot::Y2,
        })
    );
    assert!(
        !reopened
            .series()
            .into_iter()
            .find(|series| series.id == secondary)
            .unwrap()
            .visible
    );

    reopened.set_axis_mode(AxisMode::DualY).unwrap();
    assert!(!reopened.project().artist_id_effectively_visible(&secondary));
}

#[test]
fn secondary_axis_record_and_label_are_editable_without_replacing_identity() {
    let mut document = FigureDocument::fixed();
    document.set_axis_mode(AxisMode::DualY).unwrap();
    let mut y2 = document.axis_record_by_identity(AxisIdentity::Y2).unwrap();
    let original_id = y2.id.clone();
    y2.autoscale = false;
    y2.minimum = -20.0;
    y2.maximum = 80.0;
    document
        .set_axis_record_by_identity(AxisIdentity::Y2, y2)
        .unwrap();
    document
        .set_axis_label_by_identity(
            AxisIdentity::Y2,
            vec![LabelNode::Text("Secondary response".to_owned())],
        )
        .unwrap();

    let saved = document.axis_record_by_identity(AxisIdentity::Y2).unwrap();
    assert_eq!(saved.id, original_id);
    assert_eq!((saved.minimum, saved.maximum), (-20.0, 80.0));
    assert_eq!(
        document.axis_label_by_identity(AxisIdentity::Y2),
        &[LabelNode::Text("Secondary response".to_owned())]
    );
}

#[test]
fn axis_master_visibility_hides_only_requested_ink_and_preserves_detail_settings() {
    let mut document = FigureDocument::fixed();
    let mut x = document.axis_record_by_identity(AxisIdentity::X1).unwrap();
    x.appearance.near_ticks = false;
    x.appearance.far_ticks = true;
    x.appearance.near_tick_labels = true;
    let detail = x.appearance.clone();
    document
        .set_axis_record_by_identity(AxisIdentity::X1, x)
        .unwrap();
    let visible_height = document
        .layout_figure()
        .unwrap()
        .result
        .display_list
        .height
        .get();
    let visible_export_height = crate::resolve_document_for_export(&document)
        .unwrap()
        .display
        .height;

    let mut visibility = document.axis_visibility(AxisIdentity::X1).unwrap();
    visibility.label = false;
    visibility.ticks = false;
    document
        .set_axis_visibility(AxisIdentity::X1, visibility)
        .unwrap();
    let hidden = document.layout_figure().unwrap();
    let x_id = document
        .axis_record_by_identity(AxisIdentity::X1)
        .unwrap()
        .id;
    let x_node = hidden
        .project_ids
        .iter()
        .find_map(|(node, id)| (id == &x_id).then_some(*node))
        .unwrap();
    assert_eq!(hidden.result.x_label_bounds.width, 0.0);
    assert_eq!(hidden.result.display_list.height.get(), visible_height);
    let hidden_export_height = crate::resolve_document_for_export(&document)
        .unwrap()
        .display
        .height;
    assert!(hidden_export_height < visible_export_height);
    assert!(
        !hidden
            .result
            .hit_map
            .items
            .iter()
            .any(|item| item.node == x_node && item.role == SelectableRole::AxisLabel)
    );

    let stored = document.axis_record_by_identity(AxisIdentity::X1).unwrap();
    assert_eq!(stored.appearance.near_ticks, detail.near_ticks);
    assert_eq!(stored.appearance.far_ticks, detail.far_ticks);
    assert_eq!(stored.appearance.near_tick_labels, detail.near_tick_labels);
    visibility.label = true;
    visibility.ticks = true;
    document
        .set_axis_visibility(AxisIdentity::X1, visibility)
        .unwrap();
    let restored = document.axis_record_by_identity(AxisIdentity::X1).unwrap();
    assert_eq!(restored.appearance.near_ticks, detail.near_ticks);
    assert_eq!(restored.appearance.far_ticks, detail.far_ticks);
}

#[test]
fn dual_x_and_dual_y_spine_colors_survive_mode_switches_independently() {
    let mut document = FigureDocument::fixed();
    document.set_axis_mode(AxisMode::DualY).unwrap();
    let mut y1 = document.axis_record_by_identity(AxisIdentity::Y1).unwrap();
    let mut y2 = document.axis_record_by_identity(AxisIdentity::Y2).unwrap();
    y1.appearance.spine_color_id = "blue".to_owned();
    y2.appearance.spine_color_id = "gray".to_owned();
    y2.autoscale = false;
    document
        .set_axis_record_by_identity(AxisIdentity::Y1, y1)
        .unwrap();
    document
        .set_axis_record_by_identity(AxisIdentity::Y2, y2)
        .unwrap();

    document.set_axis_mode(AxisMode::DualX).unwrap();
    let mut x1 = document.axis_record_by_identity(AxisIdentity::X1).unwrap();
    let mut x2 = document.axis_record_by_identity(AxisIdentity::X2).unwrap();
    x1.appearance.spine_color_id = "gray".to_owned();
    x2.appearance.spine_color_id = "blue".to_owned();
    x2.autoscale = false;
    document
        .set_axis_record_by_identity(AxisIdentity::X1, x1)
        .unwrap();
    document
        .set_axis_record_by_identity(AxisIdentity::X2, x2)
        .unwrap();

    document.set_axis_mode(AxisMode::DualY).unwrap();
    assert_eq!(
        document
            .axis_record_by_identity(AxisIdentity::Y1)
            .unwrap()
            .appearance
            .spine_color_id,
        "blue"
    );
    assert_eq!(
        document
            .axis_record_by_identity(AxisIdentity::Y2)
            .unwrap()
            .appearance
            .spine_color_id,
        "gray"
    );

    document.set_axis_mode(AxisMode::DualX).unwrap();
    assert_eq!(
        document
            .axis_record_by_identity(AxisIdentity::X1)
            .unwrap()
            .appearance
            .spine_color_id,
        "gray"
    );
    assert_eq!(
        document
            .axis_record_by_identity(AxisIdentity::X2)
            .unwrap()
            .appearance
            .spine_color_id,
        "blue"
    );

    document.set_axis_mode(AxisMode::Single).unwrap();
    let encoded = serde_json::to_vec(document.project()).unwrap();
    let reopened =
        FigureDocument::from_project(crate::project::decode_project(&encoded).unwrap()).unwrap();
    let layout = reopened.layout_figure().unwrap();
    let axis_ids = [
        document
            .axis_record_by_identity(AxisIdentity::X1)
            .unwrap()
            .id,
        document
            .axis_record_by_identity(AxisIdentity::Y1)
            .unwrap()
            .id,
    ];
    for id in axis_ids {
        let node = layout
            .project_ids
            .iter()
            .find_map(|(node, project_id)| (project_id == &id).then_some(*node))
            .unwrap();
        assert!(layout.result.display_list.items.iter().any(|item| matches!(
            item,
            DisplayItem::Path { source, stroke: Some(stroke), .. }
                if *source == node && stroke.color == Color(0, 0, 0, 255)
        )));
    }
}

#[test]
fn first_secondary_assignment_suggests_a_label_but_never_overwrites_user_text() {
    let mut document = FigureDocument::fixed();
    let secondary = add_secondary_dataset(&mut document);
    document.set_axis_mode(AxisMode::DualY).unwrap();
    document
        .set_series_axis_binding(
            &secondary,
            AxisBinding {
                x: XAxisSlot::X1,
                y: YAxisSlot::Y2,
            },
        )
        .unwrap();
    assert_eq!(
        document.axis_label_by_identity(AxisIdentity::Y2),
        &[LabelNode::Text("y2".to_owned())]
    );

    document
        .set_axis_label_by_identity(
            AxisIdentity::Y2,
            vec![LabelNode::Text("Custom unit".to_owned())],
        )
        .unwrap();
    document
        .set_series_axis_binding(&secondary, AxisBinding::PRIMARY)
        .unwrap();
    document
        .set_series_axis_binding(
            &secondary,
            AxisBinding {
                x: XAxisSlot::X1,
                y: YAxisSlot::Y2,
            },
        )
        .unwrap();
    assert_eq!(
        document.axis_label_by_identity(AxisIdentity::Y2),
        &[LabelNode::Text("Custom unit".to_owned())]
    );
}

#[test]
fn empty_secondary_axis_tracks_effectively_visible_series() {
    let mut document = FigureDocument::fixed();
    let secondary = add_secondary_dataset(&mut document);
    assert_eq!(document.empty_active_secondary_axis(), None);

    document.set_axis_mode(AxisMode::DualY).unwrap();
    assert_eq!(
        document.empty_active_secondary_axis(),
        Some(AxisIdentity::Y2)
    );

    document
        .set_series_axis_binding(
            &secondary,
            AxisBinding {
                x: XAxisSlot::X1,
                y: YAxisSlot::Y2,
            },
        )
        .unwrap();
    assert_eq!(document.empty_active_secondary_axis(), None);

    let secondary_members = document
        .project()
        .series_group_for_artist(&secondary)
        .unwrap()
        .artist_ids
        .clone();
    for artist_id in secondary_members {
        document.set_series_visible(&artist_id, false).unwrap();
    }
    assert_eq!(
        document.empty_active_secondary_axis(),
        Some(AxisIdentity::Y2)
    );

    document.set_axis_mode(AxisMode::Single).unwrap();
    assert_eq!(document.empty_active_secondary_axis(), None);
}

#[test]
fn formal_layout_maps_dual_axes_and_single_mode_removes_secondary_geometry() {
    let mut document = FigureDocument::fixed();
    let single = document.layout_figure().unwrap();
    let secondary = add_secondary_dataset(&mut document);
    document.set_axis_mode(AxisMode::DualY).unwrap();
    document
        .set_series_axis_binding(
            &secondary,
            AxisBinding {
                x: XAxisSlot::X1,
                y: YAxisSlot::Y2,
            },
        )
        .unwrap();
    let dual = document.layout_figure().unwrap();
    assert!(
        dual.result
            .y2_axis
            .as_ref()
            .is_some_and(|axis| !axis.major.is_empty())
    );
    assert!(dual.result.y2_label_bounds.is_some());
    assert!(dual.result.display_list.width.get() > single.result.display_list.width.get());
    assert!((dual.result.axes.width - single.result.axes.width).abs() < 0.05);
    assert!((dual.result.axes.height - single.result.axes.height).abs() < 0.05);
    let secondary_node = dual
        .project_ids
        .iter()
        .find_map(|(node, id)| (id == &secondary).then_some(*node))
        .unwrap();
    assert!(
        dual.result
            .hit_map
            .items
            .iter()
            .any(|item| item.node == secondary_node && item.role == SelectableRole::Series)
    );

    document.set_axis_mode(AxisMode::Single).unwrap();
    let restored = document.layout_figure().unwrap();
    assert!(restored.result.y2_axis.is_none());
    assert!(
        !restored
            .result
            .hit_map
            .items
            .iter()
            .any(|item| item.node == secondary_node)
    );
    assert_eq!(restored.result.axes, single.result.axes);
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
            axes: AxisBinding::PRIMARY,
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
            arrow_head: crate::project::ArrowHead::Open,
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

    let measurement_id = document
        .add_measurement_arrow(MeasurementArrowSpec {
            start: (-1.0, 1.0),
            end: (1.0, 1.0),
            axes: AxisBinding::PRIMARY,
            constraint: MeasurementConstraint::Horizontal,
            start_arrow: true,
            end_arrow: true,
            label_nodes: Some(vec![LabelNode::Text("Δt = 255 s\nfirst cycle".to_owned())]),
        })
        .unwrap();
    let encoded = serde_json::to_vec(document.project()).unwrap();
    let reopened =
        FigureDocument::from_project(crate::project::decode_project(&encoded).unwrap()).unwrap();
    let layout = reopened.layout_figure().unwrap();
    let measurement_node = layout
        .project_ids
        .iter()
        .find_map(|(node, id)| (id == &measurement_id).then_some(*node))
        .unwrap();
    assert_eq!(
        layout
            .result
            .display_list
            .items
            .iter()
            .filter(
                |item| matches!(item, DisplayItem::GlyphRun(run) if run.source == measurement_node)
            )
            .count(),
        2
    );
    let label_hit = layout
        .result
        .hit_map
        .items
        .iter()
        .find(|item| {
            item.node == measurement_node && item.role == SelectableRole::MeasurementArrowLabel
        })
        .unwrap();
    assert!(label_hit.bounds.height > 10.0);
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
fn adding_only_y_errors_invalidates_y_but_preserves_a_manual_x_axis() {
    let mut document = FigureDocument::from_datasets(&[error_dataset()]).unwrap();
    let series_id = document.logical_series().into_iter().next().unwrap().id;
    for identity in [AxisIdentity::X1, AxisIdentity::Y1] {
        let mut axis = document.axis_record_by_identity(identity).unwrap();
        axis.autoscale = false;
        axis.minimum = -0.01;
        axis.maximum = 0.01;
        axis.locator = LocatorSpec::Interval { step: 0.005 };
        document
            .set_axis_record_by_identity(identity, axis)
            .unwrap();
    }

    document
        .set_series_error_columns(&series_id, None, Some("ye"))
        .unwrap();

    let x = document.axis_record_by_identity(AxisIdentity::X1).unwrap();
    let y = document.axis_record_by_identity(AxisIdentity::Y1).unwrap();
    assert!(!x.autoscale);
    assert_eq!((x.minimum, x.maximum), (-0.01, 0.01));
    assert!(matches!(x.locator, LocatorSpec::Interval { .. }));
    assert!(y.autoscale);
    assert!(matches!(y.locator, LocatorSpec::Auto { .. }));
    assert!(y.minimum < 7.0 && y.maximum > 24.0, "{y:?}");
}

#[test]
fn moving_visible_data_to_secondary_axis_invalidates_old_and_new_axis_state() {
    let mut document = FigureDocument::fixed();
    let secondary = add_secondary_dataset(&mut document);
    document.set_axis_mode(AxisMode::DualY).unwrap();
    for identity in [AxisIdentity::Y1, AxisIdentity::Y2] {
        let mut axis = document.axis_record_by_identity(identity).unwrap();
        axis.autoscale = false;
        axis.minimum = -1.0;
        axis.maximum = 1.0;
        axis.locator = LocatorSpec::Interval { step: 0.5 };
        document
            .set_axis_record_by_identity(identity, axis)
            .unwrap();
    }

    document
        .set_series_axis_binding(
            &secondary,
            AxisBinding {
                x: XAxisSlot::X1,
                y: YAxisSlot::Y2,
            },
        )
        .unwrap();

    let y1 = document.axis_record_by_identity(AxisIdentity::Y1).unwrap();
    let y2 = document.axis_record_by_identity(AxisIdentity::Y2).unwrap();
    assert!(y1.autoscale && y2.autoscale);
    assert!(matches!(y1.locator, LocatorSpec::Auto { .. }));
    assert!(matches!(y2.locator, LocatorSpec::Auto { .. }));
    assert!(y2.minimum < 1_000.0 && y2.maximum > 2_000.0, "{y2:?}");
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
fn legend_key_matches_scatter_line_and_combined_series_styles() {
    let mut document = FigureDocument::from_datasets(&[error_dataset()]).unwrap();
    let series_id = document.logical_series()[0].id.clone();

    let legend_series = |document: &FigureDocument| {
        let axes = &document.project().figure.axes[0];
        let mut project_ids = BTreeMap::new();
        let (legend, labels) = legend_spec(axes, document.project(), &mut project_ids).unwrap();
        let legend_id = legend.unwrap().entry_order[0];
        formal_series(axes, document.project(), &labels, &mut project_ids)
            .unwrap()
            .0
            .into_iter()
            .find(|series| series.id == legend_id)
            .unwrap()
    };

    document
        .set_series_style(&series_id, SeriesCreationStyle::Scatter)
        .unwrap();
    let scatter = legend_series(&document);
    assert!(scatter.line.is_none());
    assert!(scatter.marker.is_some());
    assert!(scatter.legend_marker.is_none());

    document
        .set_series_style(&series_id, SeriesCreationStyle::Line)
        .unwrap();
    let line = legend_series(&document);
    assert!(line.line.is_some());
    assert!(line.marker.is_none());
    assert!(line.legend_marker.is_none());

    document
        .set_series_style(&series_id, SeriesCreationStyle::Scatter)
        .unwrap();
    document
        .set_series_style(&series_id, SeriesCreationStyle::LineAndMarker)
        .unwrap();
    let combined = legend_series(&document);
    assert!(combined.line.is_some());
    assert!(combined.marker.is_none());
    assert!(combined.legend_marker.is_some());
}

#[test]
fn logical_series_uses_one_color_for_line_marker_error_and_legend() {
    let mut document = FigureDocument::from_datasets(&[error_dataset()]).unwrap();
    let series_id = document.logical_series()[0].id.clone();
    document
        .set_series_style(&series_id, SeriesCreationStyle::LineAndMarker)
        .unwrap();
    document
        .set_series_error_columns(&series_id, Some("xe"), Some("ye"))
        .unwrap();
    let mut line = document
        .project()
        .series_group_for_artist(&series_id)
        .unwrap()
        .artist_ids
        .iter()
        .filter_map(|id| document.artist_record(id))
        .find(|artist| artist.kind == ArtistKind::Line)
        .unwrap();
    let ArtistProperties::Line { stroke, .. } = &mut line.properties else {
        unreachable!()
    };
    stroke.color_id = "object-purple".to_owned();
    document.set_artist_record(line).unwrap();

    let group = document
        .project()
        .series_group_for_artist(&series_id)
        .unwrap();
    let colors = group
        .artist_ids
        .iter()
        .filter_map(|id| document.artist_record(id))
        .filter_map(|artist| match artist.properties {
            ArtistProperties::Line { stroke, .. } | ArtistProperties::ErrorBar { stroke, .. } => {
                Some(stroke.color_id)
            }
            ArtistProperties::Scatter { marker, .. } => Some(marker.color_id),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(colors.len() >= 3);
    assert!(colors.iter().all(|color| color == "object-purple"));

    let axes = &document.project().figure.axes[0];
    let mut project_ids = BTreeMap::new();
    let (legend, labels) = legend_spec(axes, document.project(), &mut project_ids).unwrap();
    let legend_id = legend.unwrap().entry_order[0];
    let rendered = formal_series(axes, document.project(), &labels, &mut project_ids)
        .unwrap()
        .0
        .into_iter()
        .find(|series| series.id == legend_id)
        .unwrap();
    assert_eq!(
        rendered.color,
        palette_color("object-purple", &document.project().palette).unwrap()
    );
    assert!(rendered.legend_marker.is_some());
    assert!(rendered.legend_error.is_some());
}

#[test]
fn plot_type_conversion_preserves_the_selected_series_color() {
    let mut document = FigureDocument::from_datasets(&[error_dataset()]).unwrap();
    let series_id = document.logical_series()[0].id.clone();
    document
        .set_series_style(&series_id, SeriesCreationStyle::Scatter)
        .unwrap();
    document
        .set_series_color(&series_id, "object-purple")
        .unwrap();
    document
        .set_series_style(&series_id, SeriesCreationStyle::LineAndMarker)
        .unwrap();

    let group = document
        .project()
        .series_group_for_artist(&series_id)
        .unwrap();
    for artist_id in &group.artist_ids {
        let artist = document.artist_record(artist_id).unwrap();
        let color = match artist.properties {
            ArtistProperties::Line { stroke, .. } | ArtistProperties::ErrorBar { stroke, .. } => {
                stroke.color_id
            }
            ArtistProperties::Scatter { marker, .. } => marker.color_id,
            _ => continue,
        };
        assert_eq!(color, "object-purple");
    }
}

#[test]
fn opening_an_inconsistent_legacy_series_normalizes_color_with_provenance() {
    let mut document = FigureDocument::from_datasets(&[error_dataset()]).unwrap();
    let series_id = document.logical_series()[0].id.clone();
    document
        .set_series_style(&series_id, SeriesCreationStyle::LineAndMarker)
        .unwrap();
    let ids = document
        .project
        .series_group_for_artist(&series_id)
        .unwrap()
        .artist_ids
        .clone();
    let palette_colors = palette_series_color_ids(&document.project.palette.id)
        .iter()
        .filter(|id| {
            document
                .project
                .palette
                .colors
                .iter()
                .any(|color| color.id == **id)
        })
        .take(2)
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(palette_colors.len(), 2);
    let mut assigned = 0;
    for artist in &mut document.project.figure.artists {
        if !ids.contains(&artist.id) {
            continue;
        }
        let color = if artist.id == ids[0] {
            palette_colors[0]
        } else {
            palette_colors[1]
        };
        match &mut artist.properties {
            ArtistProperties::Line { stroke, .. } => {
                stroke.color_id = color.to_owned();
                assigned += 1;
            }
            ArtistProperties::Scatter { marker, .. } => {
                marker.color_id = color.to_owned();
                assigned += 1;
            }
            _ => {}
        }
    }
    assert_eq!(assigned, 2);

    let reopened = FigureDocument::from_project(document.project.clone()).unwrap();
    let colors =
        ids.iter()
            .filter_map(|id| reopened.artist_record(id))
            .filter_map(|artist| match artist.properties {
                ArtistProperties::Line { stroke, .. }
                | ArtistProperties::ErrorBar { stroke, .. } => Some(stroke.color_id),
                ArtistProperties::Scatter { marker, .. } => Some(marker.color_id),
                _ => None,
            })
            .collect::<Vec<_>>();
    assert!(colors.iter().all(|color| color == palette_colors[0]));
    assert!(reopened.project().provenance.iter().any(|record| {
        record.operation == "normalize-logical-series-color"
            && record
                .input_ids
                .iter()
                .any(|id| id.starts_with("series-group"))
    }));
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
    let x_appearance = document.project().figure.axes[0].x.appearance.clone();
    let y_appearance = document.project().figure.axes[0].y.appearance.clone();
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
        &document.project().palette,
        &mut BTreeMap::new(),
        true,
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
fn rebinding_to_a_small_value_column_refreshes_automatic_axes() {
    let mut document = FigureDocument::fixed();
    let fixture_sources = document
        .project()
        .data_sources
        .iter()
        .map(|source| source.id.clone())
        .collect::<Vec<_>>();
    document.delete_data_sources(&fixture_sources).unwrap();
    document
        .project
        .upsert_embedded_source(
            "rebind-scale-data",
            "Rebind scale data",
            vec![
                EmbeddedColumn {
                    name: "x".to_owned(),
                    values: vec![0.0, 1.0, 2.0],
                    valid: Vec::new(),
                },
                EmbeddedColumn {
                    name: "large".to_owned(),
                    values: vec![0.0, 50.0, 100.0],
                    valid: Vec::new(),
                },
                EmbeddedColumn {
                    name: "small".to_owned(),
                    values: vec![-0.18, -0.02, 0.07],
                    valid: Vec::new(),
                },
            ],
            vec![true; 3],
            DataSourceKind::Source,
            None,
        )
        .unwrap();
    let series = document
        .create_series(
            "rebind-scale-data",
            "x",
            "large",
            SeriesCreationStyle::Scatter,
        )
        .unwrap()[0]
        .clone();
    let mut x = document.axis_record(AxisDimension::X);
    x.autoscale = false;
    x.minimum = -1.0;
    x.maximum = 1.0;
    x.locator = LocatorSpec::Interval { step: 0.25 };
    document.set_axis_record(AxisDimension::X, x).unwrap();
    let mut y = document.axis_record(AxisDimension::Y);
    y.autoscale = false;
    y.minimum = -1.0;
    y.maximum = 1.0;
    y.locator = LocatorSpec::Interval { step: 0.25 };
    y.minor_interval = Some(0.05);
    y.formatter = FormatterSpec::Decimal { precision: 1 };
    document.set_axis_record(AxisDimension::Y, y).unwrap();

    document
        .rebind_series(&series, "rebind-scale-data", "x", "small", None)
        .unwrap();

    let x = document.axis_record(AxisDimension::X);
    let y = document.axis_record(AxisDimension::Y);
    assert!(
        !x.autoscale,
        "unchanged X binding must preserve its manual axis"
    );
    assert!(y.autoscale);
    assert!(matches!(y.locator, LocatorSpec::Auto { .. }));
    assert_eq!(y.minor_interval, None);
    assert_eq!(y.formatter, FormatterSpec::Auto);
    assert!(y.minimum < -0.18, "{y:?}");
    assert!(y.maximum > 0.07 && y.maximum < 1.0, "{y:?}");
}

#[test]
fn axis_display_factor_is_part_of_the_semantic_axis_label() {
    let mut document = FigureDocument::fixed();
    document
        .set_axis_ranges(AxisRanges {
            x_min: -2.0,
            x_max: 2.0,
            y_min: 0.001,
            y_max: 0.009,
        })
        .unwrap();
    document
        .set_axis_label(
            AxisDimension::Y,
            vec![
                LabelNode::Text("R (".to_owned()),
                LabelNode::ScaleFactorSlot,
                LabelNode::Text(" Ω)".to_owned()),
            ],
        )
        .unwrap();
    let layout = document.layout_figure().unwrap();
    let text = layout.result.y_axis.label.normalized_text();
    assert!(text.contains("×10−3"), "{text}");
    assert!(!layout.result.y_axis.major.is_empty());
    assert!(
        layout
            .result
            .y_axis
            .major
            .iter()
            .all(|tick| !tick.label.contains('e'))
    );

    let mut axis = document.axis_record(AxisDimension::Y);
    axis.display_scale = AxisDisplayScaleRecord::manual_incorporated(-3).unwrap();
    assert!(document.set_axis_record(AxisDimension::Y, axis).is_err());
}
