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
    Annotation, AnnotationConnector, AnnotationPosition, AxisAppearance, AxisSpec, Chart,
    DashStyle, DataPoint, ErrorBar, ErrorStyle, GridSpec, LegendErrorStyle, LegendGrid,
    LegendPosition, LegendSpec, LineStyle, MarkerShape, MarkerStyle, Series, TickDirection,
};
pub use scale::{
    FormattedTicks, Formatter, Locator, Scale, collision_stride, format_ticks, format_ticks_with,
    minor_ticks, minor_ticks_with_interval,
};
pub use text::{ParleyMeasurer, TextMeasurer, TextSize};
