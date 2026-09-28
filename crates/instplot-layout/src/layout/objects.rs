use super::*;

pub(super) fn draw_reference_lines(
    chart: &Chart,
    axes: Bounds,
    list: &mut DisplayList,
    hit_map: &mut HitMap,
) -> Result<(), LayoutError> {
    if chart.reference_lines.is_empty() {
        return Ok(());
    }
    list.items.push(DisplayItem::ClipPush {
        source: chart.id,
        x: pt(axes.x),
        y: pt(axes.y),
        width: pt(axes.width),
        height: pt(axes.height),
    });
    for (index, reference) in chart.reference_lines.iter().enumerate() {
        let pair = reference.axes;
        let Some((x_axis, y_axis)) = axis_specs(chart, pair) else {
            return Err(LayoutError::InvalidData(reference.id));
        };
        let (start, end) = match reference.orientation {
            ReferenceOrientation::Vertical => (
                DataPoint {
                    x: reference.value,
                    y: y_axis.minimum,
                },
                DataPoint {
                    x: reference.value,
                    y: y_axis.maximum,
                },
            ),
            ReferenceOrientation::Horizontal => (
                DataPoint {
                    x: x_axis.minimum,
                    y: reference.value,
                },
                DataPoint {
                    x: x_axis.maximum,
                    y: reference.value,
                },
            ),
        };
        let Some((start, end)) =
            map_point(chart, axes, pair, start).zip(map_point(chart, axes, pair, end))
        else {
            return Err(LayoutError::InvalidData(reference.id));
        };
        let points = vec![start, end];
        list.items.push(DisplayItem::Path {
            source: reference.id,
            path: polyline(&points),
            fill: None,
            stroke: Some(stroke(
                reference.color,
                reference.stroke.width,
                reference.stroke.dash,
            )),
        });
        hit_map.items.push(HitItem {
            node: reference.id,
            bounds: bounds_of_points(&points, reference.stroke.width + 4.0),
            z_order: 50 + index as u32,
            role: SelectableRole::ReferenceLine,
            data_index: None,
            tooltip: Some(format!("reference: {:.6}", reference.value)),
            path_proximity: points,
        });
    }
    list.items.push(DisplayItem::ClipPop { source: chart.id });
    Ok(())
}

pub(super) fn draw_measurement_arrows(
    chart: &Chart,
    axes: Bounds,
    measurer: &mut dyn TextMeasurer,
    list: &mut DisplayList,
    hit_map: &mut HitMap,
    warnings: &mut Vec<LayoutWarning>,
) -> Result<(), LayoutError> {
    if chart.measurement_arrows.is_empty() {
        return Ok(());
    }
    list.items.push(DisplayItem::ClipPush {
        source: chart.id,
        x: pt(axes.x),
        y: pt(axes.y),
        width: pt(axes.width),
        height: pt(axes.height),
    });
    for (index, arrow) in chart.measurement_arrows.iter().enumerate() {
        let Some((start, end)) = map_point(chart, axes, arrow.axes, arrow.start)
            .zip(map_point(chart, axes, arrow.axes, arrow.end))
        else {
            return Err(LayoutError::InvalidData(arrow.id));
        };
        draw_measurement_arrow_path(arrow, start, end, list);
        let points = vec![start, end];
        let z = 600 + index as u32 * 4;
        let arrow_padding =
            (arrow.arrow_size + arrow.stroke.width / 2.0 + 4.0).max(arrow.stroke.width + 5.0);
        hit_map.items.push(HitItem {
            node: arrow.id,
            bounds: bounds_of_points(&points, arrow_padding),
            z_order: z,
            role: SelectableRole::MeasurementArrow,
            data_index: None,
            tooltip: Some("measurement arrow".to_owned()),
            path_proximity: points,
        });
        for (point, role, endpoint) in [
            (start, SelectableRole::MeasurementArrowStart, 0),
            (end, SelectableRole::MeasurementArrowEnd, 1),
        ] {
            hit_map.items.push(HitItem {
                node: arrow.id,
                bounds: Bounds {
                    x: point.0 - 5.0,
                    y: point.1 - 5.0,
                    width: 10.0,
                    height: 10.0,
                },
                z_order: z + 2,
                role,
                data_index: Some(endpoint),
                tooltip: None,
                path_proximity: vec![point],
            });
        }
    }
    list.items.push(DisplayItem::ClipPop { source: chart.id });
    for (index, arrow) in chart.measurement_arrows.iter().enumerate() {
        if arrow.labels.is_empty() {
            continue;
        }
        let Some((start, end)) = map_point(chart, axes, arrow.axes, arrow.start)
            .zip(map_point(chart, axes, arrow.axes, arrow.end))
        else {
            continue;
        };
        let center = (
            (start.0 + end.0) / 2.0 + arrow.label_offset_pt.0,
            (start.1 + end.1) / 2.0 + arrow.label_offset_pt.1,
        );
        let sizes = arrow
            .labels
            .iter()
            .map(|label| measurer.measure_label(label, TICK_FONT))
            .collect::<Vec<_>>();
        let line_height = TICK_FONT * 1.25;
        let width = sizes.iter().map(|size| size.width).fold(0.0, f64::max);
        let first_ascent = sizes.first().map_or(0.0, |size| size.ascent);
        let last_descent = sizes.last().map_or(0.0, |size| size.descent);
        let bounds = Bounds {
            x: center.0 - width / 2.0,
            y: center.1 - first_ascent,
            width,
            height: first_ascent
                + line_height * arrow.labels.len().saturating_sub(1) as f64
                + last_descent,
        };
        for (line, label) in arrow.labels.iter().enumerate() {
            label_text(
                list,
                arrow.id,
                label,
                (center.0, center.1 + line as f64 * line_height),
                TICK_FONT,
                TextAnchor::Middle,
                0.0,
            );
        }
        if bounds.x < 0.0
            || bounds.y < 0.0
            || bounds.right() > chart.width_pt
            || bounds.bottom() > chart.height_pt
        {
            warnings.push(LayoutWarning::TextOutsideFigure {
                node: arrow.id,
                text: arrow
                    .labels
                    .iter()
                    .map(Label::normalized_text)
                    .collect::<Vec<_>>()
                    .join("\n"),
            });
        }
        hit_map.items.push(HitItem {
            node: arrow.id,
            bounds,
            z_order: 603 + index as u32 * 4,
            role: SelectableRole::MeasurementArrowLabel,
            data_index: None,
            tooltip: Some(
                arrow
                    .labels
                    .iter()
                    .map(Label::normalized_text)
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            path_proximity: vec![center],
        });
    }
    Ok(())
}

fn draw_measurement_arrow_path(
    arrow: &MeasurementArrow,
    start: (f64, f64),
    end: (f64, f64),
    list: &mut DisplayList,
) {
    let (shaft_start, shaft_end, filled_head_size) = if arrow.arrow_head == ArrowHead::Filled {
        trimmed_arrow_shaft(
            start,
            end,
            arrow.arrow_size,
            arrow.start_arrow,
            arrow.end_arrow,
        )
    } else {
        (start, end, arrow.arrow_size)
    };
    let mut verbs = vec![
        PathVerb::MoveTo(pt(shaft_start.0), pt(shaft_start.1)),
        PathVerb::LineTo(pt(shaft_end.0), pt(shaft_end.1)),
    ];
    if arrow.arrow_head == ArrowHead::Open {
        if arrow.start_arrow {
            append_arrow_head(&mut verbs, start, end, arrow.arrow_size);
        }
        if arrow.end_arrow {
            append_arrow_head(&mut verbs, end, start, arrow.arrow_size);
        }
    }
    list.items.push(DisplayItem::Path {
        source: arrow.id,
        path: Path { verbs },
        fill: None,
        stroke: Some(stroke(arrow.color, arrow.stroke.width, arrow.stroke.dash)),
    });
    if arrow.arrow_head == ArrowHead::Filled {
        if arrow.start_arrow {
            draw_filled_arrow_head(list, arrow.id, start, end, filled_head_size, arrow.color);
        }
        if arrow.end_arrow {
            draw_filled_arrow_head(list, arrow.id, end, start, filled_head_size, arrow.color);
        }
    }
}

pub(super) fn trimmed_arrow_shaft(
    start: (f64, f64),
    end: (f64, f64),
    requested_size: f64,
    start_arrow: bool,
    end_arrow: bool,
) -> ((f64, f64), (f64, f64), f64) {
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    let length = dx.hypot(dy);
    let head_count = u8::from(start_arrow) + u8::from(end_arrow);
    if length <= f64::EPSILON || head_count == 0 {
        return (start, end, requested_size.max(0.0));
    }
    let size = requested_size.max(0.0).min(length / f64::from(head_count));
    let ux = dx / length;
    let uy = dy / length;
    let shaft_start = if start_arrow {
        (start.0 + ux * size, start.1 + uy * size)
    } else {
        start
    };
    let shaft_end = if end_arrow {
        (end.0 - ux * size, end.1 - uy * size)
    } else {
        end
    };
    (shaft_start, shaft_end, size)
}

fn draw_filled_arrow_head(
    list: &mut DisplayList,
    source: NodeId,
    tip: (f64, f64),
    away: (f64, f64),
    size: f64,
    color: Color,
) {
    let dx = away.0 - tip.0;
    let dy = away.1 - tip.1;
    let length = dx.hypot(dy);
    if length <= f64::EPSILON {
        return;
    }
    let ux = dx / length;
    let uy = dy / length;
    let wing = size * 0.45;
    let base = (tip.0 + ux * size, tip.1 + uy * size);
    let perpendicular = (-uy * wing, ux * wing);
    list.items.push(DisplayItem::Path {
        source,
        path: polygon(&[
            tip,
            (base.0 + perpendicular.0, base.1 + perpendicular.1),
            (base.0 - perpendicular.0, base.1 - perpendicular.1),
        ]),
        fill: Some(Fill {
            color,
            rule: FillRule::NonZero,
        }),
        stroke: None,
    });
}

pub(super) fn draw_annotations(
    chart: &Chart,
    axes: Bounds,
    canvas_top: f64,
    measurer: &mut dyn TextMeasurer,
    list: &mut DisplayList,
    hit_map: &mut HitMap,
) -> Result<(), LayoutError> {
    for (index, annotation) in chart.annotations.iter().enumerate() {
        let Some((x, y)) = annotation_position(chart, axes, annotation, canvas_top) else {
            return Err(LayoutError::InvalidData(annotation.id));
        };
        let x = x + annotation.offset_pt.0;
        let y = y + annotation.offset_pt.1;
        let sizes = annotation
            .labels
            .iter()
            .map(|label| measurer.measure_label(label, TICK_FONT))
            .collect::<Vec<_>>();
        let line_height = TICK_FONT * 1.25;
        let width = sizes.iter().map(|size| size.width).fold(0.0, f64::max);
        let first_ascent = sizes.first().map_or(0.0, |size| size.ascent);
        let last_descent = sizes.last().map_or(0.0, |size| size.descent);
        let top = y - first_ascent;
        let bottom = y + line_height * (sizes.len().saturating_sub(1)) as f64 + last_descent;
        for (connector_index, connector) in annotation.connectors.iter().enumerate() {
            let Some(target) = map_point(chart, axes, connector.axes, connector.target) else {
                return Err(LayoutError::InvalidData(annotation.id));
            };
            let start = (target.0.clamp(x, x + width), target.1.clamp(top, bottom));
            let (shaft_start, shaft_end, filled_head_size) =
                if connector.arrow_head == ArrowHead::Filled {
                    trimmed_arrow_shaft(
                        start,
                        target,
                        connector.arrow_size,
                        connector.start_arrow,
                        connector.end_arrow,
                    )
                } else {
                    (start, target, connector.arrow_size)
                };
            let mut verbs = vec![
                PathVerb::MoveTo(pt(shaft_start.0), pt(shaft_start.1)),
                PathVerb::LineTo(pt(shaft_end.0), pt(shaft_end.1)),
            ];
            if connector.start_arrow && connector.arrow_head == ArrowHead::Open {
                append_arrow_head(&mut verbs, start, target, connector.arrow_size);
            }
            if connector.end_arrow && connector.arrow_head == ArrowHead::Open {
                append_arrow_head(&mut verbs, target, start, connector.arrow_size);
            }
            list.items.push(DisplayItem::Path {
                source: annotation.id,
                path: Path { verbs },
                fill: None,
                stroke: Some(stroke(
                    connector.color,
                    connector.stroke.width,
                    connector.stroke.dash,
                )),
            });
            if connector.arrow_head == ArrowHead::Filled {
                if connector.start_arrow {
                    draw_filled_arrow_head(
                        list,
                        annotation.id,
                        start,
                        target,
                        filled_head_size,
                        connector.color,
                    );
                }
                if connector.end_arrow {
                    draw_filled_arrow_head(
                        list,
                        annotation.id,
                        target,
                        start,
                        filled_head_size,
                        connector.color,
                    );
                }
            }
            hit_map.items.push(HitItem {
                node: annotation.id,
                bounds: bounds_of_points(
                    &[start, target],
                    (connector.arrow_size + connector.stroke.width / 2.0 + 4.0)
                        .max(connector.stroke.width + 4.0),
                ),
                z_order: 650 + index as u32,
                role: SelectableRole::AnnotationConnector,
                data_index: Some(connector_index),
                tooltip: Some(format!("connector {}", connector_index + 1)),
                path_proximity: vec![start, target],
            });
        }
        for (line, label) in annotation.labels.iter().enumerate() {
            label_text(
                list,
                annotation.id,
                label,
                (x, y + line as f64 * line_height),
                TICK_FONT,
                TextAnchor::Start,
                0.0,
            );
        }
        hit_map.items.push(HitItem {
            node: annotation.id,
            bounds: Bounds {
                x,
                y: top,
                width,
                height: (bottom - top).max(f64::EPSILON),
            },
            z_order: 500 + index as u32,
            role: SelectableRole::Annotation,
            data_index: None,
            tooltip: Some(
                annotation
                    .labels
                    .iter()
                    .map(Label::normalized_text)
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            path_proximity: vec![(x, y)],
        });
    }
    Ok(())
}

fn append_arrow_head(verbs: &mut Vec<PathVerb>, tip: (f64, f64), away: (f64, f64), size: f64) {
    let dx = away.0 - tip.0;
    let dy = away.1 - tip.1;
    let length = dx.hypot(dy);
    if length <= f64::EPSILON {
        return;
    }
    let ux = dx / length;
    let uy = dy / length;
    let wing = size * 0.45;
    let base = (tip.0 + ux * size, tip.1 + uy * size);
    let perpendicular = (-uy * wing, ux * wing);
    verbs.extend([
        PathVerb::MoveTo(pt(base.0 + perpendicular.0), pt(base.1 + perpendicular.1)),
        PathVerb::LineTo(pt(tip.0), pt(tip.1)),
        PathVerb::LineTo(pt(base.0 - perpendicular.0), pt(base.1 - perpendicular.1)),
    ]);
}
