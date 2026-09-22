//! Production single-axes layout compiler for InstPlot Studio.

mod fixture;
mod layout;
mod model;
mod scale;
mod text;

pub use fixture::{marker_gallery_fixture, publication_fixture};
pub use layout::{
    AxisLayout, Bounds, HitItem, HitMap, LayoutError, LayoutResult, LayoutWarning, SelectableRole,
    TickLayout, layout, layout_with_measurer,
};
pub use model::{
    Annotation, AnnotationPosition, AxisSpec, Chart, DashStyle, DataPoint, ErrorBar, ErrorStyle,
    GridSpec, LegendPosition, LegendSpec, LineStyle, MarkerShape, MarkerStyle, Series,
};
pub use scale::{
    FormattedTicks, Formatter, Locator, Scale, collision_stride, format_ticks, format_ticks_with,
    minor_ticks,
};
pub use text::{ParleyMeasurer, TextMeasurer, TextSize};
