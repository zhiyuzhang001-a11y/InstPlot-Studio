use super::*;

pub(super) fn axis_layout(
    axis: &AxisSpec,
    horizontal: bool,
    axes: Bounds,
    measurer: &mut dyn TextMeasurer,
) -> AxisLayout {
    if !axis.has_data {
        return AxisLayout {
            major: Vec::new(),
            minor: Vec::new(),
            shared_exponent: None,
            label: axis.label.clone(),
            duplicate_tick_labels: false,
            long_tick_label: false,
            scaled_ticks_without_visible_factor: false,
        };
    }
    let length = if horizontal { axes.width } else { axes.height };
    let values = axis
        .locator
        .major_ticks(axis.scale, axis.minimum, axis.maximum, length);
    let step = values.windows(2).next().map(|pair| pair[1] - pair[0]);
    let exponent = match axis.display_scale {
        AxisDisplayScale::AutoFactor => {
            crate::automatic_display_exponent(&values, axis.minimum, axis.maximum).unwrap_or(0)
        }
        AxisDisplayScale::None => 0,
        AxisDisplayScale::ManualFactor(exponent)
        | AxisDisplayScale::ManualIncorporated(exponent) => exponent,
    };
    let formatted = crate::format_ticks_with_exponent(&values, step, &axis.formatter, exponent);
    let duplicate_tick_labels = matches!(axis.formatter, Formatter::Decimal { .. })
        && formatted
            .labels
            .iter()
            .enumerate()
            .any(|(index, label)| formatted.labels[index + 1..].contains(label));
    let long_tick_label = formatted
        .labels
        .iter()
        .any(|label| label.chars().count() > 24);
    let positions: Vec<f64> = values
        .iter()
        .map(|value| {
            let fraction = axis
                .scale
                .map(*value, axis.minimum, axis.maximum)
                .unwrap_or(0.0);
            if horizontal {
                axes.x + fraction * axes.width
            } else {
                axes.bottom() - fraction * axes.height
            }
        })
        .collect();
    let sizes: Vec<TextSize> = formatted
        .labels
        .iter()
        .map(|label| measurer.measure(label, TICK_FONT))
        .collect();
    let stride = if horizontal {
        collision_stride(
            &positions,
            &sizes.iter().map(|size| size.width).collect::<Vec<_>>(),
            3.0,
        )
    } else {
        1
    };
    let major = values
        .iter()
        .zip(formatted.labels.iter())
        .zip(positions.iter().zip(sizes.iter()))
        .enumerate()
        .filter(|(index, _)| index % stride == 0)
        .map(|(_, ((value, label), (position, size)))| TickLayout {
            value: *value,
            label: label.clone(),
            position: *position,
            label_bounds: if horizontal {
                Bounds {
                    x: position - size.width / 2.0,
                    y: axes.bottom() + axis.appearance.tick_label_pad_pt,
                    width: size.width,
                    height: size.height,
                }
            } else {
                Bounds {
                    x: axes.x - axis.appearance.tick_label_pad_pt - size.width,
                    y: position - size.height / 2.0,
                    width: size.width,
                    height: size.height,
                }
            },
        })
        .collect();
    AxisLayout {
        major,
        minor: minor_ticks_with_interval(
            axis.scale,
            &values,
            axis.minimum,
            axis.maximum,
            axis.minor_interval,
        ),
        shared_exponent: formatted.shared_exponent,
        label: resolved_axis_label(&axis.label, axis.display_scale, exponent),
        duplicate_tick_labels,
        long_tick_label,
        scaled_ticks_without_visible_factor: exponent != 0
            && axis.appearance.tick_labels_visible
            && !axis.appearance.label_visible,
    }
}

const SCALE_SLOT_SENTINEL: &str = "\u{e000}instplot-scale\u{e001}";

fn resolved_axis_label(
    label: &instplot_text::Label,
    mode: AxisDisplayScale,
    exponent: i32,
) -> instplot_text::Label {
    let factor_visible = matches!(
        mode,
        AxisDisplayScale::AutoFactor | AxisDisplayScale::ManualFactor(_)
    );
    let factor = scale_factor_label(exponent);
    let (mut resolved, replaced) = replace_scale_slot(label, factor_visible.then_some(&factor));
    if factor_visible && exponent != 0 && !replaced {
        resolved = instplot_text::Label::Group(vec![
            resolved,
            instplot_text::Label::Text(" ".to_owned()),
            factor,
        ]);
    }
    resolved
}

fn scale_factor_label(exponent: i32) -> instplot_text::Label {
    instplot_text::Label::Group(vec![
        instplot_text::Label::Operator("×".to_owned()),
        instplot_text::Label::Number("10".to_owned()),
        instplot_text::Label::Superscript(Box::new(instplot_text::Label::Number(
            exponent.to_string().replace('-', "−"),
        ))),
    ])
}

fn replace_scale_slot(
    label: &instplot_text::Label,
    factor: Option<&instplot_text::Label>,
) -> (instplot_text::Label, bool) {
    use instplot_text::Label;
    match label {
        Label::Text(text) if text == SCALE_SLOT_SENTINEL => (
            factor
                .cloned()
                .unwrap_or_else(|| Label::Text(String::new())),
            true,
        ),
        Label::Group(children) => {
            let mut replaced = false;
            let children = children
                .iter()
                .map(|child| {
                    let (child, child_replaced) = replace_scale_slot(child, factor);
                    replaced |= child_replaced;
                    child
                })
                .collect();
            (Label::Group(children), replaced)
        }
        Label::DescriptiveSubscript(child) => {
            let (child, replaced) = replace_scale_slot(child, factor);
            (Label::DescriptiveSubscript(Box::new(child)), replaced)
        }
        Label::VariableSubscript(child) => {
            let (child, replaced) = replace_scale_slot(child, factor);
            (Label::VariableSubscript(Box::new(child)), replaced)
        }
        Label::Superscript(child) => {
            let (child, replaced) = replace_scale_slot(child, factor);
            (Label::Superscript(Box::new(child)), replaced)
        }
        other => (other.clone(), false),
    }
}

pub(super) struct DrawAxesRequest<'a> {
    pub(super) chart: &'a Chart,
    pub(super) axes: Bounds,
    pub(super) x_axis: &'a AxisLayout,
    pub(super) y_axis: &'a AxisLayout,
    pub(super) x2_axis: Option<&'a AxisLayout>,
    pub(super) y2_axis: Option<&'a AxisLayout>,
    pub(super) canvas_bottom: f64,
    pub(super) measurer: &'a mut dyn TextMeasurer,
    pub(super) list: &'a mut DisplayList,
    pub(super) hit_map: &'a mut HitMap,
    pub(super) warnings: &'a mut Vec<LayoutWarning>,
}

pub(super) fn draw_axes(
    request: DrawAxesRequest<'_>,
) -> (Bounds, Bounds, Option<Bounds>, Option<Bounds>) {
    let DrawAxesRequest {
        chart,
        axes,
        x_axis,
        y_axis,
        x2_axis,
        y2_axis,
        canvas_bottom,
        measurer,
        list,
        hit_map,
        warnings,
    } = request;
    draw_grid(chart, axes, x_axis, y_axis, list);
    let x_spine = stroke(
        chart.x.appearance.spine_color,
        AXIS_STROKE_WIDTH_PT,
        DashStyle::Solid,
    );
    let y_spine = stroke(
        chart.y.appearance.spine_color,
        AXIS_STROKE_WIDTH_PT,
        DashStyle::Solid,
    );
    if chart.x.appearance.near_spine {
        grid_line(
            list,
            chart.id,
            (axes.x, axes.bottom()),
            (axes.right(), axes.bottom()),
            &x_spine,
        );
    }
    if chart.x2.is_none() && chart.x.appearance.far_spine {
        grid_line(
            list,
            chart.id,
            (axes.x, axes.y),
            (axes.right(), axes.y),
            &x_spine,
        );
    }
    if chart.y.appearance.near_spine {
        grid_line(
            list,
            chart.id,
            (axes.x, axes.y),
            (axes.x, axes.bottom()),
            &y_spine,
        );
    }
    if chart.y2.is_none() && chart.y.appearance.far_spine {
        grid_line(
            list,
            chart.id,
            (axes.right(), axes.y),
            (axes.right(), axes.bottom()),
            &y_spine,
        );
    }
    for tick in &x_axis.major {
        if chart.x.appearance.major_ticks && chart.x.appearance.near_ticks {
            directional_tick(
                list,
                chart.x.id,
                tick.position,
                axes.bottom(),
                false,
                -1.0,
                4.0,
                chart.x.appearance.tick_direction,
            );
        }
        if chart.x2.is_none() && chart.x.appearance.major_ticks && chart.x.appearance.far_ticks {
            directional_tick(
                list,
                chart.x.id,
                tick.position,
                axes.y,
                false,
                1.0,
                4.0,
                chart.x.appearance.tick_direction,
            );
        }
        let size = measurer.measure(&tick.label, TICK_FONT);
        if chart.x.appearance.tick_labels_visible && chart.x.appearance.near_tick_labels {
            text(
                list,
                chart.x.id,
                &tick.label,
                (tick.position, tick.label_bounds.y + size.ascent),
                TICK_FONT,
                TextAnchor::Middle,
                0.0,
            );
            hit_map.items.push(HitItem {
                node: chart.x.id,
                bounds: tick.label_bounds,
                z_order: 20,
                role: SelectableRole::Tick,
                data_index: None,
                tooltip: Some(format!("x = {}", tick.value)),
                path_proximity: vec![(tick.position, axes.bottom())],
            });
        }
        if chart.x.appearance.tick_labels_visible
            && chart.x2.is_none()
            && chart.x.appearance.far_tick_labels
        {
            let bounds = Bounds {
                x: tick.position - size.width / 2.0,
                y: axes.y - chart.x.appearance.tick_label_pad_pt - size.height,
                width: size.width,
                height: size.height,
            };
            text(
                list,
                chart.x.id,
                &tick.label,
                (tick.position, bounds.y + size.ascent),
                TICK_FONT,
                TextAnchor::Middle,
                0.0,
            );
            hit_map.items.push(HitItem {
                node: chart.x.id,
                bounds,
                z_order: 20,
                role: SelectableRole::Tick,
                data_index: None,
                tooltip: Some(format!("x = {}", tick.value)),
                path_proximity: vec![(tick.position, axes.y)],
            });
        }
    }
    for value in &x_axis.minor {
        if let Some(fraction) = chart.x.scale.map(*value, chart.x.minimum, chart.x.maximum) {
            if chart.x.appearance.minor_ticks && chart.x.appearance.near_ticks {
                directional_tick(
                    list,
                    chart.x.id,
                    axes.x + fraction * axes.width,
                    axes.bottom(),
                    false,
                    -1.0,
                    2.0,
                    chart.x.appearance.tick_direction,
                );
            }
            if chart.x2.is_none() && chart.x.appearance.minor_ticks && chart.x.appearance.far_ticks
            {
                directional_tick(
                    list,
                    chart.x.id,
                    axes.x + fraction * axes.width,
                    axes.y,
                    false,
                    1.0,
                    2.0,
                    chart.x.appearance.tick_direction,
                );
            }
        }
    }
    for tick in &y_axis.major {
        if chart.y.appearance.major_ticks && chart.y.appearance.near_ticks {
            directional_tick(
                list,
                chart.y.id,
                axes.x,
                tick.position,
                true,
                1.0,
                4.0,
                chart.y.appearance.tick_direction,
            );
        }
        if chart.y2.is_none() && chart.y.appearance.major_ticks && chart.y.appearance.far_ticks {
            directional_tick(
                list,
                chart.y.id,
                axes.right(),
                tick.position,
                true,
                -1.0,
                4.0,
                chart.y.appearance.tick_direction,
            );
        }
        let size = measurer.measure(&tick.label, TICK_FONT);
        if chart.y.appearance.tick_labels_visible && chart.y.appearance.near_tick_labels {
            text(
                list,
                chart.y.id,
                &tick.label,
                (
                    tick.label_bounds.x,
                    tick.position + (size.ascent - size.descent) / 2.0,
                ),
                TICK_FONT,
                TextAnchor::Start,
                0.0,
            );
            hit_map.items.push(HitItem {
                node: chart.y.id,
                bounds: tick.label_bounds,
                z_order: 20,
                role: SelectableRole::Tick,
                data_index: None,
                tooltip: Some(format!("y = {}", tick.value)),
                path_proximity: vec![(axes.x, tick.position)],
            });
        }
        if chart.y.appearance.tick_labels_visible
            && chart.y2.is_none()
            && chart.y.appearance.far_tick_labels
        {
            let bounds = Bounds {
                x: axes.right() + chart.y.appearance.tick_label_pad_pt,
                y: tick.position - size.height / 2.0,
                width: size.width,
                height: size.height,
            };
            text(
                list,
                chart.y.id,
                &tick.label,
                (bounds.x, tick.position + (size.ascent - size.descent) / 2.0),
                TICK_FONT,
                TextAnchor::Start,
                0.0,
            );
            hit_map.items.push(HitItem {
                node: chart.y.id,
                bounds,
                z_order: 20,
                role: SelectableRole::Tick,
                data_index: None,
                tooltip: Some(format!("y = {}", tick.value)),
                path_proximity: vec![(axes.right(), tick.position)],
            });
        }
    }
    for value in &y_axis.minor {
        if let Some(fraction) = chart.y.scale.map(*value, chart.y.minimum, chart.y.maximum) {
            if chart.y.appearance.minor_ticks && chart.y.appearance.near_ticks {
                directional_tick(
                    list,
                    chart.y.id,
                    axes.x,
                    axes.bottom() - fraction * axes.height,
                    true,
                    1.0,
                    2.0,
                    chart.y.appearance.tick_direction,
                );
            }
            if chart.y2.is_none() && chart.y.appearance.minor_ticks && chart.y.appearance.far_ticks
            {
                directional_tick(
                    list,
                    chart.y.id,
                    axes.right(),
                    axes.bottom() - fraction * axes.height,
                    true,
                    -1.0,
                    2.0,
                    chart.y.appearance.tick_direction,
                );
            }
        }
    }

    let x_size = measurer.measure_label(&x_axis.label, LABEL_FONT);
    let x_bounds = if chart.x.appearance.label_visible {
        Bounds {
            x: axes.x + (axes.width - x_size.width) / 2.0,
            y: list.height.get()
                - canvas_bottom
                - chart.x.appearance.label_edge_pad_pt
                - x_size.height,
            width: x_size.width,
            height: x_size.height,
        }
    } else {
        Bounds {
            x: axes.x + axes.width / 2.0,
            y: axes.bottom(),
            width: 0.0,
            height: 0.0,
        }
    };
    if chart.x.appearance.label_visible {
        label_text(
            list,
            chart.x.id,
            &x_axis.label,
            (axes.x + axes.width / 2.0, x_bounds.y + x_size.ascent),
            LABEL_FONT,
            TextAnchor::Middle,
            0.0,
        );
    }
    let y_size = measurer.measure_label(&y_axis.label, LABEL_FONT);
    let y_bounds = if chart.y.appearance.label_visible {
        Bounds {
            x: chart.y.appearance.label_edge_pad_pt,
            y: axes.y + (axes.height - y_size.width) / 2.0,
            width: y_size.height,
            height: y_size.width,
        }
    } else {
        Bounds {
            x: axes.x,
            y: axes.y + axes.height / 2.0,
            width: 0.0,
            height: 0.0,
        }
    };
    if chart.y.appearance.label_visible {
        label_text(
            list,
            chart.y.id,
            &y_axis.label,
            (
                y_bounds.right() - y_size.descent,
                axes.y + axes.height / 2.0,
            ),
            LABEL_FONT,
            TextAnchor::Middle,
            -90.0,
        );
    }
    let figure = Bounds {
        x: 0.0,
        y: 0.0,
        width: list.width.get(),
        height: list.height.get(),
    };
    for (visible, node, value, bounds) in [
        (
            chart.x.appearance.label_visible,
            chart.x.id,
            &x_axis.label,
            x_bounds,
        ),
        (
            chart.y.appearance.label_visible,
            chart.y.id,
            &y_axis.label,
            y_bounds,
        ),
    ] {
        if visible && !figure.contains(bounds) {
            warnings.push(LayoutWarning::TextOutsideFigure {
                node,
                text: value.normalized_text(),
            });
        }
    }
    for (visible, node, bounds) in [
        (chart.x.appearance.label_visible, chart.x.id, x_bounds),
        (chart.y.appearance.label_visible, chart.y.id, y_bounds),
    ] {
        if visible {
            hit_map.items.push(HitItem {
                node,
                bounds,
                z_order: 22,
                role: SelectableRole::AxisLabel,
                data_index: None,
                tooltip: None,
                path_proximity: Vec::new(),
            });
        }
    }
    if chart.x.appearance.near_spine {
        hit_map.items.push(axis_hit_item(
            chart.x.id,
            Bounds {
                x: axes.x,
                y: axes.bottom(),
                width: axes.width,
                height: 0.0,
            },
        ));
    }
    if chart.x2.is_none() && chart.x.appearance.far_spine {
        hit_map.items.push(axis_hit_item(
            chart.x.id,
            Bounds {
                x: axes.x,
                y: axes.y,
                width: axes.width,
                height: 0.0,
            },
        ));
    }
    if chart.y.appearance.near_spine {
        hit_map.items.push(axis_hit_item(
            chart.y.id,
            Bounds {
                x: axes.x,
                y: axes.y,
                width: 0.0,
                height: axes.height,
            },
        ));
    }
    if chart.y2.is_none() && chart.y.appearance.far_spine {
        hit_map.items.push(axis_hit_item(
            chart.y.id,
            Bounds {
                x: axes.right(),
                y: axes.y,
                width: 0.0,
                height: axes.height,
            },
        ));
    }
    let x2_bounds = chart.x2.as_ref().zip(x2_axis).map(|(axis, layout)| {
        draw_secondary_x_axis(axis, layout, axes, measurer, list, hit_map, warnings)
    });
    let y2_bounds = chart.y2.as_ref().zip(y2_axis).map(|(axis, layout)| {
        draw_secondary_y_axis(axis, layout, axes, measurer, list, hit_map, warnings)
    });
    (x_bounds, y_bounds, x2_bounds, y2_bounds)
}

fn axis_hit_item(node: NodeId, bounds: Bounds) -> HitItem {
    HitItem {
        node,
        bounds,
        z_order: 18,
        role: SelectableRole::Axis,
        data_index: None,
        tooltip: None,
        path_proximity: Vec::new(),
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_secondary_x_axis(
    axis: &AxisSpec,
    layout: &AxisLayout,
    axes: Bounds,
    measurer: &mut dyn TextMeasurer,
    list: &mut DisplayList,
    hit_map: &mut HitMap,
    warnings: &mut Vec<LayoutWarning>,
) -> Bounds {
    let spine = stroke(
        axis.appearance.spine_color,
        AXIS_STROKE_WIDTH_PT,
        DashStyle::Solid,
    );
    if axis.appearance.near_spine {
        grid_line(
            list,
            axis.id,
            (axes.x, axes.y),
            (axes.right(), axes.y),
            &spine,
        );
        hit_map.items.push(axis_hit_item(
            axis.id,
            Bounds {
                x: axes.x,
                y: axes.y,
                width: axes.width,
                height: 0.0,
            },
        ));
    }
    let mut max_tick_height: f64 = 0.0;
    for tick in &layout.major {
        if axis.appearance.major_ticks && axis.appearance.near_ticks {
            directional_tick(
                list,
                axis.id,
                tick.position,
                axes.y,
                false,
                1.0,
                4.0,
                axis.appearance.tick_direction,
            );
        }
        if axis.appearance.tick_labels_visible && axis.appearance.near_tick_labels {
            let size = measurer.measure(&tick.label, TICK_FONT);
            max_tick_height = max_tick_height.max(size.height);
            let bounds = Bounds {
                x: tick.position - size.width / 2.0,
                y: axes.y - axis.appearance.tick_label_pad_pt - size.height,
                width: size.width,
                height: size.height,
            };
            text(
                list,
                axis.id,
                &tick.label,
                (tick.position, bounds.y + size.ascent),
                TICK_FONT,
                TextAnchor::Middle,
                0.0,
            );
            hit_map.items.push(HitItem {
                node: axis.id,
                bounds,
                z_order: 20,
                role: SelectableRole::Tick,
                data_index: None,
                tooltip: Some(format!("x2 = {}", tick.value)),
                path_proximity: vec![(tick.position, axes.y)],
            });
        }
    }
    for value in &layout.minor {
        if let Some(fraction) = axis.scale.map(*value, axis.minimum, axis.maximum)
            && axis.appearance.minor_ticks
            && axis.appearance.near_ticks
        {
            directional_tick(
                list,
                axis.id,
                axes.x + fraction * axes.width,
                axes.y,
                false,
                1.0,
                2.0,
                axis.appearance.tick_direction,
            );
        }
    }
    let size = measurer.measure_label(&layout.label, LABEL_FONT);
    let label_bottom = axes.y
        - if axis.has_data && axis.appearance.near_tick_labels {
            max_tick_height + axis.appearance.tick_label_pad_pt
        } else {
            0.0
        }
        - axis.appearance.label_tick_pad_pt;
    let bounds = if axis.appearance.label_visible {
        Bounds {
            x: axes.x + (axes.width - size.width) / 2.0,
            y: label_bottom - size.height,
            width: size.width,
            height: size.height,
        }
    } else {
        Bounds {
            x: axes.x + axes.width / 2.0,
            y: axes.y,
            width: 0.0,
            height: 0.0,
        }
    };
    if axis.appearance.label_visible {
        label_text(
            list,
            axis.id,
            &layout.label,
            (axes.x + axes.width / 2.0, bounds.y + size.ascent),
            LABEL_FONT,
            TextAnchor::Middle,
            0.0,
        );
        register_secondary_label(axis, &layout.label, bounds, list, hit_map, warnings);
    }
    bounds
}

#[allow(clippy::too_many_arguments)]
fn draw_secondary_y_axis(
    axis: &AxisSpec,
    layout: &AxisLayout,
    axes: Bounds,
    measurer: &mut dyn TextMeasurer,
    list: &mut DisplayList,
    hit_map: &mut HitMap,
    warnings: &mut Vec<LayoutWarning>,
) -> Bounds {
    let spine = stroke(
        axis.appearance.spine_color,
        AXIS_STROKE_WIDTH_PT,
        DashStyle::Solid,
    );
    if axis.appearance.near_spine {
        grid_line(
            list,
            axis.id,
            (axes.right(), axes.y),
            (axes.right(), axes.bottom()),
            &spine,
        );
        hit_map.items.push(axis_hit_item(
            axis.id,
            Bounds {
                x: axes.right(),
                y: axes.y,
                width: 0.0,
                height: axes.height,
            },
        ));
    }
    let mut max_tick_width: f64 = 0.0;
    for tick in &layout.major {
        if axis.appearance.major_ticks && axis.appearance.near_ticks {
            directional_tick(
                list,
                axis.id,
                axes.right(),
                tick.position,
                true,
                -1.0,
                4.0,
                axis.appearance.tick_direction,
            );
        }
        if axis.appearance.tick_labels_visible && axis.appearance.near_tick_labels {
            let size = measurer.measure(&tick.label, TICK_FONT);
            max_tick_width = max_tick_width.max(size.width);
            let bounds = Bounds {
                x: axes.right() + axis.appearance.tick_label_pad_pt,
                y: tick.position - size.height / 2.0,
                width: size.width,
                height: size.height,
            };
            text(
                list,
                axis.id,
                &tick.label,
                (bounds.x, tick.position + (size.ascent - size.descent) / 2.0),
                TICK_FONT,
                TextAnchor::Start,
                0.0,
            );
            hit_map.items.push(HitItem {
                node: axis.id,
                bounds,
                z_order: 20,
                role: SelectableRole::Tick,
                data_index: None,
                tooltip: Some(format!("y2 = {}", tick.value)),
                path_proximity: vec![(axes.right(), tick.position)],
            });
        }
    }
    for value in &layout.minor {
        if let Some(fraction) = axis.scale.map(*value, axis.minimum, axis.maximum)
            && axis.appearance.minor_ticks
            && axis.appearance.near_ticks
        {
            directional_tick(
                list,
                axis.id,
                axes.right(),
                axes.bottom() - fraction * axes.height,
                true,
                -1.0,
                2.0,
                axis.appearance.tick_direction,
            );
        }
    }
    let size = measurer.measure_label(&layout.label, LABEL_FONT);
    let left = axes.right()
        + if axis.has_data && axis.appearance.near_tick_labels {
            max_tick_width + axis.appearance.tick_label_pad_pt
        } else {
            0.0
        }
        + axis.appearance.label_tick_pad_pt;
    let bounds = if axis.appearance.label_visible {
        Bounds {
            x: left,
            y: axes.y + (axes.height - size.width) / 2.0,
            width: size.height,
            height: size.width,
        }
    } else {
        Bounds {
            x: axes.right(),
            y: axes.y + axes.height / 2.0,
            width: 0.0,
            height: 0.0,
        }
    };
    if axis.appearance.label_visible {
        label_text(
            list,
            axis.id,
            &layout.label,
            (bounds.right() - size.descent, axes.y + axes.height / 2.0),
            LABEL_FONT,
            TextAnchor::Middle,
            -90.0,
        );
        register_secondary_label(axis, &layout.label, bounds, list, hit_map, warnings);
    }
    bounds
}

fn register_secondary_label(
    axis: &AxisSpec,
    label: &instplot_text::Label,
    bounds: Bounds,
    list: &DisplayList,
    hit_map: &mut HitMap,
    warnings: &mut Vec<LayoutWarning>,
) {
    let figure = Bounds {
        x: 0.0,
        y: 0.0,
        width: list.width.get(),
        height: list.height.get(),
    };
    if !figure.contains(bounds) {
        warnings.push(LayoutWarning::TextOutsideFigure {
            node: axis.id,
            text: label.normalized_text(),
        });
    }
    hit_map.items.push(HitItem {
        node: axis.id,
        bounds,
        z_order: 22,
        role: SelectableRole::AxisLabel,
        data_index: None,
        tooltip: None,
        path_proximity: Vec::new(),
    });
}

fn draw_grid(
    chart: &Chart,
    axes: Bounds,
    x_axis: &AxisLayout,
    y_axis: &AxisLayout,
    list: &mut DisplayList,
) {
    let major = stroke(Color(218, 221, 224, 255), 0.4, DashStyle::Solid);
    let minor = stroke(Color(232, 234, 236, 255), 0.3, DashStyle::Dotted);
    if chart.x.grid.minor {
        for value in &x_axis.minor {
            if let Some(fraction) = chart.x.scale.map(*value, chart.x.minimum, chart.x.maximum) {
                grid_line(
                    list,
                    chart.x.id,
                    (axes.x + fraction * axes.width, axes.y),
                    (axes.x + fraction * axes.width, axes.bottom()),
                    &minor,
                );
            }
        }
    }
    if chart.y.grid.minor {
        for value in &y_axis.minor {
            if let Some(fraction) = chart.y.scale.map(*value, chart.y.minimum, chart.y.maximum) {
                let y = axes.bottom() - fraction * axes.height;
                grid_line(list, chart.y.id, (axes.x, y), (axes.right(), y), &minor);
            }
        }
    }
    if chart.x.grid.major {
        for tick in &x_axis.major {
            grid_line(
                list,
                chart.x.id,
                (tick.position, axes.y),
                (tick.position, axes.bottom()),
                &major,
            );
        }
    }
    if chart.y.grid.major {
        for tick in &y_axis.major {
            grid_line(
                list,
                chart.y.id,
                (axes.x, tick.position),
                (axes.right(), tick.position),
                &major,
            );
        }
    }
}

fn grid_line(
    list: &mut DisplayList,
    node: NodeId,
    start: (f64, f64),
    end: (f64, f64),
    style: &Stroke,
) {
    list.items.push(DisplayItem::Path {
        source: node,
        path: Path {
            verbs: vec![
                PathVerb::MoveTo(pt(start.0), pt(start.1)),
                PathVerb::LineTo(pt(end.0), pt(end.1)),
            ],
        },
        fill: None,
        stroke: Some(style.clone()),
    });
}
