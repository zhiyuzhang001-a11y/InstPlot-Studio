use studio_render_spike::{Color, NodeId};

use crate::scale::{Locator, Scale};

#[derive(Clone, Debug)]
pub struct Chart {
    pub id: NodeId,
    pub width_pt: f64,
    pub height_pt: f64,
    pub x: AxisSpec,
    pub y: AxisSpec,
    pub series: Vec<Series>,
    pub annotations: Vec<Annotation>,
}

#[derive(Clone, Debug)]
pub struct AxisSpec {
    pub id: NodeId,
    pub label: String,
    pub minimum: f64,
    pub maximum: f64,
    pub scale: Scale,
    pub locator: Locator,
}

#[derive(Clone, Debug)]
pub struct Series {
    pub id: NodeId,
    pub label: String,
    pub points: Vec<DataPoint>,
    pub line: Option<LineStyle>,
    pub marker: Option<MarkerStyle>,
    pub errors: Vec<ErrorBar>,
    pub color: Color,
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
    pub text: String,
    pub point: DataPoint,
    pub offset_pt: (f64, f64),
}
