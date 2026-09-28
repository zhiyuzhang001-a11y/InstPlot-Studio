use super::*;

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
            .any(|item| { item.node == NodeId(10) && item.role == SelectableRole::ReferenceLine })
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
fn scientific_guides_are_not_logical_series_or_legend_entries() {
    let mut document = FigureDocument::fixed();
    let before_series = document.logical_series().len();
    let reference_id = document
        .add_reference_line(ReferenceOrientation::Vertical, 1.25, AxisBinding::PRIMARY)
        .unwrap();
    let arrow_id = document
        .add_measurement_arrow(MeasurementArrowSpec {
            start: (-1.0, 1.0),
            end: (1.0, 1.0),
            axes: AxisBinding::PRIMARY,
            constraint: MeasurementConstraint::Horizontal,
            start_arrow: true,
            end_arrow: true,
            label_nodes: Some(vec![LabelNode::Text("Δx".to_owned())]),
        })
        .unwrap();
    assert_eq!(document.logical_series().len(), before_series);
    assert!(document.project().figure.artists.iter().all(|artist| {
        let ArtistProperties::Legend { entries, .. } = &artist.properties else {
            return true;
        };
        entries
            .iter()
            .all(|entry| entry.artist_id != reference_id && entry.artist_id != arrow_id)
    }));
    let layout = document.layout_figure().unwrap();
    assert!(layout.result.hit_map.items.iter().any(|item| {
        layout.project_ids.get(&item.node).map(String::as_str) == Some(reference_id.as_str())
            && item.role == SelectableRole::ReferenceLine
    }));
    assert!(layout.result.hit_map.items.iter().any(|item| {
        layout.project_ids.get(&item.node).map(String::as_str) == Some(arrow_id.as_str())
            && item.role == SelectableRole::MeasurementArrow
    }));
}

#[test]
fn one_hundred_reference_lines_resolve_with_independent_hit_targets() {
    let mut document = FigureDocument::fixed();
    let mut ids = Vec::new();
    for index in 0..100 {
        ids.push(
            document
                .add_reference_line(
                    if index % 2 == 0 {
                        ReferenceOrientation::Vertical
                    } else {
                        ReferenceOrientation::Horizontal
                    },
                    -2.0 + index as f64 * 0.04,
                    AxisBinding::PRIMARY,
                )
                .unwrap(),
        );
    }

    let layout = document.layout_figure().unwrap();
    let hit_ids = layout
        .result
        .hit_map
        .items
        .iter()
        .filter(|item| item.role == SelectableRole::ReferenceLine)
        .filter_map(|item| layout.project_ids.get(&item.node))
        .filter(|id| ids.contains(id))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(hit_ids.len(), 100);
    assert!(layout.result.display_list.validation_errors().is_empty());
}

#[test]
fn guide_geometry_reaches_pdf_svg_and_png_from_one_resolved_scene() {
    let mut document = FigureDocument::fixed();
    let reference_id = document
        .add_reference_line(ReferenceOrientation::Vertical, 0.75, AxisBinding::PRIMARY)
        .unwrap();
    let arrow_id = document
        .add_measurement_arrow(MeasurementArrowSpec {
            start: (-1.0, 0.8),
            end: (1.0, 0.8),
            axes: AxisBinding::PRIMARY,
            constraint: MeasurementConstraint::Horizontal,
            start_arrow: true,
            end_arrow: true,
            label_nodes: Some(vec![LabelNode::Text("Δx = 2".to_owned())]),
        })
        .unwrap();

    let layout = document.layout_figure().unwrap();
    for id in [&reference_id, &arrow_id] {
        let node = layout
            .project_ids
            .iter()
            .find_map(|(node, project_id)| (project_id == id).then_some(*node))
            .unwrap();
        assert!(
            layout.result.display_list.items.iter().any(|item| {
                matches!(item, DisplayItem::Path { source, .. } if *source == node)
            })
        );
    }

    let pdf = crate::figure_pdf(&document).unwrap();
    let svg = crate::figure_svg(&document).unwrap();
    let png = crate::figure_png(&document, 300).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
    assert!(svg.starts_with(b"<svg"));
    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
}

#[test]
fn reference_line_autoscale_is_explicit_and_orientation_specific() {
    let mut document = FigureDocument::fixed();
    let mut x = document.axis_record(AxisDimension::X);
    x.autoscale = true;
    document.set_axis_record(AxisDimension::X, x).unwrap();
    let reference_id = document
        .add_reference_line(ReferenceOrientation::Vertical, 100.0, AxisBinding::PRIMARY)
        .unwrap();
    document.refresh_autoscale().unwrap();
    assert!(document.axis_record(AxisDimension::X).maximum < 100.0);

    let mut reference = document.artist_record(&reference_id).unwrap();
    let ArtistProperties::ReferenceLine {
        include_in_autoscale,
        ..
    } = &mut reference.properties
    else {
        unreachable!()
    };
    *include_in_autoscale = true;
    document.set_artist_record(reference).unwrap();
    document.refresh_autoscale().unwrap();
    assert!(document.axis_record(AxisDimension::X).maximum > 100.0);
    assert!(document.axis_record(AxisDimension::Y).maximum < 100.0);

    let mut reference = document.artist_record(&reference_id).unwrap();
    let ArtistProperties::ReferenceLine {
        include_in_autoscale,
        ..
    } = &mut reference.properties
    else {
        unreachable!()
    };
    *include_in_autoscale = false;
    document.set_artist_record(reference).unwrap();
    document.refresh_autoscale().unwrap();
    assert!(document.axis_record(AxisDimension::X).maximum < 100.0);

    let mut reference = document.artist_record(&reference_id).unwrap();
    let ArtistProperties::ReferenceLine {
        include_in_autoscale,
        ..
    } = &mut reference.properties
    else {
        unreachable!()
    };
    *include_in_autoscale = true;
    document.set_artist_record(reference).unwrap();
    document.refresh_autoscale().unwrap();
    assert!(document.axis_record(AxisDimension::X).maximum > 100.0);
    document.delete_drawing_object(&reference_id).unwrap();
    assert!(document.axis_record(AxisDimension::X).maximum < 100.0);
}

#[test]
fn deleting_measurement_arrow_removes_only_its_owned_label() {
    let mut document = FigureDocument::fixed();
    let arrow_id = document
        .add_measurement_arrow(MeasurementArrowSpec {
            start: (-1.0, 0.0),
            end: (1.0, 0.0),
            axes: AxisBinding::PRIMARY,
            constraint: MeasurementConstraint::Horizontal,
            start_arrow: true,
            end_arrow: true,
            label_nodes: Some(vec![LabelNode::Text("Δt".to_owned())]),
        })
        .unwrap();
    let ArtistProperties::MeasurementArrow {
        label_id: Some(label_id),
        ..
    } = document.artist_record(&arrow_id).unwrap().properties
    else {
        unreachable!()
    };
    document.delete_drawing_object(&arrow_id).unwrap();
    assert!(document.artist_record(&arrow_id).is_none());
    assert!(document.semantic_label_nodes(&label_id).is_none());
    document.project().validate().unwrap();
}

#[test]
fn new_reference_lines_use_the_canonical_dashed_pattern() {
    let mut document = FigureDocument::fixed();
    let reference_id = document
        .add_reference_line(ReferenceOrientation::Vertical, 0.5, AxisBinding::PRIMARY)
        .unwrap();
    let ArtistProperties::ReferenceLine { stroke, .. } =
        document.artist_record(&reference_id).unwrap().properties
    else {
        unreachable!()
    };
    assert_eq!(stroke.dash_pt, DEFAULT_REFERENCE_DASH_PT);
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

#[test]
fn axis_identity_lookup_recognizes_axis_and_label_ids() {
    let mut document = FigureDocument::fixed();
    document.set_axis_mode(AxisMode::DualY).unwrap();
    let axes = &document.project().figure.axes[0];
    for (identity, axis) in [
        (AxisIdentity::X1, &axes.x),
        (AxisIdentity::Y1, &axes.y),
        (AxisIdentity::Y2, axes.y2.as_ref().unwrap()),
    ] {
        assert_eq!(
            document.axis_identity_for_project_id(&axis.id),
            Some(identity)
        );
        assert_eq!(
            document.axis_identity_for_project_id(&axis.label_id),
            Some(identity)
        );
    }
    assert_eq!(document.axis_identity_for_project_id("not-an-axis"), None);
}

#[test]
fn object_visibility_uses_the_bound_axis_ranges_and_log_domain() {
    let mut document = FigureDocument::fixed();
    document.set_axis_mode(AxisMode::DualY).unwrap();
    let mut y2 = document.axis_record_by_identity(AxisIdentity::Y2).unwrap();
    y2.minimum = 1.0;
    y2.maximum = 100.0;
    y2.autoscale = false;
    y2.scale = AxisScale::Log10;
    document
        .set_axis_record_by_identity(AxisIdentity::Y2, y2)
        .unwrap();
    let binding = AxisBinding {
        x: XAxisSlot::X1,
        y: YAxisSlot::Y2,
    };

    assert!(document.reference_value_is_visible(ReferenceOrientation::Horizontal, binding, 10.0));
    assert!(!document.reference_value_is_visible(ReferenceOrientation::Horizontal, binding, 0.0));
    let x1 = document.axis_record_by_identity(AxisIdentity::X1).unwrap();
    assert!(document.measurement_points_are_visible(
        binding,
        (x1.minimum, 1.0),
        (x1.maximum, 100.0)
    ));
    assert!(!document.measurement_points_are_visible(
        binding,
        (x1.minimum, 0.0),
        (x1.maximum, 100.0)
    ));
}
