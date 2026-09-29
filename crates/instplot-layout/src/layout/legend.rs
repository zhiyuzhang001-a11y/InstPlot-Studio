use super::*;

#[derive(Clone, Copy)]
pub(super) struct LegendChoice {
    pub(super) bounds: Bounds,
    pub(super) outside: bool,
    pub(super) visible: bool,
    pub(super) position: LegendPosition,
    pub(super) columns: usize,
    pub(super) rows: usize,
    pub(super) column_major: bool,
}

fn legend_grid(
    grid: crate::LegendGrid,
    entries: usize,
    automatic_columns: usize,
) -> (usize, usize, bool) {
    match grid {
        crate::LegendGrid::Auto => {
            let columns = automatic_columns.clamp(1, entries);
            (columns, entries.div_ceil(columns), false)
        }
        crate::LegendGrid::Columns(requested) => {
            let columns = requested.clamp(1, entries);
            (columns, entries.div_ceil(columns), false)
        }
        crate::LegendGrid::Rows(requested) => {
            let rows = requested.clamp(1, entries);
            (entries.div_ceil(rows), rows, true)
        }
    }
}

fn legend_series(chart: &Chart) -> Vec<&Series> {
    let Some(legend) = chart.legend.as_ref() else {
        return Vec::new();
    };
    if legend.entry_order.is_empty() {
        return chart
            .series
            .iter()
            .filter(|series| !series.label.is_empty())
            .collect();
    }
    legend
        .entry_order
        .iter()
        .filter_map(|id| {
            chart
                .series
                .iter()
                .find(|series| series.id == *id && !series.label.is_empty())
        })
        .collect()
}

fn legend_column_widths(
    entries: &[&Series],
    columns: usize,
    rows: usize,
    column_major: bool,
    measurer: &mut dyn TextMeasurer,
) -> Vec<f64> {
    let mut widths = vec![0.0_f64; columns.max(1)];
    for (index, series) in entries.iter().enumerate() {
        let column = if column_major {
            index / rows.max(1)
        } else {
            index % columns.max(1)
        };
        if let Some(width) = widths.get_mut(column) {
            let size = if let Some(label) = &series.legend_label {
                measurer.measure_label(label, LEGEND_FONT)
            } else {
                measurer.measure(&series.label, LEGEND_FONT)
            };
            *width = (*width).max(size.width + 24.0);
        }
    }
    widths
}

pub(super) fn choose_legend(
    chart: &Chart,
    axes: Bounds,
    measurer: &mut dyn TextMeasurer,
    override_position: Option<LegendPosition>,
) -> LegendChoice {
    let Some(legend) = chart.legend.as_ref() else {
        return empty_legend();
    };
    let entries = legend_series(chart);
    if entries.is_empty() {
        return empty_legend();
    }
    let cell_width = entries
        .iter()
        .map(|series| {
            if let Some(label) = &series.legend_label {
                measurer.measure_label(label, LEGEND_FONT).width
            } else {
                measurer.measure(&series.label, LEGEND_FONT).width
            }
        })
        .fold(0.0, f64::max)
        + 24.0;
    let (inside_columns, inside_rows, inside_column_major) =
        legend_grid(legend.grid, entries.len(), 1);
    let inside_width = legend_column_widths(
        &entries,
        inside_columns,
        inside_rows,
        inside_column_major,
        measurer,
    )
    .iter()
    .sum();
    let inside_height = inside_rows as f64 * 12.0 + 6.0;
    let position = override_position.unwrap_or(legend.position);
    if let LegendPosition::FigurePoints { x, y } = position {
        return LegendChoice {
            bounds: Bounds {
                x,
                y,
                width: inside_width,
                height: inside_height,
            },
            outside: false,
            visible: true,
            position,
            columns: inside_columns,
            rows: inside_rows,
            column_major: inside_column_major,
        };
    }
    let mut above = above_legend(chart, axes, &entries, cell_width, legend.grid, measurer);
    let mut right = LegendChoice {
        bounds: Bounds {
            x: axes.right() + 6.0,
            y: axes.y,
            width: inside_width,
            height: inside_height,
        },
        outside: true,
        visible: true,
        position: LegendPosition::Right,
        columns: inside_columns,
        rows: inside_rows,
        column_major: inside_column_major,
    };
    if let Some((x, y)) = legend.manual_position {
        above.bounds.x = x;
        above.bounds.y = y;
        right.bounds.x = x.max(axes.right() + 6.0);
        right.bounds.y = y;
    }
    match position {
        LegendPosition::Above => return above,
        LegendPosition::Right => return right,
        LegendPosition::Auto => {}
        LegendPosition::FigurePoints { .. } => unreachable!(),
    }
    let inset = 6.0;
    let candidates = [
        Bounds {
            x: axes.right() - inside_width - inset,
            y: axes.y + inset,
            width: inside_width,
            height: inside_height,
        },
        Bounds {
            x: axes.x + inset,
            y: axes.y + inset,
            width: inside_width,
            height: inside_height,
        },
        Bounds {
            x: axes.right() - inside_width - inset,
            y: axes.bottom() - inside_height - inset,
            width: inside_width,
            height: inside_height,
        },
        Bounds {
            x: axes.x + inset,
            y: axes.bottom() - inside_height - inset,
            width: inside_width,
            height: inside_height,
        },
    ];
    let occupied = occupancy(chart, axes);
    let extrema = extrema_regions(chart, axes);
    let mut best = (f64::INFINITY, candidates[0]);
    for candidate in candidates {
        let overlap = occupied
            .iter()
            .map(|bounds| candidate.intersection_area(*bounds))
            .sum::<f64>();
        let edge_penalty = 0.01
            * ((candidate.x - axes.x).min(axes.right() - candidate.right())
                + (candidate.y - axes.y).min(axes.bottom() - candidate.bottom()))
            .abs();
        let extrema_penalty = extrema
            .iter()
            .filter(|bounds| candidate.intersection_area(**bounds) > 0.0)
            .count() as f64
            * 25.0;
        let score = overlap + extrema_penalty + edge_penalty;
        if score < best.0 {
            best = (score, candidate);
        }
    }
    if best.0 <= 20.0
        && inside_width + 2.0 * inset <= axes.width
        && inside_height + 2.0 * inset <= axes.height
    {
        LegendChoice {
            bounds: best.1,
            outside: false,
            visible: true,
            position: LegendPosition::Auto,
            columns: inside_columns,
            rows: inside_rows,
            column_major: inside_column_major,
        }
    } else if chart.width_pt * (above.bounds.height + 8.0)
        <= chart.height_pt * (right.bounds.width + 10.0)
    {
        above
    } else {
        right
    }
}

fn empty_legend() -> LegendChoice {
    LegendChoice {
        bounds: Bounds {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        },
        outside: false,
        visible: false,
        position: LegendPosition::Auto,
        columns: 1,
        rows: 1,
        column_major: false,
    }
}

fn above_legend(
    chart: &Chart,
    axes: Bounds,
    entries: &[&Series],
    cell_width: f64,
    grid: crate::LegendGrid,
    measurer: &mut dyn TextMeasurer,
) -> LegendChoice {
    let available_width = (chart.width_pt - axes.x - 12.0).max(1.0);
    let entry_count = entries.len();
    let automatic_columns = ((available_width / cell_width).floor() as usize).clamp(1, entry_count);
    let (columns, rows, column_major) = legend_grid(grid, entry_count, automatic_columns);
    let width = legend_column_widths(entries, columns, rows, column_major, measurer)
        .iter()
        .sum::<f64>();
    let height = rows as f64 * 12.0 + 6.0;
    LegendChoice {
        bounds: Bounds {
            x: axes.x + (available_width - width).max(0.0) / 2.0,
            y: axes.y - height - 6.0,
            width,
            height,
        },
        outside: true,
        visible: true,
        position: LegendPosition::Above,
        columns,
        rows,
        column_major,
    }
}

pub(super) fn draw_legend(
    chart: &Chart,
    choice: LegendChoice,
    measurer: &mut dyn TextMeasurer,
    list: &mut DisplayList,
    hit_map: &mut HitMap,
) {
    let bounds = choice.bounds;
    let legend_id = chart.legend.as_ref().map_or(chart.id, |legend| legend.id);
    let entries = legend_series(chart);
    let column_widths = legend_column_widths(
        &entries,
        choice.columns,
        choice.rows,
        choice.column_major,
        measurer,
    );
    for (index, series) in entries.iter().enumerate() {
        let (column, row) = if choice.column_major {
            (index / choice.rows, index % choice.rows)
        } else {
            (index % choice.columns, index / choice.columns)
        };
        let y = bounds.y + 8.0 + row as f64 * 12.0;
        let column_offset = column_widths.iter().take(column).sum::<f64>();
        let key_start = bounds.x + column_offset + 3.0;
        let key_end = key_start + 14.0;
        if let Some(line) = series.line {
            list.items.push(DisplayItem::Path {
                source: series.id,
                path: Path {
                    verbs: vec![
                        PathVerb::MoveTo(pt(key_start), pt(y)),
                        PathVerb::LineTo(pt(key_end), pt(y)),
                    ],
                },
                fill: None,
                stroke: Some(stroke(series.color, line.width, line.dash)),
            });
        }
        if let Some(marker) = series.legend_marker.or(series.marker) {
            list.items.push(DisplayItem::Path {
                source: series.id,
                path: marker_path((key_start + key_end) / 2.0, y, marker),
                fill: marker_fill(marker, series.color),
                stroke: Some(stroke(
                    series.color,
                    marker_outline_width(marker),
                    DashStyle::Solid,
                )),
            });
        }
        if let Some(error) = series.legend_error {
            let center = (key_start + key_end) / 2.0;
            let marker_height = series
                .legend_marker
                .or(series.marker)
                .map_or(4.0, |marker| marker.size);
            let height = (marker_height + 4.0).clamp(9.0, 10.5);
            let half_height = height / 2.0;
            let half_cap = error.style.cap_width.clamp(3.0, 10.0) / 2.0;
            list.items.push(DisplayItem::Path {
                source: series.id,
                path: Path {
                    verbs: vec![
                        PathVerb::MoveTo(pt(center), pt(y - half_height)),
                        PathVerb::LineTo(pt(center), pt(y + half_height)),
                        PathVerb::MoveTo(pt(center - half_cap), pt(y - half_height)),
                        PathVerb::LineTo(pt(center + half_cap), pt(y - half_height)),
                        PathVerb::MoveTo(pt(center - half_cap), pt(y + half_height)),
                        PathVerb::LineTo(pt(center + half_cap), pt(y + half_height)),
                    ],
                },
                fill: None,
                stroke: Some(stroke(error.color, error.style.width, error.style.dash)),
            });
        }
        if let Some(label) = &series.legend_label {
            let _ = measurer.measure_label(label, LEGEND_FONT);
            label_text(
                list,
                series.id,
                label,
                (key_end + 4.0, y + 2.5),
                LEGEND_FONT,
                TextAnchor::Start,
                0.0,
            );
        } else {
            let _ = measurer.measure(&series.label, LEGEND_FONT);
            text(
                list,
                series.id,
                &series.label,
                (key_end + 4.0, y + 2.5),
                LEGEND_FONT,
                TextAnchor::Start,
                0.0,
            );
        }
    }
    hit_map.items.push(HitItem {
        node: legend_id,
        bounds,
        z_order: 1000,
        role: SelectableRole::Legend,
        data_index: None,
        tooltip: Some("Legend".into()),
        path_proximity: Vec::new(),
    });
}
