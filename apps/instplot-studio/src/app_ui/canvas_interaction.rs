use super::*;

pub(crate) fn reference_tool_allows_object_interaction(
    drawing_tool: DrawingTool,
    hovered_role: Option<SelectableRole>,
    pressed_role: Option<SelectableRole>,
    active_drag_role: Option<SelectableRole>,
) -> bool {
    matches!(drawing_tool, DrawingTool::Reference { .. })
        && [hovered_role, pressed_role, active_drag_role]
            .into_iter()
            .flatten()
            .any(|role| role == SelectableRole::ReferenceLine)
}

pub(crate) fn opens_axis_visibility_from_blank_double_click(
    drawing_tool: DrawingTool,
    double_clicked: bool,
    hovered_role: Option<SelectableRole>,
    pointer_in_axes: bool,
) -> bool {
    drawing_tool == DrawingTool::Select
        && double_clicked
        && hovered_role.is_none_or(|role| role == SelectableRole::Axes)
        && pointer_in_axes
}
