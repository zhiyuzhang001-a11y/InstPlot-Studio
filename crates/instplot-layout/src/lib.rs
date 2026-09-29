//! Production single-axes layout compiler for InstPlot Studio.

mod fixture;
mod layout;
mod model;
mod scale;
mod text;

pub use fixture::{marker_gallery_fixture, publication_fixture};
pub use layout::{
    AXIS_LABEL_FONT_PT, AxisLayout, Bounds, HitItem, HitMap, LayoutError, LayoutResult,
    LayoutWarning, SelectableRole, TickLayout, layout, layout_with_measurer,
};
pub use model::{
    Annotation, AnnotationConnector, AnnotationPosition, ArrowHead, AxisAppearance,
    AxisDisplayScale, AxisPair, AxisSpec, Chart, DashStyle, DataPoint, ErrorBar, ErrorStyle,
    GridSpec, LegendErrorStyle, LegendGrid, LegendPosition, LegendSpec, LineStyle, MarkerShape,
    MarkerStyle, MeasurementArrow, MeasurementConstraint, ReferenceLine, ReferenceOrientation,
    Series, TickDirection,
};
pub use scale::{
    FormattedTicks, Formatter, Locator, Scale, automatic_display_exponent, checked_pow10,
    collision_stride, finite_midpoint, finite_span, format_ticks, format_ticks_with,
    format_ticks_with_exponent, linear_fraction, linear_value, minor_ticks,
    minor_ticks_with_interval, tick_count_exceeds,
};
pub use text::{ParleyMeasurer, TextMeasurer, TextSize};
