use core::fmt;

use instplot_render::{
    Color, DisplayItem, DisplayList, Fill, FillRule, GlyphRun, LineCap, LineJoin, NodeId, Path,
    PathVerb, Pt, Stroke, TextAnchor,
};
use instplot_text::Label;

use crate::model::{
    Annotation, AnnotationPosition, Chart, DashStyle, DataPoint, LegendPosition, MarkerShape,
    MarkerStyle, Series, TickDirection,
};
use crate::scale::{Scale, collision_stride, minor_ticks_with_interval};
use crate::text::{ParleyMeasurer, TextMeasurer, TextSize};

const TICK_FONT: f64 = 8.0;
const LABEL_FONT: f64 = 9.0;
const LEGEND_FONT: f64 = 8.0;
const MAX_ITERATIONS: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Bounds {
    pub fn right(self) -> f64 {
        self.x + self.width
    }

    pub fn bottom(self) -> f64 {
        self.y + self.height
    }

    pub fn intersection_area(self, other: Self) -> f64 {
        let width = (self.right().min(other.right()) - self.x.max(other.x)).max(0.0);
        let height = (self.bottom().min(other.bottom()) - self.y.max(other.y)).max(0.0);
        width * height
    }

    pub fn contains(self, other: Self) -> bool {
        other.x >= self.x
            && other.y >= self.y
            && other.right() <= self.right()
            && other.bottom() <= self.bottom()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SelectableRole {
    Axes,
    Axis,
    AxisLabel,
    Tick,
    Series,
    DataPoint,
    ErrorBar,
    Annotation,
    AnnotationConnector,
    Legend,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HitItem {
    pub node: NodeId,
    pub bounds: Bounds,
    pub z_order: u32,
    pub role: SelectableRole,
    pub data_index: Option<usize>,
    pub tooltip: Option<String>,
    pub path_proximity: Vec<(f64, f64)>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HitMap {
    pub items: Vec<HitItem>,
}

impl HitMap {
    pub fn hit_test(&self, x: f64, y: f64, tolerance: f64) -> Option<&HitItem> {
        self.hit_candidates(x, y, tolerance).into_iter().next()
    }

    pub fn hit_candidates(&self, x: f64, y: f64, tolerance: f64) -> Vec<&HitItem> {
        let mut matches: Vec<_> = self
            .items
            .iter()
            .filter(|item| {
                let expanded = Bounds {
                    x: item.bounds.x - tolerance,
                    y: item.bounds.y - tolerance,
                    width: item.bounds.width + tolerance * 2.0,
                    height: item.bounds.height + tolerance * 2.0,
                };
                let point_bounds = Bounds {
                    x,
                    y,
                    width: 0.0,
                    height: 0.0,
                };
                if !expanded.contains(point_bounds) {
                    return false;
                }
                item.role != SelectableRole::Series
                    || distance_to_polyline(x, y, &item.path_proximity) <= tolerance
            })
            .collect();
        matches.sort_by_key(|item: &&HitItem| std::cmp::Reverse(item.z_order));
        matches
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TickLayout {
    pub value: f64,
    pub label: String,
    pub position: f64,
    pub label_bounds: Bounds,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AxisLayout {
    pub major: Vec<TickLayout>,
    pub minor: Vec<f64>,
    pub shared_exponent: Option<i32>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LayoutWarning {
    NonConvergent { node: NodeId, iterations: usize },
    LegendMovedOutside { node: NodeId },
    TextOutsideFigure { node: NodeId, text: String },
    InsufficientPlotArea { node: NodeId },
}

#[derive(Clone, Debug)]
pub struct LayoutResult {
    pub display_list: DisplayList,
    pub axes: Bounds,
    pub x_axis: AxisLayout,
    pub y_axis: AxisLayout,
    pub x_label_bounds: Bounds,
    pub y_label_bounds: Bounds,
    pub legend: Option<Bounds>,
    pub hit_map: HitMap,
    pub warnings: Vec<LayoutWarning>,
    pub iterations: usize,
}

impl LayoutResult {
    pub fn snapshot(&self) -> String {
        use core::fmt::Write;

        let mut output = String::new();
        writeln!(
            output,
            "AXES {:.3} {:.3} {:.3} {:.3} iterations={}",
            self.axes.x, self.axes.y, self.axes.width, self.axes.height, self.iterations
        )
        .unwrap();
        for (name, axis) in [("X", &self.x_axis), ("Y", &self.y_axis)] {
            writeln!(output, "{name} exponent={:?}", axis.shared_exponent).unwrap();
            for tick in &axis.major {
                writeln!(
                    output,
                    "  {:.6} {:?} pos={:.3} bounds={:.3},{:.3},{:.3},{:.3}",
                    tick.value,
                    tick.label,
                    tick.position,
                    tick.label_bounds.x,
                    tick.label_bounds.y,
                    tick.label_bounds.width,
                    tick.label_bounds.height
                )
                .unwrap();
            }
            writeln!(output, "  minor={}", axis.minor.len()).unwrap();
        }
        if let Some(legend) = self.legend {
            writeln!(
                output,
                "LEGEND {:.3} {:.3} {:.3} {:.3}",
                legend.x, legend.y, legend.width, legend.height
            )
            .unwrap();
        }
        writeln!(
            output,
            "DISPLAY items={} HIT items={} WARNINGS {:?}",
            self.display_list.items.len(),
            self.hit_map.items.len(),
            self.warnings
        )
        .unwrap();
        output
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LayoutError {
    InvalidFigure,
    InvalidAxis(NodeId),
    InvalidData(NodeId),
}

impl fmt::Display for LayoutError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFigure => {
                output.write_str("figure dimensions must be finite and positive")
            }
            Self::InvalidAxis(node) => write!(output, "axis {} has an invalid scale range", node.0),
            Self::InvalidData(node) => write!(output, "node {} contains invalid data", node.0),
        }
    }
}

impl std::error::Error for LayoutError {}

#[derive(Clone, Copy, Debug)]
struct Margins {
    left: f64,
    right: f64,
    top: f64,
    bottom: f64,
    canvas_right: f64,
    canvas_top: f64,
    canvas_bottom: f64,
}

pub fn layout(chart: &Chart) -> Result<LayoutResult, LayoutError> {
    let mut measurer = ParleyMeasurer::default();
    layout_with_measurer(chart, &mut measurer)
}

pub fn layout_with_measurer(
    chart: &Chart,
    measurer: &mut dyn TextMeasurer,
) -> Result<LayoutResult, LayoutError> {
    validate(chart)?;
    let mut margins = Margins {
        left: 38.0,
        right: 14.0,
        top: 14.0,
        bottom: 34.0,
        canvas_right: 0.0,
        canvas_top: 0.0,
        canvas_bottom: 0.0,
    };
    // Freeze automatic placement for this layout pass. Reconsidering it after
    // reserving space can alternate between an inside and an outside choice.
    let auto_position = chart.legend.as_ref().and_then(|legend| {
        if !matches!(legend.position, LegendPosition::Auto) {
            return None;
        }
        let preliminary = choose_legend(chart, axes_from_margins(chart, margins), measurer, None);
        Some(if preliminary.outside {
            preliminary.position
        } else {
            LegendPosition::FigurePoints {
                x: preliminary.bounds.x,
                y: preliminary.bounds.y,
            }
        })
    });
    let mut converged = false;
    let mut iterations = 0;
    for iteration in 1..=MAX_ITERATIONS {
        iterations = iteration;
        let axes = axes_from_margins(chart, margins);
        let x = axis_layout(chart, true, axes, measurer);
        let y = axis_layout(chart, false, axes, measurer);
        let legend = choose_legend(chart, axes, measurer, auto_position);
        let y_label = measurer.measure_label(&chart.y.label, LABEL_FONT);
        let x_label = measurer.measure_label(&chart.x.label, LABEL_FONT);
        let max_y_tick = y
            .major
            .iter()
            .map(|tick| tick.label_bounds.width)
            .fold(0.0, f64::max);
        let tick_height = x
            .major
            .iter()
            .map(|tick| tick.label_bounds.height)
            .fold(0.0, f64::max);
        let mut next = Margins {
            left: chart.y.appearance.label_edge_pad_pt
                + y_label.height
                + chart.y.appearance.label_tick_pad_pt
                + if chart.y.appearance.near_tick_labels {
                    max_y_tick + chart.y.appearance.tick_label_pad_pt
                } else {
                    0.0
                },
            right: if chart.y.appearance.far_tick_labels {
                max_y_tick + chart.y.appearance.tick_label_pad_pt + 6.0
            } else {
                12.0
            },
            top: if chart.x.appearance.far_tick_labels {
                tick_height + chart.x.appearance.tick_label_pad_pt + 6.0
            } else {
                12.0
            },
            bottom: chart.x.appearance.label_edge_pad_pt
                + x_label.height
                + chart.x.appearance.label_tick_pad_pt
                + if chart.x.appearance.near_tick_labels {
                    tick_height + chart.x.appearance.tick_label_pad_pt
                } else {
                    0.0
                },
            canvas_right: 0.0,
            canvas_top: 0.0,
            canvas_bottom: 0.0,
        };
        if legend.outside {
            match legend.position {
                LegendPosition::Above => {
                    next.canvas_top = if chart
                        .legend
                        .as_ref()
                        .is_some_and(|spec| spec.manual_position.is_some())
                    {
                        (legend.bounds.bottom() + 6.0 - next.top).max(0.0)
                    } else {
                        legend.bounds.height + 6.0
                    };
                    next.top += next.canvas_top;
                    next.canvas_right = (legend.bounds.right() + 3.0 - chart.width_pt).max(0.0);
                    next.right += next.canvas_right;
                }
                LegendPosition::Right => {
                    next.canvas_right = (legend.bounds.right() + 3.0 - chart.width_pt).max(0.0);
                    next.right += next.canvas_right;
                    next.canvas_bottom = (legend.bounds.bottom() + 3.0 - chart.height_pt).max(0.0);
                    next.bottom += next.canvas_bottom;
                }
                _ => {}
            }
        } else if legend.visible {
            next.canvas_right = (legend.bounds.right() - chart.width_pt).max(0.0);
            next.right += next.canvas_right;
            next.canvas_bottom = (legend.bounds.bottom() - chart.height_pt).max(0.0);
            next.bottom += next.canvas_bottom;
        }
        if margins_close(margins, next) {
            margins = next;
            converged = true;
            break;
        }
        margins = next;
    }

    let axes = axes_from_margins(chart, margins);
    let x_axis = axis_layout(chart, true, axes, measurer);
    let y_axis = axis_layout(chart, false, axes, measurer);
    let legend_choice = choose_legend(chart, axes, measurer, auto_position);
    let mut warnings = Vec::new();
    if !converged {
        warnings.push(LayoutWarning::NonConvergent {
            node: chart.id,
            iterations,
        });
    }
    if legend_choice.outside {
        warnings.push(LayoutWarning::LegendMovedOutside { node: chart.id });
    }
    if chart.width_pt + margins.canvas_right - margins.left - margins.right < 20.0
        || chart.height_pt + margins.canvas_top + margins.canvas_bottom
            - margins.top
            - margins.bottom
            < 20.0
    {
        warnings.push(LayoutWarning::InsufficientPlotArea { node: chart.id });
    }

    let mut display_list = DisplayList {
        width: pt(chart.width_pt + margins.canvas_right),
        height: pt(chart.height_pt + margins.canvas_top + margins.canvas_bottom),
        items: Vec::new(),
    };
    let mut hit_map = HitMap::default();
    hit_map.items.push(HitItem {
        node: chart.id,
        bounds: axes,
        z_order: 0,
        role: SelectableRole::Axes,
        data_index: None,
        tooltip: None,
        path_proximity: Vec::new(),
    });

    let (x_label_bounds, y_label_bounds) = draw_axes(
        chart,
        axes,
        &x_axis,
        &y_axis,
        margins.canvas_bottom,
        measurer,
        &mut display_list,
        &mut hit_map,
        &mut warnings,
    );
    draw_series(chart, axes, &mut display_list, &mut hit_map)?;
    draw_annotations(
        chart,
        axes,
        margins.canvas_top,
        measurer,
        &mut display_list,
        &mut hit_map,
    )?;
    if legend_choice.visible {
        draw_legend(
            chart,
            legend_choice,
            measurer,
            &mut display_list,
            &mut hit_map,
        );
    }

    Ok(LayoutResult {
        display_list,
        axes,
        x_axis,
        y_axis,
        x_label_bounds,
        y_label_bounds,
        legend: legend_choice.visible.then_some(legend_choice.bounds),
        hit_map,
        warnings,
        iterations,
    })
}

fn validate(chart: &Chart) -> Result<(), LayoutError> {
    if !chart.width_pt.is_finite()
        || !chart.height_pt.is_finite()
        || chart.width_pt <= 0.0
        || chart.height_pt <= 0.0
    {
        return Err(LayoutError::InvalidFigure);
    }
    for axis in [&chart.x, &chart.y] {
        let valid = axis.minimum.is_finite()
            && axis.maximum.is_finite()
            && axis.minimum < axis.maximum
            && (axis.scale != Scale::Log10 || axis.minimum > 0.0)
            && [
                axis.appearance.tick_label_pad_pt,
                axis.appearance.label_edge_pad_pt,
                axis.appearance.label_tick_pad_pt,
            ]
            .into_iter()
            .all(|value| value.is_finite() && value >= 0.0);
        if !valid {
            return Err(LayoutError::InvalidAxis(axis.id));
        }
    }
    for series in &chart.series {
        if series.points.iter().any(|point| {
            !point.x.is_finite()
                || !point.y.is_finite()
                || (chart.x.scale == Scale::Log10 && point.x <= 0.0)
                || (chart.y.scale == Scale::Log10 && point.y <= 0.0)
        }) || series.errors.iter().any(|error| {
            [error.x_minus, error.x_plus, error.y_minus, error.y_plus]
                .into_iter()
                .any(|value| !value.is_finite() || value < 0.0)
        }) {
            return Err(LayoutError::InvalidData(series.id));
        }
        if let Some(style) = series.error_style
            && (!style.width.is_finite()
                || style.width <= 0.0
                || !style.cap_width.is_finite()
                || style.cap_width < 0.0)
        {
            return Err(LayoutError::InvalidData(series.id));
        }
    }
    for annotation in &chart.annotations {
        let valid = match annotation.position {
            AnnotationPosition::Data(point) => {
                point.x.is_finite()
                    && point.y.is_finite()
                    && (chart.x.scale != Scale::Log10 || point.x > 0.0)
                    && (chart.y.scale != Scale::Log10 || point.y > 0.0)
            }
            AnnotationPosition::FigurePoints { x, y } => x.is_finite() && y.is_finite(),
        };
        let connectors_valid = annotation.connectors.iter().all(|connector| {
            connector.target.x.is_finite()
                && connector.target.y.is_finite()
                && (chart.x.scale != Scale::Log10 || connector.target.x > 0.0)
                && (chart.y.scale != Scale::Log10 || connector.target.y > 0.0)
                && connector.stroke.width.is_finite()
                && connector.stroke.width > 0.0
                && connector.arrow_size.is_finite()
                && (2.0..=18.0).contains(&connector.arrow_size)
        });
        if !valid || annotation.labels.is_empty() || !connectors_valid {
            return Err(LayoutError::InvalidData(annotation.id));
        }
    }
    if let Some(legend) = chart.legend.as_ref() {
        if let LegendPosition::FigurePoints { x, y } = legend.position
            && (!x.is_finite() || !y.is_finite())
        {
            return Err(LayoutError::InvalidData(legend.id));
        }
        if let Some((x, y)) = legend.manual_position
            && (!x.is_finite() || !y.is_finite() || x < 0.0 || y < 0.0)
        {
            return Err(LayoutError::InvalidData(legend.id));
        }
    }
    Ok(())
}

fn axes_from_margins(chart: &Chart, margins: Margins) -> Bounds {
    Bounds {
        x: margins.left,
        y: margins.top,
        width: (chart.width_pt + margins.canvas_right - margins.left - margins.right).max(20.0),
        height: (chart.height_pt + margins.canvas_top + margins.canvas_bottom
            - margins.top
            - margins.bottom)
            .max(20.0),
    }
}

fn margins_close(first: Margins, second: Margins) -> bool {
    (first.left - second.left).abs() < 0.05
        && (first.right - second.right).abs() < 0.05
        && (first.top - second.top).abs() < 0.05
        && (first.bottom - second.bottom).abs() < 0.05
        && (first.canvas_right - second.canvas_right).abs() < 0.05
        && (first.canvas_top - second.canvas_top).abs() < 0.05
        && (first.canvas_bottom - second.canvas_bottom).abs() < 0.05
}

fn axis_layout(
    chart: &Chart,
    horizontal: bool,
    axes: Bounds,
    measurer: &mut dyn TextMeasurer,
) -> AxisLayout {
    let axis = if horizontal { &chart.x } else { &chart.y };
    let length = if horizontal { axes.width } else { axes.height };
    let values = axis
        .locator
        .major_ticks(axis.scale, axis.minimum, axis.maximum, length);
    let step = values.windows(2).next().map(|pair| pair[1] - pair[0]);
    let formatted = crate::format_ticks_with(&values, step, &axis.formatter);
    let positions: Vec<f64> = values
        .iter()
        .map(|value| {
            let fraction = axis
                .scale
                .map(*value, axis.minimum, axis.maximum)
                .unwrap_or(0.0);
            if horizontal {
                axes.x + fraction * axes.width
            } else {
                axes.bottom() - fraction * axes.height
            }
        })
        .collect();
    let sizes: Vec<TextSize> = formatted
        .labels
        .iter()
        .map(|label| measurer.measure(label, TICK_FONT))
        .collect();
    let stride = if horizontal {
        collision_stride(
            &positions,
            &sizes.iter().map(|size| size.width).collect::<Vec<_>>(),
            3.0,
        )
    } else {
        1
    };
    let major = values
        .iter()
        .zip(formatted.labels.iter())
        .zip(positions.iter().zip(sizes.iter()))
        .enumerate()
        .filter(|(index, _)| index % stride == 0)
        .map(|(_, ((value, label), (position, size)))| TickLayout {
            value: *value,
            label: label.clone(),
            position: *position,
            label_bounds: if horizontal {
                Bounds {
                    x: position - size.width / 2.0,
                    y: axes.bottom() + axis.appearance.tick_label_pad_pt,
                    width: size.width,
                    height: size.height,
                }
            } else {
                Bounds {
                    x: axes.x - axis.appearance.tick_label_pad_pt - size.width,
                    y: position - size.height / 2.0,
                    width: size.width,
                    height: size.height,
                }
            },
        })
        .collect();
    AxisLayout {
        major,
        minor: minor_ticks_with_interval(
            axis.scale,
            &values,
            axis.minimum,
            axis.maximum,
            axis.minor_interval,
        ),
        shared_exponent: formatted.shared_exponent,
    }
}

#[derive(Clone, Copy)]
struct LegendChoice {
    bounds: Bounds,
    outside: bool,
    visible: bool,
    position: LegendPosition,
    columns: usize,
    rows: usize,
    column_major: bool,
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

fn choose_legend(
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

fn extrema_regions(chart: &Chart, axes: Bounds) -> Vec<Bounds> {
    let points: Vec<(f64, f64)> = chart
        .series
        .iter()
        .flat_map(|series| series.points.iter())
        .filter_map(|point| map_point(chart, axes, *point))
        .collect();
    let mut selected = Vec::new();
    for (x, y) in [
        points.iter().min_by(|a, b| a.0.total_cmp(&b.0)),
        points.iter().max_by(|a, b| a.0.total_cmp(&b.0)),
        points.iter().min_by(|a, b| a.1.total_cmp(&b.1)),
        points.iter().max_by(|a, b| a.1.total_cmp(&b.1)),
    ]
    .into_iter()
    .flatten()
    {
        let bounds = Bounds {
            x: x - 4.0,
            y: y - 4.0,
            width: 8.0,
            height: 8.0,
        };
        if !selected.contains(&bounds) {
            selected.push(bounds);
        }
    }
    selected
}

fn occupancy(chart: &Chart, axes: Bounds) -> Vec<Bounds> {
    let mut output = Vec::new();
    for series in &chart.series {
        for (index, point) in series.points.iter().enumerate() {
            if let Some((x, y)) = map_point(chart, axes, *point) {
                let mut radius: f64 = 3.0;
                if let Some(marker) = series.marker {
                    radius = radius.max(marker.size / 2.0);
                }
                if let Some(error) = series.errors.get(index)
                    && let Some((left, top)) = map_point(
                        chart,
                        axes,
                        DataPoint {
                            x: point.x - error.x_minus,
                            y: point.y + error.y_plus,
                        },
                    )
                    && let Some((right, bottom)) = map_point(
                        chart,
                        axes,
                        DataPoint {
                            x: point.x + error.x_plus,
                            y: point.y - error.y_minus,
                        },
                    )
                {
                    output.push(Bounds {
                        x: left.min(right) - 2.0,
                        y: top.min(bottom) - 2.0,
                        width: (right - left).abs() + 4.0,
                        height: (bottom - top).abs() + 4.0,
                    });
                }
                output.push(Bounds {
                    x: x - radius,
                    y: y - radius,
                    width: radius * 2.0,
                    height: radius * 2.0,
                });
            }
        }
        if series.line.is_some() {
            for segment in series.points.windows(2) {
                let (Some(start), Some(end)) = (
                    map_point(chart, axes, segment[0]),
                    map_point(chart, axes, segment[1]),
                ) else {
                    continue;
                };
                let distance = (end.0 - start.0).hypot(end.1 - start.1);
                let samples = (distance / 12.0).ceil().clamp(1.0, 8.0) as usize;
                for step in 1..samples {
                    let fraction = step as f64 / samples as f64;
                    output.push(Bounds {
                        x: start.0 + (end.0 - start.0) * fraction - 2.0,
                        y: start.1 + (end.1 - start.1) * fraction - 2.0,
                        width: 4.0,
                        height: 4.0,
                    });
                }
            }
        }
    }
    for annotation in &chart.annotations {
        if let Some((x, y)) = annotation_position(chart, axes, annotation, 0.0) {
            output.push(Bounds {
                x: x + annotation.offset_pt.0,
                y: y + annotation.offset_pt.1 - 10.0,
                width: 42.0,
                height: 12.0,
            });
        }
    }
    output
}

#[allow(clippy::too_many_arguments)]
fn draw_axes(
    chart: &Chart,
    axes: Bounds,
    x_axis: &AxisLayout,
    y_axis: &AxisLayout,
    canvas_bottom: f64,
    measurer: &mut dyn TextMeasurer,
    list: &mut DisplayList,
    hit_map: &mut HitMap,
    warnings: &mut Vec<LayoutWarning>,
) -> (Bounds, Bounds) {
    draw_grid(chart, axes, x_axis, y_axis, list);
    let spine = stroke(Color(45, 50, 55, 255), 0.6, DashStyle::Solid);
    if chart.x.appearance.near_spine {
        grid_line(
            list,
            chart.id,
            (axes.x, axes.bottom()),
            (axes.right(), axes.bottom()),
            &spine,
        );
    }
    if chart.x.appearance.far_spine {
        grid_line(
            list,
            chart.id,
            (axes.x, axes.y),
            (axes.right(), axes.y),
            &spine,
        );
    }
    if chart.y.appearance.near_spine {
        grid_line(
            list,
            chart.id,
            (axes.x, axes.y),
            (axes.x, axes.bottom()),
            &spine,
        );
    }
    if chart.y.appearance.far_spine {
        grid_line(
            list,
            chart.id,
            (axes.right(), axes.y),
            (axes.right(), axes.bottom()),
            &spine,
        );
    }
    for tick in &x_axis.major {
        if chart.x.appearance.major_ticks && chart.x.appearance.near_ticks {
            directional_tick(
                list,
                chart.x.id,
                tick.position,
                axes.bottom(),
                false,
                -1.0,
                4.0,
                chart.x.appearance.tick_direction,
            );
        }
        if chart.x.appearance.major_ticks && chart.x.appearance.far_ticks {
            directional_tick(
                list,
                chart.x.id,
                tick.position,
                axes.y,
                false,
                1.0,
                4.0,
                chart.x.appearance.tick_direction,
            );
        }
        let size = measurer.measure(&tick.label, TICK_FONT);
        if chart.x.appearance.near_tick_labels {
            text(
                list,
                chart.x.id,
                &tick.label,
                (tick.position, tick.label_bounds.y + size.ascent),
                TICK_FONT,
                TextAnchor::Middle,
                0.0,
            );
            hit_map.items.push(HitItem {
                node: chart.x.id,
                bounds: tick.label_bounds,
                z_order: 20,
                role: SelectableRole::Tick,
                data_index: None,
                tooltip: Some(format!("x = {}", tick.value)),
                path_proximity: vec![(tick.position, axes.bottom())],
            });
        }
        if chart.x.appearance.far_tick_labels {
            let bounds = Bounds {
                x: tick.position - size.width / 2.0,
                y: axes.y - chart.x.appearance.tick_label_pad_pt - size.height,
                width: size.width,
                height: size.height,
            };
            text(
                list,
                chart.x.id,
                &tick.label,
                (tick.position, bounds.y + size.ascent),
                TICK_FONT,
                TextAnchor::Middle,
                0.0,
            );
            hit_map.items.push(HitItem {
                node: chart.x.id,
                bounds,
                z_order: 20,
                role: SelectableRole::Tick,
                data_index: None,
                tooltip: Some(format!("x = {}", tick.value)),
                path_proximity: vec![(tick.position, axes.y)],
            });
        }
    }
    for value in &x_axis.minor {
        if let Some(fraction) = chart.x.scale.map(*value, chart.x.minimum, chart.x.maximum) {
            if chart.x.appearance.minor_ticks && chart.x.appearance.near_ticks {
                directional_tick(
                    list,
                    chart.x.id,
                    axes.x + fraction * axes.width,
                    axes.bottom(),
                    false,
                    -1.0,
                    2.0,
                    chart.x.appearance.tick_direction,
                );
            }
            if chart.x.appearance.minor_ticks && chart.x.appearance.far_ticks {
                directional_tick(
                    list,
                    chart.x.id,
                    axes.x + fraction * axes.width,
                    axes.y,
                    false,
                    1.0,
                    2.0,
                    chart.x.appearance.tick_direction,
                );
            }
        }
    }
    for tick in &y_axis.major {
        if chart.y.appearance.major_ticks && chart.y.appearance.near_ticks {
            directional_tick(
                list,
                chart.y.id,
                axes.x,
                tick.position,
                true,
                1.0,
                4.0,
                chart.y.appearance.tick_direction,
            );
        }
        if chart.y.appearance.major_ticks && chart.y.appearance.far_ticks {
            directional_tick(
                list,
                chart.y.id,
                axes.right(),
                tick.position,
                true,
                -1.0,
                4.0,
                chart.y.appearance.tick_direction,
            );
        }
        let size = measurer.measure(&tick.label, TICK_FONT);
        if chart.y.appearance.near_tick_labels {
            text(
                list,
                chart.y.id,
                &tick.label,
                (
                    tick.label_bounds.x,
                    tick.position + (size.ascent - size.descent) / 2.0,
                ),
                TICK_FONT,
                TextAnchor::Start,
                0.0,
            );
            hit_map.items.push(HitItem {
                node: chart.y.id,
                bounds: tick.label_bounds,
                z_order: 20,
                role: SelectableRole::Tick,
                data_index: None,
                tooltip: Some(format!("y = {}", tick.value)),
                path_proximity: vec![(axes.x, tick.position)],
            });
        }
        if chart.y.appearance.far_tick_labels {
            let bounds = Bounds {
                x: axes.right() + chart.y.appearance.tick_label_pad_pt,
                y: tick.position - size.height / 2.0,
                width: size.width,
                height: size.height,
            };
            text(
                list,
                chart.y.id,
                &tick.label,
                (bounds.x, tick.position + (size.ascent - size.descent) / 2.0),
                TICK_FONT,
                TextAnchor::Start,
                0.0,
            );
            hit_map.items.push(HitItem {
                node: chart.y.id,
                bounds,
                z_order: 20,
                role: SelectableRole::Tick,
                data_index: None,
                tooltip: Some(format!("y = {}", tick.value)),
                path_proximity: vec![(axes.right(), tick.position)],
            });
        }
    }
    for value in &y_axis.minor {
        if let Some(fraction) = chart.y.scale.map(*value, chart.y.minimum, chart.y.maximum) {
            if chart.y.appearance.minor_ticks && chart.y.appearance.near_ticks {
                directional_tick(
                    list,
                    chart.y.id,
                    axes.x,
                    axes.bottom() - fraction * axes.height,
                    true,
                    1.0,
                    2.0,
                    chart.y.appearance.tick_direction,
                );
            }
            if chart.y.appearance.minor_ticks && chart.y.appearance.far_ticks {
                directional_tick(
                    list,
                    chart.y.id,
                    axes.right(),
                    axes.bottom() - fraction * axes.height,
                    true,
                    -1.0,
                    2.0,
                    chart.y.appearance.tick_direction,
                );
            }
        }
    }

    let x_size = measurer.measure_label(&chart.x.label, LABEL_FONT);
    let x_bounds = Bounds {
        x: axes.x + (axes.width - x_size.width) / 2.0,
        y: list.height.get() - canvas_bottom - chart.x.appearance.label_edge_pad_pt - x_size.height,
        width: x_size.width,
        height: x_size.height,
    };
    label_text(
        list,
        chart.x.id,
        &chart.x.label,
        (axes.x + axes.width / 2.0, x_bounds.y + x_size.ascent),
        LABEL_FONT,
        TextAnchor::Middle,
        0.0,
    );
    let y_size = measurer.measure_label(&chart.y.label, LABEL_FONT);
    let y_bounds = Bounds {
        x: chart.y.appearance.label_edge_pad_pt,
        y: axes.y + (axes.height - y_size.width) / 2.0,
        width: y_size.height,
        height: y_size.width,
    };
    label_text(
        list,
        chart.y.id,
        &chart.y.label,
        (
            y_bounds.right() - y_size.descent,
            axes.y + axes.height / 2.0,
        ),
        LABEL_FONT,
        TextAnchor::Middle,
        -90.0,
    );
    let figure = Bounds {
        x: 0.0,
        y: 0.0,
        width: list.width.get(),
        height: list.height.get(),
    };
    for (node, value, bounds) in [
        (chart.x.id, &chart.x.label, x_bounds),
        (chart.y.id, &chart.y.label, y_bounds),
    ] {
        if !figure.contains(bounds) {
            warnings.push(LayoutWarning::TextOutsideFigure {
                node,
                text: value.normalized_text(),
            });
        }
    }
    for (node, bounds) in [(chart.x.id, x_bounds), (chart.y.id, y_bounds)] {
        hit_map.items.push(HitItem {
            node,
            bounds,
            z_order: 22,
            role: SelectableRole::AxisLabel,
            data_index: None,
            tooltip: None,
            path_proximity: Vec::new(),
        });
    }
    if chart.x.appearance.near_spine {
        hit_map.items.push(axis_hit_item(
            chart.x.id,
            Bounds {
                x: axes.x,
                y: axes.bottom(),
                width: axes.width,
                height: 0.0,
            },
        ));
    }
    if chart.x.appearance.far_spine {
        hit_map.items.push(axis_hit_item(
            chart.x.id,
            Bounds {
                x: axes.x,
                y: axes.y,
                width: axes.width,
                height: 0.0,
            },
        ));
    }
    if chart.y.appearance.near_spine {
        hit_map.items.push(axis_hit_item(
            chart.y.id,
            Bounds {
                x: axes.x,
                y: axes.y,
                width: 0.0,
                height: axes.height,
            },
        ));
    }
    if chart.y.appearance.far_spine {
        hit_map.items.push(axis_hit_item(
            chart.y.id,
            Bounds {
                x: axes.right(),
                y: axes.y,
                width: 0.0,
                height: axes.height,
            },
        ));
    }
    (x_bounds, y_bounds)
}

fn axis_hit_item(node: NodeId, bounds: Bounds) -> HitItem {
    HitItem {
        node,
        bounds,
        z_order: 18,
        role: SelectableRole::Axis,
        data_index: None,
        tooltip: None,
        path_proximity: Vec::new(),
    }
}

fn draw_grid(
    chart: &Chart,
    axes: Bounds,
    x_axis: &AxisLayout,
    y_axis: &AxisLayout,
    list: &mut DisplayList,
) {
    let major = stroke(Color(218, 221, 224, 255), 0.4, DashStyle::Solid);
    let minor = stroke(Color(232, 234, 236, 255), 0.3, DashStyle::Dotted);
    if chart.x.grid.minor {
        for value in &x_axis.minor {
            if let Some(fraction) = chart.x.scale.map(*value, chart.x.minimum, chart.x.maximum) {
                grid_line(
                    list,
                    chart.x.id,
                    (axes.x + fraction * axes.width, axes.y),
                    (axes.x + fraction * axes.width, axes.bottom()),
                    &minor,
                );
            }
        }
    }
    if chart.y.grid.minor {
        for value in &y_axis.minor {
            if let Some(fraction) = chart.y.scale.map(*value, chart.y.minimum, chart.y.maximum) {
                let y = axes.bottom() - fraction * axes.height;
                grid_line(list, chart.y.id, (axes.x, y), (axes.right(), y), &minor);
            }
        }
    }
    if chart.x.grid.major {
        for tick in &x_axis.major {
            grid_line(
                list,
                chart.x.id,
                (tick.position, axes.y),
                (tick.position, axes.bottom()),
                &major,
            );
        }
    }
    if chart.y.grid.major {
        for tick in &y_axis.major {
            grid_line(
                list,
                chart.y.id,
                (axes.x, tick.position),
                (axes.right(), tick.position),
                &major,
            );
        }
    }
}

fn grid_line(
    list: &mut DisplayList,
    node: NodeId,
    start: (f64, f64),
    end: (f64, f64),
    style: &Stroke,
) {
    list.items.push(DisplayItem::Path {
        source: node,
        path: Path {
            verbs: vec![
                PathVerb::MoveTo(pt(start.0), pt(start.1)),
                PathVerb::LineTo(pt(end.0), pt(end.1)),
            ],
        },
        fill: None,
        stroke: Some(style.clone()),
    });
}

fn draw_series(
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
        let mapped: Vec<(f64, f64)> = series
            .points
            .iter()
            .filter_map(|point| map_point(chart, axes, *point))
            .collect();
        if let Some(line) = series.line
            && mapped.len() >= 2
        {
            list.items.push(DisplayItem::Path {
                source: series.id,
                path: polyline(&mapped),
                fill: None,
                stroke: Some(stroke(series.color, line.width, line.dash)),
            });
            hit_map.items.push(HitItem {
                node: series.id,
                bounds: bounds_of_points(&mapped, line.width + 3.0),
                z_order: z,
                role: SelectableRole::Series,
                data_index: None,
                tooltip: Some(series.label.clone()),
                path_proximity: mapped.clone(),
            });
            z += 1;
        }
        for (index, ((x, y), point)) in mapped.iter().zip(series.points.iter()).enumerate() {
            if let Some(error) = series.errors.get(index) {
                draw_error_bar(chart, axes, series, index, *point, *error, list, hit_map, z)?;
                z += 1;
            }
            if let Some(marker) = series.marker
                && index % marker.interval.max(1) == 0
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
                    data_index: Some(index),
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

#[allow(clippy::too_many_arguments)]
fn draw_error_bar(
    chart: &Chart,
    axes: Bounds,
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
        DataPoint {
            x: point.x - error.x_minus,
            y: point.y,
        },
    );
    let right = map_point(
        chart,
        axes,
        DataPoint {
            x: point.x + error.x_plus,
            y: point.y,
        },
    );
    let top = map_point(
        chart,
        axes,
        DataPoint {
            x: point.x,
            y: point.y + error.y_plus,
        },
    );
    let bottom = map_point(
        chart,
        axes,
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
        width: 0.65,
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

fn draw_annotations(
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
            let Some(target) = map_point(chart, axes, connector.target) else {
                return Err(LayoutError::InvalidData(annotation.id));
            };
            let start = (target.0.clamp(x, x + width), target.1.clamp(top, bottom));
            let mut verbs = vec![
                PathVerb::MoveTo(pt(start.0), pt(start.1)),
                PathVerb::LineTo(pt(target.0), pt(target.1)),
            ];
            if connector.start_arrow {
                append_arrow_head(&mut verbs, start, target, connector.arrow_size);
            }
            if connector.end_arrow {
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
            hit_map.items.push(HitItem {
                node: annotation.id,
                bounds: Bounds {
                    x: target.0 - 4.0,
                    y: target.1 - 4.0,
                    width: 8.0,
                    height: 8.0,
                },
                z_order: 650 + index as u32,
                role: SelectableRole::AnnotationConnector,
                data_index: Some(connector_index),
                tooltip: Some(format!("connector {}", connector_index + 1)),
                path_proximity: vec![target],
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
                height: (bottom - top).max(TICK_FONT),
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

fn draw_legend(
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

fn annotation_position(
    chart: &Chart,
    axes: Bounds,
    annotation: &Annotation,
    canvas_top: f64,
) -> Option<(f64, f64)> {
    match annotation.position {
        AnnotationPosition::Data(point) => map_point(chart, axes, point),
        AnnotationPosition::FigurePoints { x, y } if x.is_finite() && y.is_finite() => {
            Some((x, y + canvas_top))
        }
        AnnotationPosition::FigurePoints { .. } => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn directional_tick(
    list: &mut DisplayList,
    node: NodeId,
    x: f64,
    y: f64,
    horizontal_delta: bool,
    inward_sign: f64,
    length: f64,
    direction: TickDirection,
) {
    let (start, end) = match direction {
        TickDirection::In => (0.0, inward_sign * length),
        TickDirection::Out => (0.0, -inward_sign * length),
        TickDirection::InOut => (-inward_sign * length / 2.0, inward_sign * length / 2.0),
    };
    if horizontal_delta {
        tick_mark(list, node, x + start, y, end - start, 0.0);
    } else {
        tick_mark(list, node, x, y + start, 0.0, end - start);
    }
}

fn tick_mark(list: &mut DisplayList, node: NodeId, x: f64, y: f64, dx: f64, dy: f64) {
    list.items.push(DisplayItem::Path {
        source: node,
        path: Path {
            verbs: vec![
                PathVerb::MoveTo(pt(x), pt(y)),
                PathVerb::LineTo(pt(x + dx), pt(y + dy)),
            ],
        },
        fill: None,
        stroke: Some(stroke(Color(45, 50, 55, 255), 0.6, DashStyle::Solid)),
    });
}

fn text(
    list: &mut DisplayList,
    node: NodeId,
    value: &str,
    position: (f64, f64),
    size: f64,
    anchor: TextAnchor,
    rotation: f64,
) {
    list.items.push(DisplayItem::GlyphRun(GlyphRun {
        source: node,
        label: Label::Text(value.into()),
        x: pt(position.0),
        y: pt(position.1),
        size: pt(size),
        color: Color(25, 25, 25, 255),
        rotation_degrees: rotation,
        anchor,
    }));
}

fn label_text(
    list: &mut DisplayList,
    node: NodeId,
    label: &Label,
    position: (f64, f64),
    size: f64,
    anchor: TextAnchor,
    rotation: f64,
) {
    list.items.push(DisplayItem::GlyphRun(GlyphRun {
        source: node,
        label: label.clone(),
        x: pt(position.0),
        y: pt(position.1),
        size: pt(size),
        color: Color(25, 25, 25, 255),
        rotation_degrees: rotation,
        anchor,
    }));
}

fn map_point(chart: &Chart, axes: Bounds, point: DataPoint) -> Option<(f64, f64)> {
    let x = chart
        .x
        .scale
        .map(point.x, chart.x.minimum, chart.x.maximum)?;
    let y = chart
        .y
        .scale
        .map(point.y, chart.y.minimum, chart.y.maximum)?;
    Some((axes.x + x * axes.width, axes.bottom() - y * axes.height))
}

fn polyline(points: &[(f64, f64)]) -> Path {
    Path {
        verbs: points
            .iter()
            .enumerate()
            .map(|(index, (x, y))| {
                if index == 0 {
                    PathVerb::MoveTo(pt(*x), pt(*y))
                } else {
                    PathVerb::LineTo(pt(*x), pt(*y))
                }
            })
            .collect(),
    }
}

fn rectangle(bounds: Bounds) -> Path {
    Path {
        verbs: vec![
            PathVerb::MoveTo(pt(bounds.x), pt(bounds.y)),
            PathVerb::LineTo(pt(bounds.right()), pt(bounds.y)),
            PathVerb::LineTo(pt(bounds.right()), pt(bounds.bottom())),
            PathVerb::LineTo(pt(bounds.x), pt(bounds.bottom())),
            PathVerb::Close,
        ],
    }
}

fn marker_path(x: f64, y: f64, style: MarkerStyle) -> Path {
    let radius = style.size / 2.0;
    match style.shape {
        MarkerShape::Circle => {
            let k = radius * 0.552_284_749_8;
            Path {
                verbs: vec![
                    PathVerb::MoveTo(pt(x + radius), pt(y)),
                    PathVerb::CurveTo(
                        pt(x + radius),
                        pt(y + k),
                        pt(x + k),
                        pt(y + radius),
                        pt(x),
                        pt(y + radius),
                    ),
                    PathVerb::CurveTo(
                        pt(x - k),
                        pt(y + radius),
                        pt(x - radius),
                        pt(y + k),
                        pt(x - radius),
                        pt(y),
                    ),
                    PathVerb::CurveTo(
                        pt(x - radius),
                        pt(y - k),
                        pt(x - k),
                        pt(y - radius),
                        pt(x),
                        pt(y - radius),
                    ),
                    PathVerb::CurveTo(
                        pt(x + k),
                        pt(y - radius),
                        pt(x + radius),
                        pt(y - k),
                        pt(x + radius),
                        pt(y),
                    ),
                    PathVerb::Close,
                ],
            }
        }
        MarkerShape::Square => rectangle(Bounds {
            x: x - radius,
            y: y - radius,
            width: style.size,
            height: style.size,
        }),
        MarkerShape::TriangleUp => polygon(&[
            (x, y - radius),
            (x + radius, y + radius),
            (x - radius, y + radius),
        ]),
        MarkerShape::TriangleDown => polygon(&[
            (x - radius, y - radius),
            (x + radius, y - radius),
            (x, y + radius),
        ]),
        MarkerShape::Diamond => polygon(&[
            (x, y - radius),
            (x + radius, y),
            (x, y + radius),
            (x - radius, y),
        ]),
        MarkerShape::Pentagon => radial_marker_polygon(x, y, radius, 5, None),
        MarkerShape::Star => radial_marker_polygon(x, y, radius, 5, Some(0.46)),
        MarkerShape::Plus => Path {
            verbs: vec![
                PathVerb::MoveTo(pt(x - radius), pt(y)),
                PathVerb::LineTo(pt(x + radius), pt(y)),
                PathVerb::MoveTo(pt(x), pt(y - radius)),
                PathVerb::LineTo(pt(x), pt(y + radius)),
            ],
        },
        MarkerShape::Cross => Path {
            verbs: vec![
                PathVerb::MoveTo(pt(x - radius), pt(y - radius)),
                PathVerb::LineTo(pt(x + radius), pt(y + radius)),
                PathVerb::MoveTo(pt(x - radius), pt(y + radius)),
                PathVerb::LineTo(pt(x + radius), pt(y - radius)),
            ],
        },
    }
}

fn radial_marker_polygon(
    x: f64,
    y: f64,
    radius: f64,
    points: usize,
    inner_ratio: Option<f64>,
) -> Path {
    let vertices = points * if inner_ratio.is_some() { 2 } else { 1 };
    let positions = (0..vertices)
        .map(|index| {
            let angle = -std::f64::consts::FRAC_PI_2
                + 2.0 * std::f64::consts::PI * index as f64 / vertices as f64;
            let current_radius = if index % 2 == 1 {
                inner_ratio.map_or(radius, |ratio| radius * ratio)
            } else {
                radius
            };
            (
                x + current_radius * angle.cos(),
                y + current_radius * angle.sin(),
            )
        })
        .collect::<Vec<_>>();
    polygon(&positions)
}

fn marker_fill(style: MarkerStyle, color: Color) -> Option<Fill> {
    (style.filled && !matches!(style.shape, MarkerShape::Plus | MarkerShape::Cross)).then_some(
        Fill {
            color,
            rule: FillRule::NonZero,
        },
    )
}

fn marker_outline_width(style: MarkerStyle) -> f64 {
    if style.filled { 0.8 } else { 1.0 }
}

fn polygon(points: &[(f64, f64)]) -> Path {
    let mut verbs: Vec<PathVerb> = points
        .iter()
        .enumerate()
        .map(|(index, (x, y))| {
            if index == 0 {
                PathVerb::MoveTo(pt(*x), pt(*y))
            } else {
                PathVerb::LineTo(pt(*x), pt(*y))
            }
        })
        .collect();
    verbs.push(PathVerb::Close);
    Path { verbs }
}

fn stroke(color: Color, width: f64, dash: DashStyle) -> Stroke {
    let pattern = match dash {
        DashStyle::Solid => Vec::new(),
        DashStyle::Dashed => vec![4.0, 2.4],
        DashStyle::Dotted => vec![0.8, 1.8],
        DashStyle::DashDot => vec![4.0, 2.0, 0.8, 2.0],
        DashStyle::LongDash => vec![8.0, 3.0],
        DashStyle::LongShortDash => vec![8.0, 2.0, 2.0, 2.0],
        DashStyle::DashDotDot => vec![6.0, 2.0, 0.8, 2.0, 0.8, 2.0],
    };
    Stroke {
        color,
        width: pt(width),
        cap: if dash == DashStyle::Dotted {
            LineCap::Round
        } else {
            LineCap::Butt
        },
        join: LineJoin::Miter,
        dash: pattern.into_iter().map(pt).collect(),
    }
}

fn bounds_of_points(points: &[(f64, f64)], padding: f64) -> Bounds {
    let min_x = points
        .iter()
        .map(|point| point.0)
        .fold(f64::INFINITY, f64::min);
    let max_x = points
        .iter()
        .map(|point| point.0)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = points
        .iter()
        .map(|point| point.1)
        .fold(f64::INFINITY, f64::min);
    let max_y = points
        .iter()
        .map(|point| point.1)
        .fold(f64::NEG_INFINITY, f64::max);
    Bounds {
        x: min_x - padding,
        y: min_y - padding,
        width: max_x - min_x + padding * 2.0,
        height: max_y - min_y + padding * 2.0,
    }
}

fn distance_to_polyline(x: f64, y: f64, points: &[(f64, f64)]) -> f64 {
    points
        .windows(2)
        .map(|pair| {
            let (x1, y1) = pair[0];
            let (x2, y2) = pair[1];
            let dx = x2 - x1;
            let dy = y2 - y1;
            let length_squared = dx * dx + dy * dy;
            let t = if length_squared == 0.0 {
                0.0
            } else {
                (((x - x1) * dx + (y - y1) * dy) / length_squared).clamp(0.0, 1.0)
            };
            ((x - (x1 + t * dx)).powi(2) + (y - (y1 + t * dy)).powi(2)).sqrt()
        })
        .fold(f64::INFINITY, f64::min)
}

fn pt(value: f64) -> Pt {
    Pt::new(value).expect("validated finite point")
}
