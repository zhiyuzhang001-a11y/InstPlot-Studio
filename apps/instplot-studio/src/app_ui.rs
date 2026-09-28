use super::*;

mod axis_placeholders;
mod canvas_interaction;
mod canvas_view;
mod shell;

pub(crate) use axis_placeholders::*;
pub(crate) use canvas_interaction::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_tool_one_frame_drag_uses_the_press_hit_even_after_pointer_leaves_line() {
        let tool = DrawingTool::Reference {
            orientation: ReferenceOrientation::Vertical,
            axes: AxisBinding::PRIMARY,
        };
        assert!(reference_tool_allows_object_interaction(
            tool,
            None,
            Some(SelectableRole::ReferenceLine),
            None,
        ));
        assert!(!reference_tool_allows_object_interaction(
            tool,
            None,
            Some(SelectableRole::Axes),
            None,
        ));
        assert!(!reference_tool_allows_object_interaction(
            DrawingTool::Select,
            Some(SelectableRole::ReferenceLine),
            Some(SelectableRole::ReferenceLine),
            None,
        ));
    }

    #[test]
    fn empty_secondary_axis_placeholder_matches_formal_axis_label_size() {
        for zoom in [0.5, 1.0, 2.0, 3.0] {
            assert_eq!(
                secondary_axis_placeholder_font_size(zoom),
                instplot_layout::AXIS_LABEL_FONT_PT as f32 * zoom
            );
        }
    }

    #[test]
    fn empty_secondary_axis_placeholder_gutter_contains_rotated_and_horizontal_text() {
        let axes = instplot_layout::Bounds {
            x: 12.0,
            y: 4.0,
            width: 80.0,
            height: 50.0,
        };
        let figure = egui::vec2(94.0, 70.0);
        let text = egui::vec2(34.0, 11.0);
        let x2 = secondary_axis_placeholder_gutter(AxisIdentity::X2, axes, figure, text, 1.0, 7.0);
        let y2 = secondary_axis_placeholder_gutter(AxisIdentity::Y2, axes, figure, text, 1.0, 7.0);

        assert!(x2.top >= 16.0);
        assert_eq!(x2.right, 0.0);
        assert_eq!(y2.top, 0.0);
        assert!(y2.right >= 18.0);

        let padded =
            secondary_axis_placeholder_gutter(AxisIdentity::Y2, axes, figure, text, 1.0, 18.0);
        assert!(padded.right > y2.right);
    }

    #[test]
    fn empty_secondary_placeholder_moves_outside_an_external_legend_lane() {
        let axes = instplot_layout::Bounds {
            x: 20.0,
            y: 20.0,
            width: 80.0,
            height: 50.0,
        };
        let right_legend = instplot_layout::Bounds {
            x: 105.0,
            y: 30.0,
            width: 28.0,
            height: 30.0,
        };
        let ordinary = secondary_axis_placeholder_pad(
            AxisIdentity::Y2,
            axes,
            None,
            egui::vec2(34.0, 11.0),
            1.0,
            7.0,
        );
        let avoided = secondary_axis_placeholder_pad(
            AxisIdentity::Y2,
            axes,
            Some(right_legend),
            egui::vec2(34.0, 11.0),
            1.0,
            7.0,
        );
        assert!(avoided > ordinary);
    }

    #[test]
    fn blank_double_click_ignores_only_the_background_axes_hit() {
        assert!(opens_axis_visibility_from_blank_double_click(
            DrawingTool::Select,
            true,
            Some(SelectableRole::Axes),
            true,
        ));
        assert!(opens_axis_visibility_from_blank_double_click(
            DrawingTool::Select,
            true,
            None,
            true,
        ));
        assert!(!opens_axis_visibility_from_blank_double_click(
            DrawingTool::Select,
            true,
            Some(SelectableRole::Series),
            true,
        ));
        assert!(!opens_axis_visibility_from_blank_double_click(
            DrawingTool::Measurement {
                axes: AxisBinding::PRIMARY,
                constraint: MeasurementConstraint::Free,
                start_arrow: false,
                end_arrow: true,
            },
            true,
            None,
            true,
        ));
    }
}
