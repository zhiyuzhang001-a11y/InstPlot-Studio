use super::*;

pub(super) fn draw_series(
    chart: &Chart,
    axes: Bounds,
    list: &mut DisplayList,
    hit_map: &mut HitMap,
) -> Result<(), LayoutError> {
    list.items.push(DisplayItem::ClipPush {
        source: chart.id,
        x: pt(axes.x),
        y: pt(axes.y),
        width: pt(axes.width),
        height: pt(axes.height),
    });
    let mut z = 100_u32;
    for series in &chart.series {
        let pair = chart
            .series_axes
            .get(&series.id)
            .copied()
            .unwrap_or_default();
        let mapped: Vec<(usize, DataPoint, (f64, f64))> = series
            .points
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(source_index, point)| {
                map_point(chart, axes, pair, point)
                    .map(|mapped_point| (source_index, point, mapped_point))
            })
            .collect();
        let line_points = mapped
            .iter()
            .map(|(_, _, mapped_point)| *mapped_point)
            .collect::<Vec<_>>();
        if let Some(line) = series.line
            && line_points.len() >= 2
        {
            list.items.push(DisplayItem::Path {
                source: series.id,
                path: polyline(&line_points),
                fill: None,
                stroke: Some(stroke(series.color, line.width, line.dash)),
            });
            hit_map.items.push(HitItem {
                node: series.id,
                bounds: bounds_of_points(&line_points, line.width + 3.0),
                z_order: z,
                role: SelectableRole::Series,
                data_index: None,
                tooltip: Some(series.label.clone()),
                path_proximity: line_points,
            });
            z += 1;
        }
        for (drawable_index, (source_index, point, (x, y))) in mapped.iter().enumerate() {
            if let Some(error) = series.errors.get(*source_index) {
                draw_error_bar(
                    chart,
                    axes,
                    pair,
                    series,
                    *source_index,
                    *point,
                    *error,
                    list,
                    hit_map,
                    z,
                )?;
                z += 1;
            }
            if let Some(marker) = series.marker
                && should_draw_marker(*source_index, drawable_index, mapped.len(), marker.interval)
            {
                let path = marker_path(*x, *y, marker);
                list.items.push(DisplayItem::Path {
                    source: series.id,
                    path,
                    fill: marker_fill(marker, series.color),
                    stroke: Some(stroke(
                        series.color,
                        marker_outline_width(marker),
                        DashStyle::Solid,
                    )),
                });
                let radius = marker.size / 2.0 + 2.0;
                hit_map.items.push(HitItem {
                    node: series.id,
                    bounds: Bounds {
                        x: x - radius,
                        y: y - radius,
                        width: radius * 2.0,
                        height: radius * 2.0,
                    },
                    z_order: z,
                    role: SelectableRole::DataPoint,
                    data_index: Some(*source_index),
                    tooltip: Some(format!(
                        "{}: ({:.4}, {:.4})",
                        series.label, point.x, point.y
                    )),
                    path_proximity: vec![(*x, *y)],
                });
                z += 1;
            }
        }
    }
    list.items.push(DisplayItem::ClipPop { source: chart.id });
    Ok(())
}

pub(super) fn should_draw_marker(
    source_index: usize,
    drawable_index: usize,
    drawable_count: usize,
    interval: usize,
) -> bool {
    drawable_count > 0
        && (drawable_index == 0
            || drawable_index + 1 == drawable_count
            || source_index.is_multiple_of(interval.max(1)))
}

#[allow(clippy::too_many_arguments)]
fn draw_error_bar(
    chart: &Chart,
    axes: Bounds,
    pair: AxisPair,
    series: &Series,
    index: usize,
    point: DataPoint,
    error: crate::model::ErrorBar,
    list: &mut DisplayList,
    hit_map: &mut HitMap,
    z: u32,
) -> Result<(), LayoutError> {
    let left = map_point(
        chart,
        axes,
        pair,
        DataPoint {
            x: point.x - error.x_minus,
            y: point.y,
        },
    );
    let right = map_point(
        chart,
        axes,
        pair,
        DataPoint {
            x: point.x + error.x_plus,
            y: point.y,
        },
    );
    let top = map_point(
        chart,
        axes,
        pair,
        DataPoint {
            x: point.x,
            y: point.y + error.y_plus,
        },
    );
    let bottom = map_point(
        chart,
        axes,
        pair,
        DataPoint {
            x: point.x,
            y: point.y - error.y_minus,
        },
    );
    let Some((left, right, top, bottom)) = left
        .zip(right)
        .zip(top)
        .zip(bottom)
        .map(|(((left, right), top), bottom)| (left, right, top, bottom))
    else {
        return Err(LayoutError::InvalidData(series.id));
    };
    let style = series.error_style.unwrap_or(crate::model::ErrorStyle {
        width: FALLBACK_ERROR_BAR_WIDTH_PT,
        cap_width: 4.0,
        dash: DashStyle::Solid,
    });
    let cap = style.cap_width / 2.0;
    let path = Path {
        verbs: vec![
            PathVerb::MoveTo(pt(left.0), pt(left.1)),
            PathVerb::LineTo(pt(right.0), pt(right.1)),
            PathVerb::MoveTo(pt(left.0), pt(left.1 - cap)),
            PathVerb::LineTo(pt(left.0), pt(left.1 + cap)),
            PathVerb::MoveTo(pt(right.0), pt(right.1 - cap)),
            PathVerb::LineTo(pt(right.0), pt(right.1 + cap)),
            PathVerb::MoveTo(pt(top.0), pt(top.1)),
            PathVerb::LineTo(pt(bottom.0), pt(bottom.1)),
            PathVerb::MoveTo(pt(top.0 - cap), pt(top.1)),
            PathVerb::LineTo(pt(top.0 + cap), pt(top.1)),
            PathVerb::MoveTo(pt(bottom.0 - cap), pt(bottom.1)),
            PathVerb::LineTo(pt(bottom.0 + cap), pt(bottom.1)),
        ],
    };
    list.items.push(DisplayItem::Path {
        source: series.id,
        path,
        fill: None,
        stroke: Some(stroke(series.color, style.width, style.dash)),
    });
    hit_map.items.push(HitItem {
        node: series.id,
        bounds: Bounds {
            x: left.0.min(top.0).min(bottom.0) - cap,
            y: top.1.min(left.1).min(right.1) - cap,
            width: right.0.max(top.0).max(bottom.0) - left.0.min(top.0).min(bottom.0) + cap * 2.0,
            height: bottom.1.max(left.1).max(right.1) - top.1.min(left.1).min(right.1) + cap * 2.0,
        },
        z_order: z,
        role: SelectableRole::ErrorBar,
        data_index: Some(index),
        tooltip: None,
        path_proximity: vec![left, right, top, bottom],
    });
    Ok(())
}
