use core::fmt;

use instplot_render::{
    Color, DisplayItem, DisplayList, Fill, FillRule, GlyphRun, LineCap, LineJoin, NodeId, Path,
    PathVerb, Pt, Stroke, TextAnchor,
};
use instplot_text::Label;

use crate::model::{
    Annotation, AnnotationPosition, ArrowHead, AxisDisplayScale, AxisPair, AxisSpec, Chart,
    DashStyle, DataPoint, LegendPosition, MarkerShape, MarkerStyle, MeasurementArrow,
    ReferenceOrientation, Series, TickDirection,
};
use crate::scale::{Formatter, Scale, collision_stride, minor_ticks_with_interval};
use crate::text::{ParleyMeasurer, TextMeasurer, TextSize};

mod axes;
mod legend;
mod objects;
mod series;

use axes::{DrawAxesRequest, axis_layout, draw_axes};
use legend::{choose_legend, draw_legend};
use objects::{draw_annotations, draw_measurement_arrows, draw_reference_lines};
use series::draw_series;

const TICK_FONT: f64 = 8.0;
/// Physical font size shared by primary and secondary axis labels.
pub const AXIS_LABEL_FONT_PT: f64 = 9.0;
const LABEL_FONT: f64 = AXIS_LABEL_FONT_PT;
const LEGEND_FONT: f64 = 8.0;
const AXIS_STROKE_WIDTH_PT: f64 = 0.7;
const FALLBACK_ERROR_BAR_WIDTH_PT: f64 = 0.7;
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
    ReferenceLine,
    MeasurementArrow,
    MeasurementArrowStart,
    MeasurementArrowEnd,
    MeasurementArrowLabel,
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
                !matches!(
                    item.role,
                    SelectableRole::Series
                        | SelectableRole::ReferenceLine
                        | SelectableRole::MeasurementArrow
                        | SelectableRole::AnnotationConnector
                ) || distance_to_polyline(x, y, &item.path_proximity) <= tolerance
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
    pub label: instplot_text::Label,
    pub duplicate_tick_labels: bool,
    pub long_tick_label: bool,
    pub scaled_ticks_without_visible_factor: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LayoutWarning {
    NonConvergent { node: NodeId, iterations: usize },
    LegendMovedOutside { node: NodeId },
    TextOutsideFigure { node: NodeId, text: String },
    InsufficientPlotArea { node: NodeId },
    DuplicateTickLabels { node: NodeId },
    LongTickLabel { node: NodeId },
    ScaledTicksWithoutVisibleFactor { node: NodeId },
}

#[derive(Clone, Debug)]
pub struct LayoutResult {
    pub display_list: DisplayList,
    pub axes: Bounds,
    pub x_axis: AxisLayout,
    pub y_axis: AxisLayout,
    pub x2_axis: Option<AxisLayout>,
    pub y2_axis: Option<AxisLayout>,
    pub x_label_bounds: Bounds,
    pub y_label_bounds: Bounds,
    pub x2_label_bounds: Option<Bounds>,
    pub y2_label_bounds: Option<Bounds>,
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
        for (name, axis) in [("X2", self.x2_axis.as_ref()), ("Y2", self.y2_axis.as_ref())] {
            let Some(axis) = axis else { continue };
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
        let x = axis_layout(&chart.x, true, axes, measurer);
        let y = axis_layout(&chart.y, false, axes, measurer);
        let x2 = chart
            .x2
            .as_ref()
            .map(|axis| axis_layout(axis, true, axes, measurer));
        let y2 = chart
            .y2
            .as_ref()
            .map(|axis| axis_layout(axis, false, axes, measurer));
        let legend = choose_legend(chart, axes, measurer, auto_position);
        let y_label = measurer.measure_label(&y.label, LABEL_FONT);
        let x_label = measurer.measure_label(&x.label, LABEL_FONT);
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
        let x2_tick_height = x2
            .as_ref()
            .into_iter()
            .flat_map(|axis| &axis.major)
            .map(|tick| tick.label_bounds.height)
            .fold(0.0, f64::max);
        let y2_tick_width = y2
            .as_ref()
            .into_iter()
            .flat_map(|axis| &axis.major)
            .map(|tick| tick.label_bounds.width)
            .fold(0.0, f64::max);
        let base_right = if chart.y.appearance.far_tick_labels && chart.y2.is_none() {
            max_y_tick + chart.y.appearance.tick_label_pad_pt + 6.0
        } else {
            12.0
        };
        let secondary_right =
            chart
                .y2
                .as_ref()
                .zip(y2.as_ref())
                .map_or(base_right, |(axis, layout)| {
                    let label = measurer.measure_label(&layout.label, LABEL_FONT);
                    axis.appearance.label_edge_pad_pt
                        + label.height
                        + axis.appearance.label_tick_pad_pt
                        + if axis.has_data && axis.appearance.near_tick_labels {
                            y2_tick_width + axis.appearance.tick_label_pad_pt
                        } else {
                            0.0
                        }
                });
        let base_top = if chart.x.appearance.far_tick_labels && chart.x2.is_none() {
            tick_height + chart.x.appearance.tick_label_pad_pt + 6.0
        } else {
            12.0
        };
        let secondary_top =
            chart
                .x2
                .as_ref()
                .zip(x2.as_ref())
                .map_or(base_top, |(axis, layout)| {
                    let label = measurer.measure_label(&layout.label, LABEL_FONT);
                    axis.appearance.label_edge_pad_pt
                        + label.height
                        + axis.appearance.label_tick_pad_pt
                        + if axis.has_data && axis.appearance.near_tick_labels {
                            x2_tick_height + axis.appearance.tick_label_pad_pt
                        } else {
                            0.0
                        }
                });
        let mut next = Margins {
            left: chart.y.appearance.label_edge_pad_pt
                + y_label.height
                + chart.y.appearance.label_tick_pad_pt
                + if chart.y.appearance.near_tick_labels {
                    max_y_tick + chart.y.appearance.tick_label_pad_pt
                } else {
                    0.0
                },
            right: secondary_right,
            top: secondary_top,
            bottom: chart.x.appearance.label_edge_pad_pt
                + x_label.height
                + chart.x.appearance.label_tick_pad_pt
                + if chart.x.appearance.near_tick_labels {
                    tick_height + chart.x.appearance.tick_label_pad_pt
                } else {
                    0.0
                },
            canvas_right: (secondary_right - base_right).max(0.0),
            canvas_top: (secondary_top - base_top).max(0.0),
            canvas_bottom: 0.0,
        };
        if legend.outside {
            match legend.position {
                LegendPosition::Above => {
                    let legend_top = if chart
                        .legend
                        .as_ref()
                        .is_some_and(|spec| spec.manual_position.is_some())
                    {
                        (legend.bounds.bottom() + 6.0 - next.top).max(0.0)
                    } else {
                        legend.bounds.height + 6.0
                    };
                    next.canvas_top += legend_top;
                    next.top += legend_top;
                    let required_right = (legend.bounds.right() + 3.0 - chart.width_pt).max(0.0);
                    if required_right > next.canvas_right {
                        next.right += required_right - next.canvas_right;
                        next.canvas_right = required_right;
                    }
                }
                LegendPosition::Right => {
                    let required_right = (legend.bounds.right() + 3.0 - chart.width_pt).max(0.0);
                    if required_right > next.canvas_right {
                        next.right += required_right - next.canvas_right;
                        next.canvas_right = required_right;
                    }
                    next.canvas_bottom = (legend.bounds.bottom() + 3.0 - chart.height_pt).max(0.0);
                    next.bottom += next.canvas_bottom;
                }
                _ => {}
            }
        } else if legend.visible {
            let required_right = (legend.bounds.right() - chart.width_pt).max(0.0);
            if required_right > next.canvas_right {
                next.right += required_right - next.canvas_right;
                next.canvas_right = required_right;
            }
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
    let x_axis = axis_layout(&chart.x, true, axes, measurer);
    let y_axis = axis_layout(&chart.y, false, axes, measurer);
    let x2_axis = chart
        .x2
        .as_ref()
        .map(|axis| axis_layout(axis, true, axes, measurer));
    let y2_axis = chart
        .y2
        .as_ref()
        .map(|axis| axis_layout(axis, false, axes, measurer));
    let legend_choice = choose_legend(chart, axes, measurer, auto_position);
    let mut warnings = Vec::new();
    for (axis, layout) in [(&chart.x, &x_axis), (&chart.y, &y_axis)]
        .into_iter()
        .chain(chart.x2.iter().zip(x2_axis.iter()))
        .chain(chart.y2.iter().zip(y2_axis.iter()))
    {
        if layout.duplicate_tick_labels {
            warnings.push(LayoutWarning::DuplicateTickLabels { node: axis.id });
        }
        if layout.long_tick_label {
            warnings.push(LayoutWarning::LongTickLabel { node: axis.id });
        }
        if layout.scaled_ticks_without_visible_factor {
            warnings.push(LayoutWarning::ScaledTicksWithoutVisibleFactor { node: axis.id });
        }
    }
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

    let (x_label_bounds, y_label_bounds, x2_label_bounds, y2_label_bounds) =
        draw_axes(DrawAxesRequest {
            chart,
            axes,
            x_axis: &x_axis,
            y_axis: &y_axis,
            x2_axis: x2_axis.as_ref(),
            y2_axis: y2_axis.as_ref(),
            canvas_bottom: margins.canvas_bottom,
            measurer,
            list: &mut display_list,
            hit_map: &mut hit_map,
            warnings: &mut warnings,
        });
    draw_reference_lines(chart, axes, &mut display_list, &mut hit_map)?;
    draw_series(chart, axes, &mut display_list, &mut hit_map)?;
    draw_measurement_arrows(
        chart,
        axes,
        measurer,
        &mut display_list,
        &mut hit_map,
        &mut warnings,
    )?;
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
        x2_axis,
        y2_axis,
        x_label_bounds,
        y_label_bounds,
        x2_label_bounds,
        y2_label_bounds,
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
    for axis in std::iter::once(&chart.x)
        .chain(std::iter::once(&chart.y))
        .chain(chart.x2.iter())
        .chain(chart.y2.iter())
    {
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
        let (x_axis, y_axis) = series_axis_specs(chart, series.id)?;
        if series.points.iter().any(|point| {
            !point.x.is_finite()
                || !point.y.is_finite()
                || (x_axis.scale == Scale::Log10 && point.x <= 0.0)
                || (y_axis.scale == Scale::Log10 && point.y <= 0.0)
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
            let Some((x_axis, y_axis)) = axis_specs(chart, connector.axes) else {
                return false;
            };
            connector.target.x.is_finite()
                && connector.target.y.is_finite()
                && (x_axis.scale != Scale::Log10 || connector.target.x > 0.0)
                && (y_axis.scale != Scale::Log10 || connector.target.y > 0.0)
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

fn extrema_regions(chart: &Chart, axes: Bounds) -> Vec<Bounds> {
    let points: Vec<(f64, f64)> = chart
        .series
        .iter()
        .flat_map(|series| {
            let pair = chart
                .series_axes
                .get(&series.id)
                .copied()
                .unwrap_or_default();
            series
                .points
                .iter()
                .filter_map(move |point| map_point(chart, axes, pair, *point))
        })
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
        let pair = chart
            .series_axes
            .get(&series.id)
            .copied()
            .unwrap_or_default();
        for (index, point) in series.points.iter().enumerate() {
            if let Some((x, y)) = map_point(chart, axes, pair, *point) {
                let mut radius: f64 = 3.0;
                if let Some(marker) = series.marker {
                    radius = radius.max(marker.size / 2.0);
                }
                if let Some(error) = series.errors.get(index)
                    && let Some((left, top)) = map_point(
                        chart,
                        axes,
                        pair,
                        DataPoint {
                            x: point.x - error.x_minus,
                            y: point.y + error.y_plus,
                        },
                    )
                    && let Some((right, bottom)) = map_point(
                        chart,
                        axes,
                        pair,
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
                    map_point(chart, axes, pair, segment[0]),
                    map_point(chart, axes, pair, segment[1]),
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
fn annotation_position(
    chart: &Chart,
    axes: Bounds,
    annotation: &Annotation,
    canvas_top: f64,
) -> Option<(f64, f64)> {
    match annotation.position {
        AnnotationPosition::Data(point) => map_point(chart, axes, AxisPair::X1Y1, point),
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
        stroke: Some(stroke(
            Color(0, 0, 0, 255),
            AXIS_STROKE_WIDTH_PT,
            DashStyle::Solid,
        )),
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
        color: Color(0, 0, 0, 255),
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
        color: Color(0, 0, 0, 255),
        rotation_degrees: rotation,
        anchor,
    }));
}

fn map_point(chart: &Chart, axes: Bounds, pair: AxisPair, point: DataPoint) -> Option<(f64, f64)> {
    let (x_axis, y_axis) = axis_specs(chart, pair)?;
    let x = x_axis.scale.map(point.x, x_axis.minimum, x_axis.maximum)?;
    let y = y_axis.scale.map(point.y, y_axis.minimum, y_axis.maximum)?;
    Some((axes.x + x * axes.width, axes.bottom() - y * axes.height))
}

fn axis_specs(chart: &Chart, pair: AxisPair) -> Option<(&AxisSpec, &AxisSpec)> {
    match pair {
        AxisPair::X1Y1 => Some((&chart.x, &chart.y)),
        AxisPair::X2Y1 => chart.x2.as_ref().map(|x| (x, &chart.y)),
        AxisPair::X1Y2 => chart.y2.as_ref().map(|y| (&chart.x, y)),
    }
}

fn series_axis_specs(chart: &Chart, id: NodeId) -> Result<(&AxisSpec, &AxisSpec), LayoutError> {
    let pair = chart.series_axes.get(&id).copied().unwrap_or_default();
    axis_specs(chart, pair).ok_or(LayoutError::InvalidData(id))
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

#[cfg(test)]
mod tests {
    use super::objects::trimmed_arrow_shaft;
    use super::series::should_draw_marker;
    use crate::{MarkerShape, MarkerStyle, SelectableRole, layout, publication_fixture};

    #[test]
    fn marker_interval_always_includes_first_and_last_drawable_points() {
        let shown = (0..23)
            .filter(|index| should_draw_marker(*index, *index, 23, 10))
            .collect::<Vec<_>>();
        assert_eq!(shown, [0, 10, 20, 22]);

        assert!(should_draw_marker(4, 0, 3, 10));
        assert!(!should_draw_marker(5, 1, 3, 10));
        assert!(should_draw_marker(6, 2, 3, 10));
        assert!(should_draw_marker(9, 0, 1, 10));
    }

    #[test]
    fn marker_interval_does_not_change_the_series_path() {
        let mut chart = publication_fixture();
        chart.series.truncate(1);
        chart.legend = None;
        let series_id = chart.series[0].id;
        let point_count = chart.series[0].points.len();
        chart.series[0].marker = Some(MarkerStyle {
            shape: MarkerShape::Circle,
            size: 4.0,
            filled: false,
            interval: 1,
        });
        let dense = layout(&chart).unwrap();
        let dense_path = dense
            .hit_map
            .items
            .iter()
            .find(|item| item.node == series_id && item.role == SelectableRole::Series)
            .unwrap()
            .path_proximity
            .clone();

        chart.series[0].marker.as_mut().unwrap().interval = 10;
        let sparse = layout(&chart).unwrap();
        let sparse_path = sparse
            .hit_map
            .items
            .iter()
            .find(|item| item.node == series_id && item.role == SelectableRole::Series)
            .unwrap()
            .path_proximity
            .clone();
        let marker_indexes = sparse
            .hit_map
            .items
            .iter()
            .filter(|item| item.node == series_id && item.role == SelectableRole::DataPoint)
            .filter_map(|item| item.data_index)
            .collect::<Vec<_>>();

        assert_eq!(sparse_path, dense_path);
        assert_eq!(marker_indexes.first(), Some(&0));
        assert_eq!(marker_indexes.last(), Some(&(point_count - 1)));
    }

    #[test]
    fn very_short_double_headed_arrow_collapses_its_shaft_without_reversing() {
        let (shaft_start, shaft_end, head_size) =
            trimmed_arrow_shaft((2.0, 3.0), (6.0, 3.0), 10.0, true, true);
        assert_eq!(shaft_start, (4.0, 3.0));
        assert_eq!(shaft_end, (4.0, 3.0));
        assert_eq!(head_size, 2.0);

        let (shaft_start, shaft_end, head_size) =
            trimmed_arrow_shaft((1.0, 1.0), (1.0, 1.0), 10.0, true, true);
        assert_eq!(shaft_start, (1.0, 1.0));
        assert_eq!(shaft_end, (1.0, 1.0));
        assert_eq!(head_size, 10.0);
    }
}
