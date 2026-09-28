use super::*;

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
fn multiple_series_from_one_source_receive_distinct_default_markers() {
    let dataset = DataSet {
        source: PathBuf::from("multi-series.csv"),
        label: Some("multi-series".to_owned()),
        kind: DataSetKind::Source,
        plot_id: "multi-series".to_owned(),
        fit_link: None,
        encoding: "UTF-8".to_owned(),
        separator: "comma".to_owned(),
        columns: vec![
            NumericColumn {
                name: "x".to_owned(),
                values: vec![0.0, 1.0],
            },
            NumericColumn {
                name: "y1".to_owned(),
                values: vec![1.0, 2.0],
            },
            NumericColumn {
                name: "y2".to_owned(),
                values: vec![2.0, 3.0],
            },
        ],
        row_count: 2,
        alive: vec![true; 2],
    };
    let mut document = FigureDocument::fixed();
    document.sync_datasets(&[dataset]).unwrap();
    for y in ["y1", "y2"] {
        document
            .create_series("multi-series", "x", y, SeriesCreationStyle::Scatter)
            .unwrap();
    }
    let shapes = document
        .project()
        .figure
        .artists
        .iter()
        .filter_map(|artist| match &artist.properties {
            ArtistProperties::Scatter { binding, marker }
                if binding.data_source_id == "multi-series" =>
            {
                Some(marker.shape)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(shapes.len(), 2);
    assert_ne!(shapes[0], shapes[1]);
}

#[test]
fn line_marker_and_error_bar_remain_one_logical_series_entry() {
    let dataset = DataSet {
        source: PathBuf::from("logical-series.csv"),
        label: Some("logical-series".to_owned()),
        kind: DataSetKind::Source,
        plot_id: "logical-series".to_owned(),
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
            NumericColumn {
                name: "error".to_owned(),
                values: vec![0.1, 0.2],
            },
        ],
        row_count: 2,
        alive: vec![true; 2],
    };
    let mut document = FigureDocument::fixed();
    document.sync_datasets(&[dataset]).unwrap();
    document
        .create_series(
            "logical-series",
            "x",
            "y",
            SeriesCreationStyle::LineAndMarker,
        )
        .unwrap();
    document
        .create_error_bars("logical-series", "x", "y", "error", None)
        .unwrap();

    let logical = document
        .logical_series()
        .into_iter()
        .filter(|series| {
            series
                .binding
                .as_ref()
                .is_some_and(|binding| binding.data_source_id == "logical-series")
        })
        .collect::<Vec<_>>();
    assert_eq!(logical.len(), 1);
    assert_eq!(document.linked_series_ids(&logical[0].id).len(), 3);
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
fn neutral_series_guide_colors_migrate_by_index_without_losing_connector_style() {
    let mut document = FigureDocument::showcase();
    document.set_palette("sciplot-neutral-v1").unwrap();
    document.set_axis_mode(AxisMode::DualY).unwrap();
    let mut y2 = document.axis_record_by_identity(AxisIdentity::Y2).unwrap();
    y2.autoscale = false;
    y2.appearance.spine_color_id = "neutral-secondary".to_owned();
    document
        .set_axis_record_by_identity(AxisIdentity::Y2, y2)
        .unwrap();

    let reference_id = document
        .add_reference_line(ReferenceOrientation::Vertical, 0.5, AxisBinding::PRIMARY)
        .unwrap();
    let mut reference = document.artist_record(&reference_id).unwrap();
    let ArtistProperties::ReferenceLine { stroke, .. } = &mut reference.properties else {
        unreachable!()
    };
    stroke.color_id = "neutral-primary".to_owned();
    document.set_artist_record(reference).unwrap();

    let annotation_id = document
        .add_annotation(vec![LabelNode::Text("note".to_owned())])
        .unwrap();
    let mut annotation = document.artist_record(&annotation_id).unwrap();
    let ArtistProperties::Annotation { connectors, .. } = &mut annotation.properties else {
        unreachable!()
    };
    connectors.push(crate::AnnotationConnectorRecord {
        target_x: 0.0,
        target_y: 0.0,
        axes: AxisBinding::PRIMARY,
        stroke: StrokeStyle {
            color_id: "neutral-secondary".to_owned(),
            width_pt: 0.9,
            dash_pt: Vec::new(),
        },
        start_arrow: false,
        end_arrow: true,
        arrow_head: crate::ArrowHead::Open,
        arrow_size_pt: 5.0,
    });
    document.set_artist_record(annotation).unwrap();

    document.set_palette("viridis-v1").unwrap();
    assert_eq!(
        document
            .axis_record_by_identity(AxisIdentity::Y2)
            .unwrap()
            .appearance
            .spine_color_id,
        "viridis-2"
    );
    let ArtistProperties::ReferenceLine { stroke, .. } =
        document.artist_record(&reference_id).unwrap().properties
    else {
        unreachable!()
    };
    assert_eq!(stroke.color_id, "viridis-1");
    let ArtistProperties::Annotation { connectors, .. } =
        document.artist_record(&annotation_id).unwrap().properties
    else {
        unreachable!()
    };
    assert_eq!(connectors[0].stroke.color_id, "viridis-2");
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
