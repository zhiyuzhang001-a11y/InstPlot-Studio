use studio_render_spike::{Color, NodeId};
use text_shaping_spike::Label;

use crate::scale::{Formatter, Locator, Scale};

#[derive(Clone, Debug)]
pub struct Chart {
    pub id: NodeId,
    pub width_pt: f64,
    pub height_pt: f64,
    pub x: AxisSpec,
    pub y: AxisSpec,
    pub series: Vec<Series>,
    pub annotations: Vec<Annotation>,
    pub legend: Option<LegendSpec>,
}

#[derive(Clone, Debug)]
pub struct AxisSpec {
    pub id: NodeId,
    pub label: Label,
    pub minimum: f64,
    pub maximum: f64,
    pub scale: Scale,
    pub locator: Locator,
    pub formatter: Formatter,
    pub grid: GridSpec,
    pub appearance: AxisAppearance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickDirection {
    In,
    Out,
    InOut,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AxisAppearance {
    pub near_spine: bool,
    pub far_spine: bool,
    pub near_ticks: bool,
    pub far_ticks: bool,
    pub near_tick_labels: bool,
    pub far_tick_labels: bool,
    pub major_ticks: bool,
    pub minor_ticks: bool,
    pub tick_direction: TickDirection,
    pub tick_label_pad_pt: f64,
    pub label_edge_pad_pt: f64,
    pub label_tick_pad_pt: f64,
}

impl Default for AxisAppearance {
    fn default() -> Self {
        Self {
            near_spine: true,
            far_spine: true,
            near_ticks: true,
            far_ticks: true,
            near_tick_labels: true,
            far_tick_labels: false,
            major_ticks: true,
            minor_ticks: true,
            tick_direction: TickDirection::In,
            tick_label_pad_pt: 4.0,
            label_edge_pad_pt: 6.0,
            label_tick_pad_pt: 4.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GridSpec {
    pub major: bool,
    pub minor: bool,
}

#[derive(Clone, Debug)]
pub struct Series {
    pub id: NodeId,
    pub label: String,
    pub points: Vec<DataPoint>,
    pub line: Option<LineStyle>,
    pub marker: Option<MarkerStyle>,
    pub errors: Vec<ErrorBar>,
    pub error_style: Option<ErrorStyle>,
    pub color: Color,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ErrorStyle {
    pub width: f64,
    pub cap_width: f64,
    pub dash: DashStyle,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DataPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ErrorBar {
    pub x_minus: f64,
    pub x_plus: f64,
    pub y_minus: f64,
    pub y_plus: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DashStyle {
    Solid,
    Dashed,
    Dotted,
    DashDot,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineStyle {
    pub width: f64,
    pub dash: DashStyle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkerShape {
    Circle,
    Square,
    TriangleUp,
    TriangleDown,
    Diamond,
    Plus,
    Cross,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MarkerStyle {
    pub shape: MarkerShape,
    pub size: f64,
    pub filled: bool,
}

#[derive(Clone, Debug)]
pub struct Annotation {
    pub id: NodeId,
    pub label: Label,
    pub position: AnnotationPosition,
    pub offset_pt: (f64, f64),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AnnotationPosition {
    Data(DataPoint),
    FigurePoints { x: f64, y: f64 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LegendSpec {
    pub id: NodeId,
    pub position: LegendPosition,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LegendPosition {
    Auto,
    FigurePoints { x: f64, y: f64 },
}
