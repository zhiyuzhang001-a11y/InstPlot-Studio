use super::*;

pub(crate) fn secondary_axis_placeholder_font_size(canvas_zoom: f32) -> f32 {
    instplot_layout::AXIS_LABEL_FONT_PT as f32 * canvas_zoom
}

pub(crate) const SECONDARY_AXIS_PLACEHOLDER_MIN_PAD_PT: f32 = 7.0;
const SECONDARY_AXIS_PLACEHOLDER_EDGE_SAFETY_PT: f32 = 2.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct SecondaryAxisPlaceholderGutter {
    pub(crate) top: f32,
    pub(crate) right: f32,
}

pub(crate) fn secondary_axis_placeholder_pad(
    identity: AxisIdentity,
    axes: instplot_layout::Bounds,
    legend: Option<instplot_layout::Bounds>,
    text_size: egui::Vec2,
    canvas_zoom: f32,
    configured_pad_pt: f32,
) -> f32 {
    let mut pad = SECONDARY_AXIS_PLACEHOLDER_MIN_PAD_PT.max(configured_pad_pt);
    let Some(legend) = legend else {
        return pad;
    };
    let text_width = text_size.x / canvas_zoom.max(0.01);
    let text_height = text_size.y / canvas_zoom.max(0.01);
    match identity {
        AxisIdentity::X2 => {
            let center = axes.x + axes.width / 2.0;
            let overlaps_x = center + f64::from(text_width) / 2.0 >= legend.x
                && center - f64::from(text_width) / 2.0 <= legend.right();
            if overlaps_x && legend.bottom() <= axes.y + 0.01 {
                pad = pad.max((axes.y - legend.y) as f32 + 2.0);
            }
        }
        AxisIdentity::Y2 => {
            let center = axes.y + axes.height / 2.0;
            let overlaps_y = center + f64::from(text_width) / 2.0 >= legend.y
                && center - f64::from(text_width) / 2.0 <= legend.bottom();
            if overlaps_y && legend.x >= axes.right() - 0.01 {
                pad = pad.max((legend.right() - axes.right()) as f32 + text_height + 2.0);
            }
        }
        AxisIdentity::X1 | AxisIdentity::Y1 => {}
    }
    pad
}

pub(crate) fn secondary_axis_placeholder_gutter(
    identity: AxisIdentity,
    axes: instplot_layout::Bounds,
    formal_figure_size: egui::Vec2,
    text_size: egui::Vec2,
    canvas_zoom: f32,
    label_tick_pad_pt: f32,
) -> SecondaryAxisPlaceholderGutter {
    let pad = SECONDARY_AXIS_PLACEHOLDER_MIN_PAD_PT.max(label_tick_pad_pt) * canvas_zoom;
    match identity {
        AxisIdentity::X2 => {
            let text_top = axes.y as f32 * canvas_zoom - pad - text_size.y;
            SecondaryAxisPlaceholderGutter {
                top: (-text_top + SECONDARY_AXIS_PLACEHOLDER_EDGE_SAFETY_PT).max(0.0),
                right: 0.0,
            }
        }
        AxisIdentity::Y2 => {
            // A quarter-turn swaps the laid-out text width and height.
            let text_right = (axes.x + axes.width) as f32 * canvas_zoom + pad + text_size.y;
            SecondaryAxisPlaceholderGutter {
                top: 0.0,
                right: (text_right - formal_figure_size.x
                    + SECONDARY_AXIS_PLACEHOLDER_EDGE_SAFETY_PT)
                    .max(0.0),
            }
        }
        AxisIdentity::X1 | AxisIdentity::Y1 => SecondaryAxisPlaceholderGutter::default(),
    }
}
