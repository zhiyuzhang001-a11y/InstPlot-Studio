//! A2 validation spike: point-based Figure IR compiled into a backend-neutral display list.

mod compile;
mod display;
mod figure;
mod units;

pub use compile::{CompileError, compile};
pub use display::{
    Color, DisplayItem, DisplayList, Fill, FillRule, GlyphRun, Image, LineCap, LineJoin, Path,
    PathVerb, Stroke, SvgOutput, to_svg,
};
pub use figure::{
    Artist, Axes, Axis, ErrorBar, Figure, Legend, Line, NodeId, ReferenceLine, Scatter, Text,
};
pub use units::{Mm, Pt, Px, UnitError};
