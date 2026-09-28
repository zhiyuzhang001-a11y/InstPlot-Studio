use super::*;

#[test]
fn axis_editor_display_scale_conversion_is_finite_and_round_trips_common_values() {
    for (raw, exponent) in [(-0.0039, -3), (0.0095, -3), (2.5e4, 4), (0.0, 0)] {
        let displayed = raw_to_axis_display(raw, exponent).unwrap();
        let restored = axis_display_to_raw(displayed, exponent).unwrap();
        assert_eq!(restored, raw, "raw={raw} exponent={exponent}");
    }
    assert!(raw_to_axis_display(f64::INFINITY, -3).is_err());
    assert!(axis_display_to_raw(f64::MAX, 308).is_err());
    assert!(raw_to_axis_display(1.0, -324).is_err());
}

#[test]
fn clearing_axis_numeric_drafts_only_discards_affected_axes() {
    let creation = eframe::CreationContext::_new_kittest(egui::Context::default());
    let mut app = StudioApp::new(&creation, Instant::now(), None);
    for identity in [AxisIdentity::X1, AxisIdentity::Y1] {
        let key = format!("quick-axis-{identity:?}-minimum");
        app.numeric_inputs.insert(
            key,
            DeferredNumericInput {
                source_value: 1.0,
                text: "dirty".to_owned(),
                error: Some("invalid".to_owned()),
            },
        );
        app.axis_numeric_scale_sessions
            .push(AxisNumericScaleSession {
                identity,
                exponent: -3,
            });
    }

    app.clear_axis_numeric_drafts(&[AxisIdentity::Y1]);

    assert!(app.numeric_inputs.contains_key("quick-axis-X1-minimum"));
    assert!(!app.numeric_inputs.contains_key("quick-axis-Y1-minimum"));
    assert_eq!(app.axis_numeric_scale_sessions.len(), 1);
    assert_eq!(
        app.axis_numeric_scale_sessions[0].identity,
        AxisIdentity::X1
    );
}

#[test]
fn trackpad_phase_is_distinct_from_plain_wheel() {
    let event = |phase| egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, 20.0),
        phase,
        modifiers: egui::Modifiers::NONE,
    };
    assert_eq!(
        trackpad_scroll_state(&[event(egui::TouchPhase::Move)], false),
        (false, false)
    );
    assert_eq!(
        trackpad_scroll_state(&[event(egui::TouchPhase::Start)], false),
        (true, true)
    );
    assert_eq!(
        trackpad_scroll_state(&[event(egui::TouchPhase::Move)], true),
        (true, true)
    );
    assert_eq!(
        trackpad_scroll_state(&[event(egui::TouchPhase::End)], true),
        (true, false)
    );
}

#[test]
fn legend_and_annotation_drag_accumulate_frame_deltas() {
    for id in ["legend", "annotation"] {
        let mut drag = ArtistDrag {
            id: id.to_owned(),
            start_x: 40.0,
            start_y: 30.0,
            bounds: (40.0, 30.0, 80.0, 50.0),
            total_delta: egui::Vec2::ZERO,
            press_pointer: None,
            legend: None,
            mode: ArtistDragMode::Move,
            preview_bounds: (40.0, 30.0, 80.0, 50.0),
            candidate_grid: None,
            candidate_placement: None,
            connector_index: None,
            connector_text_bounds: None,
            role: SelectableRole::Annotation,
            measurement: None,
        };
        assert_eq!(
            drag.position_after(egui::vec2(10.0, 4.0), 2.0, 200.0, 150.0),
            (45.0, 32.0)
        );
        assert_eq!(
            drag.position_after(egui::vec2(6.0, 8.0), 2.0, 200.0, 150.0),
            (48.0, 36.0)
        );
        assert_eq!(
            drag.position_after(egui::Vec2::ZERO, 2.0, 200.0, 150.0),
            (48.0, 36.0)
        );
    }
}

#[test]
fn annotation_connector_endpoint_drag_updates_data_coordinates() {
    let creation = eframe::CreationContext::_new_kittest(egui::Context::default());
    let mut app = StudioApp::new(&creation, Instant::now(), None);
    let annotation_id = app
        .document
        .add_annotation(vec![LabelNode::Text("target".to_owned())])
        .unwrap();
    let mut record = app.document.artist_record(&annotation_id).unwrap();
    let ArtistProperties::Annotation {
        x_pt,
        y_pt,
        connectors,
        ..
    } = &mut record.properties
    else {
        unreachable!()
    };
    *x_pt = 120.0;
    *y_pt = 100.0;
    connectors.push(AnnotationConnectorRecord {
        target_x: 0.0,
        target_y: 0.0,
        axes: AxisBinding::PRIMARY,
        stroke: StrokeStyle {
            color_id: "blue".to_owned(),
            width_pt: 0.7,
            dash_pt: Vec::new(),
        },
        start_arrow: false,
        end_arrow: true,
        arrow_head: ArrowHead::Open,
        arrow_size_pt: 5.0,
    });
    app.document.set_artist_record(record).unwrap();
    app.resolved = resolved_preview(&app.document).unwrap();
    let bounds = selected_hit_bounds_for_role(
        &app.resolved,
        &annotation_id,
        Some(SelectableRole::AnnotationConnector),
    )
    .unwrap();
    let axes = app.resolved.layout.result.axes;
    let target = (axes.x + axes.width * 0.75, axes.y + axes.height * 0.25);
    app.handle_artist_drag(
        CanvasDragEvent {
            id: annotation_id.clone(),
            role: SelectableRole::AnnotationConnector,
            bounds,
            delta: egui::Vec2::ZERO,
            pointer: Some(target),
            press_pointer: Some(((bounds.0 + bounds.2) / 2.0, (bounds.1 + bounds.3) / 2.0)),
            mode: ArtistDragMode::Move,
            started: true,
            stopped: true,
            data_index: Some(0),
            shift: false,
        },
        1.0,
    );
    let record = app.document.artist_record(&annotation_id).unwrap();
    let ArtistProperties::Annotation { connectors, .. } = record.properties else {
        unreachable!()
    };
    let expected = data_coordinates_from_local(&app.document, axes, target.0, target.1).unwrap();
    assert!((connectors[0].target_x - expected.0).abs() < 1.0e-9);
    assert!((connectors[0].target_y - expected.1).abs() < 1.0e-9);

    let node = app
        .resolved
        .layout
        .project_ids
        .iter()
        .find_map(|(node, id)| (id == &annotation_id).then_some(*node))
        .unwrap();
    let connector_path = app
        .resolved
        .layout
        .result
        .hit_map
        .items
        .iter()
        .find(|item| {
            item.node == node
                && item.role == SelectableRole::AnnotationConnector
                && item.data_index == Some(0)
        })
        .unwrap()
        .path_proximity
        .clone();
    let bounds = selected_hit_bounds_for_role(
        &app.resolved,
        &annotation_id,
        Some(SelectableRole::AnnotationConnector),
    )
    .unwrap();
    let text_bounds = selected_hit_bounds_for_role(
        &app.resolved,
        &annotation_id,
        Some(SelectableRole::Annotation),
    )
    .unwrap();
    let press = connector_path[1];
    app.handle_artist_drag(
        CanvasDragEvent {
            id: annotation_id.clone(),
            role: SelectableRole::AnnotationConnector,
            bounds,
            delta: egui::Vec2::ZERO,
            pointer: Some((text_bounds.0 - 40.0, text_bounds.3 + 40.0)),
            press_pointer: Some(press),
            mode: ArtistDragMode::Move,
            started: true,
            stopped: true,
            data_index: Some(0),
            shift: true,
        },
        1.0,
    );
    let snapped_path = app
        .resolved
        .layout
        .result
        .hit_map
        .items
        .iter()
        .find(|item| {
            item.node == node
                && item.role == SelectableRole::AnnotationConnector
                && item.data_index == Some(0)
        })
        .unwrap()
        .path_proximity
        .as_slice();
    let dx = (snapped_path[1].0 - snapped_path[0].0).abs();
    let dy = (snapped_path[1].1 - snapped_path[0].1).abs();
    assert!((dx - dy).abs() < 1.0e-3);
}

#[test]
fn shift_snap_uses_screen_space_and_preserves_drag_length() {
    let start = egui::pos2(20.0, 30.0);
    let horizontal = snap_pointer_to_special_angle(start, egui::pos2(91.0, 38.0));
    assert!((horizontal.y - start.y).abs() < 1.0e-4);
    assert!((horizontal.distance(start) - egui::pos2(91.0, 38.0).distance(start)).abs() < 1.0e-4);

    let diagonal = snap_pointer_to_special_angle(start, egui::pos2(83.0, 78.0));
    let delta = diagonal - start;
    assert!((delta.x.abs() - delta.y.abs()).abs() < 1.0e-4);
}

#[test]
fn measurement_endpoint_shift_snap_is_transient_and_uses_the_opposite_endpoint() {
    let creation = eframe::CreationContext::_new_kittest(egui::Context::default());
    let mut app = StudioApp::new(&creation, Instant::now(), None);
    let arrow_id = app
        .document
        .add_measurement_arrow(MeasurementArrowSpec {
            start: (-1.0, 0.0),
            end: (0.0, 0.4),
            axes: AxisBinding::PRIMARY,
            constraint: MeasurementConstraint::Free,
            start_arrow: false,
            end_arrow: true,
            label_nodes: None,
        })
        .unwrap();
    app.resolved = resolved_preview(&app.document).unwrap();

    let axes = app.resolved.layout.result.axes;
    let fixed =
        data_point_to_local(&app.document, axes, AxisBinding::PRIMARY, (-1.0, 0.0)).unwrap();
    let bounds = selected_hit_bounds_for_role(
        &app.resolved,
        &arrow_id,
        Some(SelectableRole::MeasurementArrowEnd),
    )
    .unwrap();
    let press = ((bounds.0 + bounds.2) / 2.0, (bounds.1 + bounds.3) / 2.0);
    app.handle_artist_drag(
        CanvasDragEvent {
            id: arrow_id.clone(),
            role: SelectableRole::MeasurementArrowEnd,
            bounds,
            delta: egui::Vec2::ZERO,
            pointer: Some((fixed.0 + 80.0, fixed.1 + 50.0)),
            press_pointer: Some(press),
            mode: ArtistDragMode::Move,
            started: true,
            stopped: true,
            data_index: None,
            shift: true,
        },
        1.0,
    );
    let ArtistProperties::MeasurementArrow { end_x, end_y, .. } =
        app.document.artist_record(&arrow_id).unwrap().properties
    else {
        unreachable!()
    };
    let snapped = data_point_to_local(
        &app.document,
        app.resolved.layout.result.axes,
        AxisBinding::PRIMARY,
        (end_x, end_y),
    )
    .unwrap();
    assert!(((snapped.0 - fixed.0).abs() - (snapped.1 - fixed.1).abs()).abs() < 1.0e-3);

    let bounds = selected_hit_bounds_for_role(
        &app.resolved,
        &arrow_id,
        Some(SelectableRole::MeasurementArrowEnd),
    )
    .unwrap();
    let press = ((bounds.0 + bounds.2) / 2.0, (bounds.1 + bounds.3) / 2.0);
    app.handle_artist_drag(
        CanvasDragEvent {
            id: arrow_id.clone(),
            role: SelectableRole::MeasurementArrowEnd,
            bounds,
            delta: egui::Vec2::ZERO,
            pointer: Some((fixed.0 + 80.0, fixed.1 + 20.0)),
            press_pointer: Some(press),
            mode: ArtistDragMode::Move,
            started: true,
            stopped: true,
            data_index: None,
            shift: false,
        },
        1.0,
    );
    let ArtistProperties::MeasurementArrow { end_x, end_y, .. } =
        app.document.artist_record(&arrow_id).unwrap().properties
    else {
        unreachable!()
    };
    let unsnapped = data_point_to_local(
        &app.document,
        app.resolved.layout.result.axes,
        AxisBinding::PRIMARY,
        (end_x, end_y),
    )
    .unwrap();
    assert!(((unsnapped.0 - fixed.0).abs() - (unsnapped.1 - fixed.1).abs()).abs() > 10.0);
}

#[test]
fn measurement_label_drag_updates_only_its_saved_point_offset() {
    let creation = eframe::CreationContext::_new_kittest(egui::Context::default());
    let mut app = StudioApp::new(&creation, Instant::now(), None);
    let arrow_id = app
        .document
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
    app.resolved = resolved_preview(&app.document).unwrap();
    let bounds = selected_hit_bounds_for_role(
        &app.resolved,
        &arrow_id,
        Some(SelectableRole::MeasurementArrowLabel),
    )
    .unwrap();
    let press = ((bounds.0 + bounds.2) / 2.0, (bounds.1 + bounds.3) / 2.0);
    app.handle_artist_drag(
        CanvasDragEvent {
            id: arrow_id.clone(),
            role: SelectableRole::MeasurementArrowLabel,
            bounds,
            delta: egui::Vec2::ZERO,
            pointer: Some((press.0 + 12.0, press.1 - 8.0)),
            press_pointer: Some(press),
            mode: ArtistDragMode::Move,
            started: true,
            stopped: true,
            data_index: None,
            shift: false,
        },
        1.0,
    );

    let ArtistProperties::MeasurementArrow {
        start_x,
        start_y,
        end_x,
        end_y,
        label_offset_x_pt,
        label_offset_y_pt,
        ..
    } = app.document.artist_record(&arrow_id).unwrap().properties
    else {
        unreachable!()
    };
    assert_eq!((start_x, start_y, end_x, end_y), (-1.0, 0.0, 1.0, 0.0));
    assert!((label_offset_x_pt - 12.0).abs() < 1.0e-9);
    assert!((label_offset_y_pt + 16.0).abs() < 1.0e-9);
    assert!(app.active_artist_drag.is_none());
}

#[test]
fn reference_tool_previews_then_commits_or_cancels_and_measurement_commits_once() {
    let creation = eframe::CreationContext::_new_kittest(egui::Context::default());
    let mut app = StudioApp::new(&creation, Instant::now(), None);
    let before = app.document.project().figure.artists.len();
    app.drawing_tool = DrawingTool::Reference {
        orientation: ReferenceOrientation::Vertical,
        axes: AxisBinding::PRIMARY,
    };
    app.handle_drawing_tool_event(CanvasToolEvent {
        clicked: true,
        started: false,
        stopped: false,
        press: None,
        current: Some(HoverDataCoordinates {
            x1: 0.75,
            y1: 0.25,
            x2: None,
            y2: None,
        }),
        shift: false,
    });
    assert_eq!(app.document.project().figure.artists.len(), before);
    assert!(app.reference_draft.is_some());
    assert!(app.commit_reference_draft(true));
    assert_eq!(app.document.project().figure.artists.len(), before + 1);
    assert!(matches!(app.drawing_tool, DrawingTool::Reference { .. }));
    assert_eq!(
        app.selected_canvas_role,
        Some(SelectableRole::ReferenceLine)
    );

    app.handle_drawing_tool_event(CanvasToolEvent {
        clicked: true,
        started: false,
        stopped: false,
        press: None,
        current: Some(HoverDataCoordinates {
            x1: 1.25,
            y1: 0.25,
            x2: None,
            y2: None,
        }),
        shift: false,
    });
    assert!(app.reference_draft.is_some());
    app.cancel_reference_draft();
    assert_eq!(app.document.project().figure.artists.len(), before + 1);
    assert_eq!(app.drawing_tool, DrawingTool::Select);

    app.drawing_tool = DrawingTool::Measurement {
        axes: AxisBinding::PRIMARY,
        constraint: MeasurementConstraint::Horizontal,
        start_arrow: true,
        end_arrow: true,
    };
    app.handle_drawing_tool_event(CanvasToolEvent {
        clicked: false,
        started: true,
        stopped: true,
        press: Some(HoverDataCoordinates {
            x1: -1.0,
            y1: 1.0,
            x2: None,
            y2: None,
        }),
        current: Some(HoverDataCoordinates {
            x1: 1.0,
            y1: 1.2,
            x2: None,
            y2: None,
        }),
        shift: false,
    });
    assert_eq!(app.document.project().figure.artists.len(), before + 2);
    assert_eq!(app.drawing_tool, DrawingTool::Select);
    let ArtistProperties::MeasurementArrow {
        start_y,
        end_y,
        start_arrow,
        end_arrow,
        ..
    } = &app
        .document
        .project()
        .figure
        .artists
        .last()
        .unwrap()
        .properties
    else {
        panic!("measurement arrow was not created");
    };
    assert_eq!(start_y, end_y);
    assert!(*start_arrow && *end_arrow);
}

#[test]
fn annotation_editor_preserves_hard_line_breaks_but_axis_editor_does_not() {
    let mut annotation = "First line\r\nSecond line".to_owned();
    assert!(normalize_label_editor_line_breaks(&mut annotation, true));
    assert_eq!(annotation, "First line\nSecond line");

    let mut axis = "Field\n(mT)".to_owned();
    assert!(normalize_label_editor_line_breaks(&mut axis, false));
    assert_eq!(axis, "Field (mT)");
}

#[test]
fn new_annotation_connectors_use_black_at_point_nine_points() {
    let document = FigureDocument::showcase();
    let black = document
        .palette_colors()
        .iter()
        .find(|color| color.id == ANNOTATION_CONNECTOR_DEFAULT_COLOR_ID)
        .expect("connector black must be available in the editor palette");
    assert_eq!(black.rgba, [0, 0, 0, 255]);
    assert_eq!(ANNOTATION_CONNECTOR_DEFAULT_WIDTH_PT, 0.9);
}

#[test]
fn legend_drag_can_cross_into_and_out_of_the_outside_bands() {
    let context = LegendDragContext {
        axes_left: 40.0,
        axes_right: 220.0,
        axes_top: 100.0,
        axes_bottom: 280.0,
        canvas_top: 88.0,
        entries: 7,
        cell_width: 60.0,
    };
    assert_eq!(context.placement_at(250.0, 110.0), LegendPlacement::Right);
    assert_eq!(context.placement_at(200.0, 110.0), LegendPlacement::Inside);
    assert_eq!(context.placement_at(100.0, 90.0), LegendPlacement::Above);
    assert_eq!(context.placement_at(160.0, 110.0), LegendPlacement::Inside);
    assert_eq!(context.placement_at(220.0, 100.0), LegendPlacement::Inside);
    let far_right = (500.0, 120.0, 560.0, 140.0);
    let compact = context.preview_bounds(far_right, LegendPlacement::Right);
    assert_eq!(compact, (226.0, 120.0, 286.0, 140.0));
    assert_eq!(
        context.stored_position(compact, LegendPlacement::Right),
        (226.0, 32.0)
    );
    let crossing = (200.0, 120.0, 260.0, 140.0);
    assert_eq!(
        context.preview_bounds(crossing, LegendPlacement::Inside),
        (160.0, 120.0, 220.0, 140.0)
    );
    assert_eq!(
        context.stored_position(crossing, LegendPlacement::Inside),
        (160.0, 32.0)
    );
}

#[test]
fn legend_drag_across_all_placements_round_trips_and_exports() {
    let context = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(context);
    let mut app = StudioApp::new(&creation, Instant::now(), None);
    let legend_id = app
        .document
        .project()
        .figure
        .artists
        .iter()
        .find(|artist| artist.kind == instplot_studio::ArtistKind::Legend)
        .unwrap()
        .id
        .clone();
    let mut record = app.document.artist_record(&legend_id).unwrap();
    record.visible = true;
    assert!(app.execute_document_edit(EditCommand::SetArtistRecord(record), "show legend"));

    for expected in [
        LegendPlacement::Right,
        LegendPlacement::Above,
        LegendPlacement::Inside,
    ] {
        let axes = app.resolved.layout.result.axes;
        let bounds =
            selected_hit_bounds_for_role(&app.resolved, &legend_id, Some(SelectableRole::Legend))
                .unwrap();
        let press = ((bounds.0 + bounds.2) / 2.0, (bounds.1 + bounds.3) / 2.0);
        let target = match expected {
            LegendPlacement::Right => (axes.right() + 20.0, axes.y + 20.0),
            LegendPlacement::Above => (axes.x + 20.0, axes.y - 20.0),
            LegendPlacement::Inside => (axes.x + 20.0, axes.y + 20.0),
            LegendPlacement::Auto => unreachable!(),
        };
        app.handle_artist_drag(
            CanvasDragEvent {
                id: legend_id.clone(),
                role: SelectableRole::Legend,
                bounds,
                delta: egui::Vec2::ZERO,
                pointer: Some(target),
                press_pointer: Some(press),
                mode: ArtistDragMode::Move,
                started: true,
                stopped: true,
                data_index: None,
                shift: false,
            },
            1.0,
        );
        let ArtistProperties::Legend { placement, .. } =
            app.document.artist_record(&legend_id).unwrap().properties
        else {
            panic!("legend artist missing");
        };
        assert_eq!(placement, expected);
        if expected == LegendPlacement::Inside {
            let legend = app.resolved.layout.result.legend.unwrap();
            let axes = app.resolved.layout.result.axes;
            assert!(legend.x >= axes.x - 1.0);
            assert!(legend.right() <= axes.right() + 1.0);
            assert!(legend.y >= axes.y - 1.0);
            assert!(legend.bottom() <= axes.bottom() + 1.0);
        }
        app.document.project().validate().unwrap();
        let encoded = serde_json::to_vec(app.document.project()).unwrap();
        let reopened =
            FigureDocument::from_project(serde_json::from_slice(&encoded).unwrap()).unwrap();
        let ArtistProperties::Legend {
            placement: reopened_placement,
            x_pt: reopened_x,
            y_pt: reopened_y,
            ..
        } = reopened.artist_record(&legend_id).unwrap().properties
        else {
            panic!("reopened legend artist missing");
        };
        let ArtistProperties::Legend { x_pt, y_pt, .. } =
            app.document.artist_record(&legend_id).unwrap().properties
        else {
            unreachable!();
        };
        assert_eq!(reopened_placement, expected);
        assert!((reopened_x - x_pt).abs() < 1e-8);
        assert!((reopened_y - y_pt).abs() < 1e-8);
        assert!(
            instplot_studio::figure_pdf(&reopened)
                .unwrap()
                .starts_with(b"%PDF-")
        );
        assert!(
            instplot_studio::figure_png(&reopened, 300)
                .unwrap()
                .starts_with(b"\x89PNG\r\n\x1a\n")
        );
        if matches!(expected, LegendPlacement::Above | LegendPlacement::Right) {
            let axes = app.resolved.layout.result.axes;
            let (axis_id, local) = if expected == LegendPlacement::Above {
                (
                    app.document.project().figure.axes[0].x.id.clone(),
                    egui::pos2((axes.x + axes.right()) as f32 * 0.5, axes.y as f32),
                )
            } else {
                (
                    app.document.project().figure.axes[0].y.id.clone(),
                    egui::pos2(axes.right() as f32, (axes.y + axes.bottom()) as f32 * 0.5),
                )
            };
            let origin = egui::pos2(73.0, 41.0);
            let zoom = 1.6;
            let point = origin + local.to_vec2() * zoom;
            let candidates = hit_project_candidates(&app.resolved, point, origin, zoom);
            assert!(candidates.iter().any(|hit| hit.project_id == axis_id));
            assert!(
                candidates
                    .iter()
                    .all(|hit| hit.role != SelectableRole::Legend)
            );
        }
    }
}

#[test]
fn legend_resize_handles_are_distinct_from_the_move_area() {
    let bounds = (10.0, 10.0, 110.0, 50.0);
    let origin = egui::Pos2::ZERO;
    assert_eq!(
        legend_resize_handle_at(egui::pos2(114.0, 30.0), bounds, origin, 1.0),
        Some(ArtistDragMode::ResizeColumns)
    );
    assert_eq!(
        legend_resize_handle_at(egui::pos2(60.0, 54.0), bounds, origin, 1.0),
        Some(ArtistDragMode::ResizeRows)
    );
    assert_eq!(
        legend_resize_handle_at(egui::pos2(60.0, 30.0), bounds, origin, 1.0),
        None
    );
}

#[test]
fn one_frame_drag_uses_release_pointer_even_without_intermediate_delta() {
    let drag = ArtistDrag {
        id: "legend".to_owned(),
        start_x: 20.0,
        start_y: 10.0,
        bounds: (20.0, 10.0, 80.0, 30.0),
        total_delta: egui::Vec2::ZERO,
        press_pointer: Some((40.0, 20.0)),
        legend: None,
        mode: ArtistDragMode::Move,
        preview_bounds: (20.0, 10.0, 80.0, 30.0),
        candidate_grid: None,
        candidate_placement: None,
        connector_index: None,
        connector_text_bounds: None,
        role: SelectableRole::Legend,
        measurement: None,
    };
    assert_eq!(
        drag.frame_delta(Some((50.0, 120.0)), egui::Vec2::ZERO, 1.5),
        egui::vec2(15.0, 150.0)
    );
}

#[test]
fn same_frame_drag_requires_a_real_primary_button_displacement() {
    let event = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    assert_eq!(
        same_frame_primary_drag(&[
            event(egui::pos2(10.0, 20.0), true),
            egui::Event::PointerMoved(egui::pos2(10.0, 100.0)),
            event(egui::pos2(10.0, 100.0), false),
        ]),
        Some((egui::pos2(10.0, 20.0), egui::pos2(10.0, 100.0)))
    );
    assert_eq!(
        same_frame_primary_drag(&[
            event(egui::pos2(10.0, 20.0), true),
            event(egui::pos2(12.0, 21.0), false),
        ]),
        None
    );
}

#[test]
fn canvas_click_does_not_close_an_existing_editor() {
    let creation = eframe::CreationContext::_new_kittest(egui::Context::default());
    let mut app = StudioApp::new(&creation, Instant::now(), None);
    let legend = CanvasHit {
        project_id: "legend".to_owned(),
        role: SelectableRole::Legend,
        data_index: None,
    };
    let axes = CanvasHit {
        project_id: "axes".to_owned(),
        role: SelectableRole::Axes,
        data_index: None,
    };
    app.open_context_editor(legend.clone());
    app.open_context_editor(axes.clone());
    app.open_context_editor(legend.clone());
    assert_eq!(
        app.context_editor_targets,
        [
            CanvasHit {
                project_id: "legend".to_owned(),
                role: SelectableRole::Legend,
                data_index: None,
            },
            axes,
        ]
    );
    assert_eq!(app.context_editor_focus_target, Some(legend));
}

#[test]
fn one_drawing_object_has_one_context_editor_across_all_hit_roles() {
    let creation = eframe::CreationContext::_new_kittest(egui::Context::default());
    let mut app = StudioApp::new(&creation, Instant::now(), None);
    let arrow_id = app
        .document
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
    for role in [
        SelectableRole::MeasurementArrowStart,
        SelectableRole::MeasurementArrowEnd,
        SelectableRole::MeasurementArrowLabel,
        SelectableRole::MeasurementArrow,
    ] {
        app.open_context_editor(CanvasHit {
            project_id: arrow_id.clone(),
            role,
            data_index: Some(0),
        });
    }
    assert_eq!(app.context_editor_targets.len(), 1);
    assert_eq!(
        app.context_editor_targets[0],
        CanvasHit {
            project_id: arrow_id,
            role: SelectableRole::MeasurementArrow,
            data_index: None,
        }
    );
}

#[test]
fn deleted_objects_are_pruned_from_context_editors() {
    let creation = eframe::CreationContext::_new_kittest(egui::Context::default());
    let mut app = StudioApp::new(&creation, Instant::now(), None);
    let arrow_id = app
        .document
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
    app.open_context_editor(CanvasHit {
        project_id: arrow_id.clone(),
        role: SelectableRole::MeasurementArrow,
        data_index: None,
    });
    assert_eq!(app.context_editor_targets.len(), 1);

    app.document.delete_drawing_object(&arrow_id).unwrap();
    app.prune_context_editor_targets();

    assert!(app.context_editor_targets.is_empty());
    assert!(app.context_editor_focus_target.is_none());
}

#[test]
fn legacy_reference_dash_is_named_as_dashed() {
    assert_eq!(
        dash_name(UiLanguage::Chinese, &[4.0, 3.0]),
        UiLanguage::Chinese.text(Text::Dashed)
    );
    assert_eq!(
        dash_name(UiLanguage::Chinese, &DEFAULT_REFERENCE_DASH_PT),
        UiLanguage::Chinese.text(Text::Dashed)
    );
}

#[test]
fn repeated_tool_commands_raise_instead_of_closing_or_resetting_the_window() {
    let creation = eframe::CreationContext::_new_kittest(egui::Context::default());
    let mut app = StudioApp::new(&creation, Instant::now(), None);

    app.request_palette_window();
    app.request_palette_window();
    assert!(app.show_palette && app.focus_palette);

    app.request_publication_window();
    app.request_publication_window();
    assert!(app.show_inspector && app.focus_inspector);

    app.request_axis_visibility_window();
    app.request_axis_visibility_window();
    assert!(app.show_axis_visibility && app.focus_axis_visibility);

    app.prepare_manual_data_window();
    app.manual_data.input.groups[0].source_name = "Unsaved draft".to_owned();
    app.focus_manual_data = false;
    app.prepare_manual_data_window();
    assert!(app.manual_data.open && app.focus_manual_data);
    assert_eq!(app.manual_data.input.groups[0].source_name, "Unsaved draft");
}

#[test]
fn studio_interface_style_keeps_readable_controls_and_dialog_spacing() {
    let context = egui::Context::default();
    configure_interface_style(&context);
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let style = context.style_of(theme);
        assert_eq!(egui::TextStyle::Small.resolve(&style).size, 15.0);
        assert_eq!(egui::TextStyle::Body.resolve(&style).size, 16.0);
        assert_eq!(egui::TextStyle::Button.resolve(&style).size, 16.0);
        assert_eq!(egui::TextStyle::Heading.resolve(&style).size, 22.0);
        assert_eq!(style.spacing.button_padding, egui::vec2(14.0, 9.0));
        assert_eq!(style.spacing.interact_size.y, 40.0);
        assert_eq!(style.spacing.item_spacing.y, 12.0);
        assert_eq!(style.spacing.extra_text_line_spacing, 4.0);
        assert_eq!(style.spacing.window_margin, egui::Margin::symmetric(18, 16));
        assert_eq!(style.interaction.resize_grab_radius_side, 8.0);
        assert_ne!(
            style.visuals.panel_fill,
            studio_surface(theme == egui::Theme::Dark)
        );
    }
}

#[test]
fn editor_buttons_keep_full_height_hit_targets() {
    let context = egui::Context::default();
    configure_interface_style(&context);
    let mut output = context.run_ui(egui::RawInput::default(), |ui| {
        let format = studio_label_format_button(ui, "斜体");
        assert!(format.rect.width() >= 64.0);
        assert!(format.rect.height() >= 40.0);
        let symbol = ui.add_sized([40.0, 40.0], egui::Button::new("σ"));
        assert!(symbol.rect.width() >= 40.0);
        assert!(symbol.rect.height() >= 40.0);
        assert!(ui.button("导出").rect.height() >= 40.0);
    });
    output.textures_delta.clear();
}

#[test]
fn data_sidebar_close_button_is_exactly_centered() {
    let context = egui::Context::default();
    configure_interface_style(&context);
    let mut output = context.run_ui(egui::RawInput::default(), |ui| {
        let response = studio_close_button(ui, "删除");
        assert_eq!(response.rect.size(), egui::vec2(34.0, 34.0));
        let file_response =
            studio_close_button_sized(ui, "删除文件", SIDEBAR_FILE_CLOSE_BUTTON_SIZE);
        assert_eq!(file_response.rect.size(), egui::vec2(28.0, 28.0));
    });
    output.textures_delta.clear();
}

#[test]
fn data_sidebar_resize_drag_persists_after_release() {
    fn frame(context: &egui::Context, events: Vec<egui::Event>) -> (f32, egui::Rect) {
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1_000.0, 700.0),
            )),
            events,
            ..Default::default()
        };
        input.focused = true;
        let mut output = context.run_ui(input, |ui| {
            egui::Panel::left("series_tree")
                .resizable(true)
                .show_separator_line(true)
                .default_size(DATA_SIDEBAR_DEFAULT_WIDTH)
                .min_size(DATA_SIDEBAR_MIN_WIDTH)
                .max_size(DATA_SIDEBAR_MAX_WIDTH)
                .show(ui, |ui| {
                    egui::ScrollArea::both()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.set_width(DATA_SIDEBAR_CONTENT_MIN_WIDTH);
                            ui.label("data");
                        });
                });
            egui::CentralPanel::default().show(ui, |_| {});
        });
        output.textures_delta.clear();
        let width =
            egui::containers::panel::PanelState::load(context, egui::Id::new("series_tree"))
                .expect("sidebar panel state")
                .size()
                .x;
        let handle = context
            .read_response(egui::Id::new("series_tree").with("__resize"))
            .expect("sidebar resize handle")
            .rect;
        (width, handle)
    }

    let context = egui::Context::default();
    let (initial_width, handle) = frame(&context, Vec::new());
    let start = handle.center();
    let end = start + egui::vec2(90.0, 0.0);
    frame(
        &context,
        vec![
            egui::Event::PointerMoved(start),
            egui::Event::PointerButton {
                pos: start,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    frame(&context, vec![egui::Event::PointerMoved(end)]);
    let (released_width, _) = frame(
        &context,
        vec![egui::Event::PointerButton {
            pos: end,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    let (next_frame_width, _) = frame(&context, Vec::new());

    assert!(released_width > initial_width + 60.0);
    assert!((next_frame_width - released_width).abs() < 0.5);
}

#[test]
fn label_preview_uses_the_same_bundled_symbol_face_as_export() {
    let job = label_preview_job(&[
        LabelNode::GreekVariable('ϵ'),
        LabelNode::Operator("≤".into()),
        LabelNode::Number("300".into()),
    ]);
    let faces = job
        .sections
        .iter()
        .map(|section| &section.format.font_id.family)
        .collect::<Vec<_>>();
    assert_eq!(
        faces,
        vec![
            &egui::FontFamily::Name(std::sync::Arc::from("TeXGyreHeros-Italic")),
            &egui::FontFamily::Name(std::sync::Arc::from("STIXTwoMath-Regular")),
            &egui::FontFamily::Name(std::sync::Arc::from("TeXGyreHeros-Regular")),
        ]
    );
}

#[test]
fn label_toolbar_wraps_selection_and_places_cursor_inside_empty_scripts() {
    let (wrapped, cursor) = apply_label_snippet("μH", 1..2, "_{}", true);
    assert_eq!(wrapped, "μ_{H}");
    assert_eq!(cursor, 4);
    let (empty, cursor) = apply_label_snippet("H", 1..1, "^{}", true);
    assert_eq!(empty, "H^{}");
    assert_eq!(cursor, 3);
    let (symbol, cursor) = apply_label_snippet("H", 1..1, "σ", false);
    assert_eq!(symbol, "Hσ");
    assert_eq!(cursor, 2);
}

#[test]
fn fixed_tick_text_accepts_finite_values_and_rejects_bad_input() {
    assert_eq!(
        parse_fixed_ticks("-2; 0, 1.5\t3").unwrap(),
        vec![-2.0, 0.0, 1.5, 3.0]
    );
    for invalid in ["", "   ", "oops", "1, NaN", "-inf 1", "1e999"] {
        assert!(parse_fixed_ticks(invalid).is_err(), "accepted {invalid:?}");
    }
}

#[test]
fn unit_conflict_detection_requires_explicit_delimiters() {
    assert_eq!(explicit_column_unit("Field (mT)"), Some("mT"));
    assert_eq!(explicit_column_unit("Current [A m^-2]"), Some("A m^-2"));
    assert_eq!(explicit_column_unit("temperature_K"), None);
    assert_eq!(explicit_column_unit("signal"), None);
}

#[test]
fn cursor_coordinates_use_formal_axes_and_scale_mapping() {
    let document = FigureDocument::fixed();
    let resolved = resolved_preview(&document).unwrap();
    let axes = resolved.layout.result.axes;
    let origin = egui::pos2(17.0, 23.0);
    let zoom = 1.75;
    let screen = |x: f64, y: f64| origin + egui::vec2(x as f32, y as f32) * zoom;
    let lower_left = data_coordinates_at(
        &resolved,
        &document,
        screen(axes.x, axes.bottom()),
        origin,
        zoom,
    )
    .unwrap();
    let upper_right = data_coordinates_at(
        &resolved,
        &document,
        screen(axes.right(), axes.y),
        origin,
        zoom,
    )
    .unwrap();
    let ranges = document.axis_ranges();
    assert!((lower_left.0 - ranges.x_min).abs() < 1e-5);
    assert!((lower_left.1 - ranges.y_min).abs() < 1e-5);
    assert!((upper_right.0 - ranges.x_max).abs() < 1e-5);
    assert!((upper_right.1 - ranges.y_max).abs() < 1e-5);
    assert!(
        data_coordinates_at(
            &resolved,
            &document,
            screen(axes.x - 1.0, axes.y),
            origin,
            zoom,
        )
        .is_none()
    );
    let mut log_axis = document.axis_record(AxisDimension::X);
    log_axis.scale = AxisScale::Log10;
    log_axis.minimum = 1.0;
    log_axis.maximum = 100.0;
    assert!((axis_value_at_fraction(&log_axis, 0.5).unwrap() - 10.0).abs() < 1e-9);
    assert_eq!(format_data_coordinate(-0.25), "−0.25");
}

#[test]
fn cursor_coordinates_include_only_the_active_secondary_axis() {
    let mut document = FigureDocument::fixed();
    document.set_axis_mode(AxisMode::DualY).unwrap();
    let mut y2 = document.axis_record_by_identity(AxisIdentity::Y2).unwrap();
    y2.autoscale = false;
    y2.minimum = 100.0;
    y2.maximum = 200.0;
    document
        .set_axis_record_by_identity(AxisIdentity::Y2, y2)
        .unwrap();
    let resolved = resolved_preview(&document).unwrap();
    let axes = resolved.layout.result.axes;
    let point = egui::pos2(axes.x as f32, axes.y as f32);
    let coordinates =
        active_data_coordinates_at(&resolved, &document, point, egui::Pos2::ZERO, 1.0).unwrap();
    assert!(coordinates.x2.is_none());
    assert_eq!(coordinates.y2, Some(200.0));
}

#[test]
fn closing_an_empty_annotation_deletes_it_as_one_undoable_edit() {
    let context = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(context);
    let mut app = StudioApp::new(&creation, Instant::now(), None);
    let before = app
        .document
        .series()
        .into_iter()
        .map(|series| series.id)
        .collect::<BTreeSet<_>>();
    app.add_text_annotation();
    let annotation = app
        .document
        .series()
        .into_iter()
        .find(|series| series.kind == SeriesKind::Annotation && !before.contains(&series.id))
        .unwrap();
    let record = app.document.artist_record(&annotation.id).unwrap();
    let ArtistProperties::Annotation { label_id, .. } = record.properties else {
        panic!("new text must be an annotation");
    };
    let nodes = app
        .document
        .semantic_label_nodes(&label_id)
        .unwrap()
        .to_vec();
    app.label_inputs.insert(
        label_id,
        LabelInputState {
            source_nodes: nodes,
            text: String::new(),
        },
    );
    app.delete_empty_annotation_on_close(&annotation.id);
    assert!(app.document.artist_record(&annotation.id).is_none());
    app.undo();
    assert!(app.document.artist_record(&annotation.id).is_some());
}

#[test]
fn preview_uses_formal_layout_and_resolved_rotated_text() {
    let resolved = resolved_preview(&FigureDocument::fixed()).unwrap();
    assert!(!resolved.display.items.iter().any(|item| matches!(
        item,
        ResolvedItem::Graphics(DisplayItem::Path {
            stroke: Some(stroke),
            ..
        }) if stroke.color == Color(218, 221, 224, 255)
    )));
    assert!(resolved.display.items.iter().any(|item| matches!(
        item,
        ResolvedItem::Text(text)
            if text.source == NodeId(4) && text.rotation_degrees == -90.0
    )));
    assert!(resolved.display.items.iter().any(|item| matches!(
        item,
        ResolvedItem::Graphics(DisplayItem::Path { source, .. })
            if *source == NodeId(11)
    )));
    assert!(resolved.display.items.iter().any(|item| matches!(
        item,
        ResolvedItem::Graphics(DisplayItem::Path { source, .. })
            if *source == NodeId(13)
    )));
    assert!(
        !resolved
            .display
            .items
            .iter()
            .any(|item| matches!(item, ResolvedItem::Graphics(DisplayItem::GlyphRun(_))))
    );
}

#[test]
fn p6_hit_testing_and_selection_bounds_use_formal_hit_map() {
    let resolved = resolved_preview(&FigureDocument::fixed()).unwrap();
    let bounds = selected_hit_bounds(&resolved, "node-15").unwrap();
    let local = egui::pos2(
        ((bounds.0 + bounds.2) / 2.0) as f32,
        ((bounds.1 + bounds.3) / 2.0) as f32,
    );
    let origin = egui::pos2(120.0, 80.0);
    let zoom = 1.75;
    let screen = origin + local.to_vec2() * zoom;
    assert_eq!(
        hit_project_id(&resolved, screen, origin, zoom).as_deref(),
        Some("node-15")
    );
}

#[test]
fn annotation_selection_uses_visible_glyph_ink() {
    let resolved = resolved_preview(&FigureDocument::fixed()).unwrap();
    let annotation = resolved
        .display
        .items
        .iter()
        .find_map(|item| match item {
            ResolvedItem::Text(text)
                if resolved
                    .layout
                    .project_ids
                    .get(&text.source)
                    .map(String::as_str)
                    == Some("node-14") =>
            {
                Some(text)
            }
            _ => None,
        })
        .unwrap();
    let ink = resolved_text_ink_bounds(annotation).unwrap();
    let selection =
        selected_hit_bounds_for_role(&resolved, "node-14", Some(SelectableRole::Annotation))
            .unwrap();
    for (actual, expected) in [
        (selection.0, ink.0),
        (selection.1, ink.1),
        (selection.2, ink.2),
        (selection.3, ink.3),
    ] {
        assert!((actual - expected).abs() < 0.01);
    }
}

#[test]
fn legend_numeric_order_moves_an_entry_without_reversing_others() {
    let mut entries = vec!["A", "B", "C", "D"];
    move_legend_entry(&mut entries, 0, 2);
    assert_eq!(entries, ["B", "C", "A", "D"]);
    move_legend_entry(&mut entries, 3, 1);
    assert_eq!(entries, ["B", "D", "C", "A"]);
}

#[test]
fn p6_pointer_centered_zoom_keeps_the_document_anchor_stable() {
    let origin = egui::pos2(40.0, 30.0);
    let pointer = egui::pos2(160.0, 120.0);
    let old_zoom = 1.0;
    let old_scroll = egui::vec2(12.0, 8.0);
    let document_point = (pointer - origin) / old_zoom;
    let (new_scroll, new_zoom) = zoom_about_pointer(old_scroll, pointer, origin, old_zoom, 120.0);
    let old_content = old_scroll + document_point * old_zoom;
    let new_content = new_scroll + document_point * old_zoom;
    assert!((new_content - old_content - document_point * (new_zoom - old_zoom)).length() < 1e-4);
    assert!(new_zoom > old_zoom);
}
