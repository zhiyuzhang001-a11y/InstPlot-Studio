use crate::{Color, Mm, Pt, Stroke};
use instplot_text::Label;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(pub u64);

#[derive(Clone, Debug, PartialEq)]
pub struct Figure {
    pub id: NodeId,
    pub width: Mm,
    pub height: Mm,
    pub axes: Vec<Axes>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Axes {
    pub id: NodeId,
    pub left: Pt,
    pub top: Pt,
    pub width: Pt,
    pub height: Pt,
    pub x: Axis,
    pub y: Axis,
    pub artists: Vec<Artist>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Axis {
    pub id: NodeId,
    pub label: Label,
    pub minimum: f64,
    pub maximum: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Artist {
    Line(Line),
    Scatter(Scatter),
    ErrorBar(ErrorBar),
    ReferenceLine(ReferenceLine),
    Text(Text),
    Legend(Legend),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub id: NodeId,
    pub points: Vec<(f64, f64)>,
    pub stroke: Stroke,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Scatter {
    pub id: NodeId,
    pub points: Vec<(f64, f64)>,
    pub radius: Pt,
    pub color: Color,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ErrorBar {
    pub id: NodeId,
    pub points: Vec<(f64, f64, f64)>,
    pub cap_width: Pt,
    pub stroke: Stroke,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReferenceLine {
    pub id: NodeId,
    pub y: f64,
    pub stroke: Stroke,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Text {
    pub id: NodeId,
    pub x: Pt,
    pub y: Pt,
    pub value: Label,
    pub size: Pt,
    pub color: Color,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Legend {
    pub id: NodeId,
    pub x: Pt,
    pub y: Pt,
    pub entries: Vec<Label>,
    pub size: Pt,
    pub color: Color,
}
