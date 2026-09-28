use super::*;

impl StudioApp {
    pub(crate) fn artist_editor(&mut self, ui: &mut egui::Ui, artist_id: &str, compact: bool) {
        let Some(mut record) = self.document.artist_record(artist_id) else {
            return;
        };
        let reference_autoscale_before = matches!(
            record.properties,
            ArtistProperties::ReferenceLine {
                include_in_autoscale: true,
                ..
            }
        );
        let palette = self.document.palette_colors().to_vec();
        let (canvas_width_mm, canvas_height_mm) = self.document.figure_size_mm();
        let mut changed = false;
        let mut continuous_change = false;
        let mut finish_coalescing = false;
        let mut semantic_labels = Vec::new();
        let mut apply_marker_size_to_all = None;
        let mut apply_marker_interval_to_all = None;
        let mut apply_marker_fill_to_all = None;
        let mut series_style_change = None;
        let mut axis_binding_change = None;
        let mut measurement_label_change = None;
        let mut delete_drawing_object = false;
        let properties_title = self.language.text(Text::ArtistProperties);
        let mut draw_properties = |ui: &mut egui::Ui| {
            let control_height = ui.spacing().interact_size.y;
            if matches!(record.role, ArtistRole::Annotation | ArtistRole::Legend) {
                changed |= ui
                    .checkbox(&mut record.visible, self.language.text(Text::Visible))
                    .changed();
            }
            if !compact && !matches!(record.role, ArtistRole::Annotation | ArtistRole::Legend) {
                changed |= role_editor(ui, self.language, &mut record.role);
            }
            if matches!(
                record.properties,
                ArtistProperties::Line { .. } | ArtistProperties::Scatter { .. }
            ) && let Some(current_style) = self.document.series_style(artist_id)
            {
                let mut selected_style = current_style;
                ui.horizontal(|ui| {
                    ui.label("图形类型");
                    egui::ComboBox::from_id_salt(("series-style", artist_id))
                        .selected_text(series_style_name(self.language, selected_style))
                        .show_ui(ui, |ui| {
                            for style in [
                                SeriesCreationStyle::Scatter,
                                SeriesCreationStyle::Line,
                                SeriesCreationStyle::LineAndMarker,
                            ] {
                                ui.selectable_value(
                                    &mut selected_style,
                                    style,
                                    series_style_name(self.language, style),
                                );
                            }
                        });
                });
                if selected_style != current_style {
                    series_style_change = Some(selected_style);
                }
            }
            if matches!(
                record.properties,
                ArtistProperties::Line { .. } | ArtistProperties::Scatter { .. }
            ) && let Some(current_axes) = self.document.series_axis_binding(artist_id)
            {
                let mode = self.document.axis_mode();
                let mut selected_axes = current_axes;
                ui.horizontal(|ui| {
                    ui.label("坐标轴");
                    egui::ComboBox::from_id_salt(("series-axis-binding", artist_id))
                        .selected_text(axis_binding_name(current_axes))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut selected_axes,
                                AxisBinding::PRIMARY,
                                "X1 / Y1",
                            );
                            if mode == AxisMode::DualY {
                                ui.selectable_value(
                                    &mut selected_axes,
                                    AxisBinding {
                                        x: XAxisSlot::X1,
                                        y: YAxisSlot::Y2,
                                    },
                                    "X1 / Y2",
                                );
                            }
                            if mode == AxisMode::DualX {
                                ui.selectable_value(
                                    &mut selected_axes,
                                    AxisBinding {
                                        x: XAxisSlot::X2,
                                        y: YAxisSlot::Y1,
                                    },
                                    "X2 / Y1",
                                );
                            }
                        });
                });
                if !current_axes.is_enabled_in(mode) {
                    ui.weak("该曲线绑定的副轴当前已关闭；重新启用对应模式后会恢复显示。");
                }
                for warning in explicit_axis_unit_conflicts(&self.document, current_axes) {
                    ui.colored_label(egui::Color32::YELLOW, warning);
                }
                if selected_axes != current_axes {
                    axis_binding_change = Some(selected_axes);
                }
            }
            match &mut record.properties {
                ArtistProperties::Line { stroke, .. } => {
                    let edit = stroke_editor(ui, self.language, artist_id, stroke, &palette);
                    changed |= edit.changed;
                    continuous_change |= edit.continuous;
                    finish_coalescing |= edit.finish;
                }
                ArtistProperties::Scatter { marker, .. } => {
                    changed |=
                        color_editor(ui, self.language, artist_id, &mut marker.color_id, &palette);
                    ui.label(self.language.text(Text::MarkerShape));
                    let preview_color = palette
                        .iter()
                        .find(|color| color.id == marker.color_id)
                        .map(|color| {
                            let [r, g, b, _] = color.rgba;
                            egui::Color32::from_rgb(r, g, b)
                        })
                        .unwrap_or(egui::Color32::from_rgb(68, 119, 170));
                    let tile_width = ((ui.available_width() - 100.0) / 2.0).max(96.0);
                    egui::Grid::new(("marker-shape-grid", artist_id))
                        .num_columns(3)
                        .spacing(egui::vec2(8.0, 8.0))
                        .show(ui, |ui| {
                            for shape in PRODUCT_MARKER_SHAPES {
                                ui.label(marker_shape_name(self.language, shape));
                                for (filled, text) in
                                    [(true, Text::FilledMarker), (false, Text::HollowMarker)]
                                {
                                    let response = ui.add_sized(
                                        egui::vec2(tile_width, 40.0),
                                        egui::Button::new(format!(
                                            "    {}",
                                            self.language.text(text)
                                        ))
                                        .selected(marker.shape == shape && marker.filled == filled),
                                    );
                                    paint_marker_preview(
                                        ui.painter(),
                                        response.rect,
                                        shape,
                                        filled,
                                        preview_color,
                                    );
                                    if response.clicked() {
                                        marker.shape = shape;
                                        marker.filled = filled;
                                        changed = true;
                                        if self.marker_fill_for_all {
                                            apply_marker_fill_to_all = Some(filled);
                                        }
                                    }
                                }
                                ui.end_row();
                            }
                        });
                    if ui
                        .checkbox(&mut self.marker_fill_for_all, "实心/空心应用于全部曲线")
                        .changed()
                        && self.marker_fill_for_all
                    {
                        apply_marker_fill_to_all = Some(marker.filled);
                    }
                    if matches!(marker.shape, MarkerShape::Plus | MarkerShape::Cross) {
                        ui.weak(marker_shape_name(self.language, marker.shape));
                    }
                    let (response, apply_on_check) = ui
                        .horizontal(|ui| {
                            let response = ui.add_sized(
                                [144.0, control_height],
                                egui::DragValue::new(&mut marker.size_pt)
                                    .range(0.1..=72.0)
                                    .prefix(format!("{}: ", self.language.text(Text::MarkerSize))),
                            );
                            let apply_on_check = ui
                                .checkbox(&mut self.marker_size_for_all, "全部曲线")
                                .changed()
                                && self.marker_size_for_all;
                            (response, apply_on_check)
                        })
                        .inner;
                    let edit = continuous_edit(&response);
                    changed |= edit.changed;
                    continuous_change |= edit.continuous;
                    finish_coalescing |= edit.finish;
                    if (edit.changed && self.marker_size_for_all) || apply_on_check {
                        apply_marker_size_to_all = Some(marker.size_pt);
                    }
                    let (response, apply_on_check) = ui
                        .horizontal(|ui| {
                            let response = ui.add_sized(
                                [144.0, control_height],
                                egui::DragValue::new(&mut marker.interval)
                                    .range(1..=100_000)
                                    .prefix("每隔点数: "),
                            );
                            let apply_on_check = ui
                                .checkbox(&mut self.marker_interval_for_all, "全部曲线")
                                .changed()
                                && self.marker_interval_for_all;
                            (response, apply_on_check)
                        })
                        .inner;
                    let edit = continuous_edit(&response);
                    changed |= edit.changed;
                    continuous_change |= edit.continuous;
                    finish_coalescing |= edit.finish;
                    if (edit.changed && self.marker_interval_for_all) || apply_on_check {
                        apply_marker_interval_to_all = Some(marker.interval);
                    }
                }
                ArtistProperties::ErrorBar {
                    cap_width_pt,
                    stroke,
                    ..
                } => {
                    let response = ui.add(
                        egui::DragValue::new(cap_width_pt)
                            .range(0.1..=72.0)
                            .prefix(format!("{}: ", self.language.text(Text::CapWidth))),
                    );
                    let edit = continuous_edit(&response);
                    changed |= edit.changed;
                    continuous_change |= edit.continuous;
                    finish_coalescing |= edit.finish;
                    let edit = stroke_editor(ui, self.language, artist_id, stroke, &palette);
                    changed |= edit.changed;
                    continuous_change |= edit.continuous;
                    finish_coalescing |= edit.finish;
                }
                ArtistProperties::ReferenceLine {
                    orientation,
                    value,
                    axes,
                    stroke,
                    include_in_autoscale,
                } => {
                    changed |= reference_axis_binding_editor(
                        ui,
                        ("reference-axis-binding", artist_id),
                        self.document.axis_mode(),
                        *orientation,
                        axes,
                    );
                    let response = ui
                        .horizontal_wrapped(|ui| {
                            ui.label(self.language.text(Text::Orientation));
                            egui::ComboBox::from_id_salt(("reference-orientation", artist_id))
                                .selected_text(reference_orientation_name(
                                    self.language,
                                    *orientation,
                                ))
                                .show_ui(ui, |ui| {
                                    for candidate in [
                                        ReferenceOrientation::Horizontal,
                                        ReferenceOrientation::Vertical,
                                    ] {
                                        changed |= ui
                                            .selectable_value(
                                                orientation,
                                                candidate,
                                                reference_orientation_name(
                                                    self.language,
                                                    candidate,
                                                ),
                                            )
                                            .changed();
                                    }
                                });
                            ui.add_sized(
                                [120.0, control_height],
                                egui::DragValue::new(value)
                                    .prefix(format!("{}: ", self.language.text(Text::Value))),
                            )
                        })
                        .inner;
                    let edit = continuous_edit(&response);
                    changed |= edit.changed;
                    continuous_change |= edit.continuous;
                    finish_coalescing |= edit.finish;
                    if !self
                        .document
                        .reference_value_is_visible(*orientation, *axes, *value)
                    {
                        ui.colored_label(
                            egui::Color32::YELLOW,
                            "当前值超出坐标轴范围；对象已保存，但画布中不可见。",
                        );
                    }
                    changed |= ui.checkbox(include_in_autoscale, "纳入自动范围").changed();
                    let edit = stroke_editor(ui, self.language, artist_id, stroke, &palette);
                    changed |= edit.changed;
                    continuous_change |= edit.continuous;
                    finish_coalescing |= edit.finish;
                    delete_drawing_object |= ui.button("删除参考线").clicked();
                }
                ArtistProperties::MeasurementArrow {
                    start_x,
                    start_y,
                    end_x,
                    end_y,
                    axes,
                    stroke,
                    start_arrow,
                    end_arrow,
                    arrow_head,
                    arrow_size_pt,
                    constraint,
                    label_id,
                    label_offset_x_pt,
                    label_offset_y_pt,
                } => {
                    changed |= axis_binding_editor(
                        ui,
                        ("measurement-arrow-axis-binding", artist_id),
                        self.document.axis_mode(),
                        axes,
                    );
                    egui::Grid::new(("measurement-endpoints", artist_id))
                        .num_columns(5)
                        .spacing(egui::vec2(8.0, 6.0))
                        .show(ui, |ui| {
                            for (name, x, y) in [
                                ("起点", &mut *start_x, &mut *start_y),
                                ("终点", &mut *end_x, &mut *end_y),
                            ] {
                                ui.strong(name);
                                ui.label("X");
                                let x_response = ui.add_sized(
                                    [112.0, control_height],
                                    egui::DragValue::new(x).speed(0.1),
                                );
                                ui.label("Y");
                                let y_response = ui.add_sized(
                                    [112.0, control_height],
                                    egui::DragValue::new(y).speed(0.1),
                                );
                                for response in [x_response, y_response] {
                                    let edit = continuous_edit(&response);
                                    changed |= edit.changed;
                                    continuous_change |= edit.continuous;
                                    finish_coalescing |= edit.finish;
                                }
                                ui.end_row();
                            }
                        });
                    if !self.document.measurement_points_are_visible(
                        *axes,
                        (*start_x, *start_y),
                        (*end_x, *end_y),
                    ) {
                        ui.colored_label(
                            egui::Color32::YELLOW,
                            "一个或多个端点超出坐标轴范围；对象已保存，但超出部分不可见。",
                        );
                    }
                    match constraint {
                        MeasurementConstraint::Horizontal => {
                            let mut delta = *end_x - *start_x;
                            let response = ui.add_sized(
                                [120.0, control_height],
                                egui::DragValue::new(&mut delta).prefix("ΔX: "),
                            );
                            if response.changed() {
                                *end_x = *start_x + delta;
                                *end_y = *start_y;
                                changed = true;
                            }
                        }
                        MeasurementConstraint::Vertical => {
                            let mut delta = *end_y - *start_y;
                            let response = ui.add_sized(
                                [120.0, control_height],
                                egui::DragValue::new(&mut delta).prefix("ΔY: "),
                            );
                            if response.changed() {
                                *end_y = *start_y + delta;
                                *end_x = *start_x;
                                changed = true;
                            }
                        }
                        MeasurementConstraint::Free => {
                            if let Some((start, end)) = data_point_to_local(
                                &self.document,
                                self.resolved.layout.result.axes,
                                *axes,
                                (*start_x, *start_y),
                            )
                            .zip(data_point_to_local(
                                &self.document,
                                self.resolved.layout.result.axes,
                                *axes,
                                (*end_x, *end_y),
                            )) {
                                let angle = (end.1 - start.1).atan2(end.0 - start.0).to_degrees();
                                ui.weak(format!("屏幕角度: {angle:.1}°"));
                            }
                        }
                    }
                    ui.horizontal(|ui| {
                        ui.label("方向约束");
                        for (candidate, name) in [
                            (MeasurementConstraint::Free, "自由"),
                            (MeasurementConstraint::Horizontal, "水平"),
                            (MeasurementConstraint::Vertical, "垂直"),
                        ] {
                            changed |= ui.selectable_value(constraint, candidate, name).changed();
                        }
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.label("箭头");
                        changed |= ui.checkbox(start_arrow, "起点箭头").changed();
                        changed |= ui.checkbox(end_arrow, "末端箭头").changed();
                        egui::ComboBox::from_id_salt(("measurement-arrow-head", artist_id))
                            .selected_text(match arrow_head {
                                ArrowHead::Open => "空心箭头",
                                ArrowHead::Filled => "实心箭头",
                            })
                            .show_ui(ui, |ui| {
                                changed |= ui
                                    .selectable_value(arrow_head, ArrowHead::Open, "空心箭头")
                                    .changed();
                                changed |= ui
                                    .selectable_value(arrow_head, ArrowHead::Filled, "实心箭头")
                                    .changed();
                            });
                    });
                    stroke.color_id = "object-black".to_owned();
                    ui.horizontal_wrapped(|ui| {
                        ui.label("样式");
                        let size_response = ui.add_sized(
                            [120.0, control_height],
                            egui::DragValue::new(arrow_size_pt)
                                .range(2.0..=18.0)
                                .prefix("大小 (pt): "),
                        );
                        let width_response = ui.add_sized(
                            [120.0, control_height],
                            egui::DragValue::new(&mut stroke.width_pt)
                                .range(0.7..=12.0)
                                .prefix("线宽 (pt): "),
                        );
                        for response in [size_response, width_response] {
                            let edit = continuous_edit(&response);
                            changed |= edit.changed;
                            continuous_change |= edit.continuous;
                            finish_coalescing |= edit.finish;
                        }
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.label("标签偏移");
                        changed |= ui
                            .add_sized(
                                [112.0, control_height],
                                egui::DragValue::new(label_offset_x_pt).prefix("X: "),
                            )
                            .changed();
                        changed |= ui
                            .add_sized(
                                [112.0, control_height],
                                egui::DragValue::new(label_offset_y_pt).prefix("Y: "),
                            )
                            .changed();
                    });
                    if let Some(label_id) = label_id {
                        if !compact {
                            semantic_labels.push(label_id.clone());
                        } else {
                            self.quick_semantic_label_editor(ui, label_id);
                        }
                        if ui.button("移除标签").clicked() {
                            measurement_label_change = Some(None);
                        }
                    } else if ui.button("添加标签").clicked() {
                        measurement_label_change =
                            Some(Some(vec![LabelNode::Text("Δx".to_owned())]));
                    }
                    delete_drawing_object |= ui.button("删除测量箭头").clicked();
                }
                ArtistProperties::Annotation {
                    label_id,
                    x_pt,
                    y_pt,
                    connectors,
                } => {
                    let edit = position_editor(
                        ui,
                        self.language,
                        x_pt,
                        y_pt,
                        canvas_width_mm,
                        canvas_height_mm,
                    );
                    changed |= edit.changed;
                    continuous_change |= edit.continuous;
                    finish_coalescing |= edit.finish;
                    if !compact {
                        semantic_labels.push(label_id.clone());
                    } else {
                        self.quick_semantic_label_editor(ui, label_id);
                    }
                    ui.separator();
                    ui.label("连接线与箭头");
                    let mut remove_connector = None;
                    let connector_count = connectors.len();
                    for (index, connector) in connectors.iter_mut().enumerate() {
                        let state_id =
                            ui.make_persistent_id(("annotation-connector", artist_id, index));
                        egui::collapsing_header::CollapsingState::load_with_default_open(
                            ui.ctx(),
                            state_id,
                            index + 1 == connector_count,
                        )
                        .show_header(ui, |ui| {
                            ui.strong(format!("连接线 {}", index + 1));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.small_button("×").on_hover_text("删除连接线").clicked()
                                    {
                                        remove_connector = Some(index);
                                    }
                                },
                            );
                        })
                        .body(|ui| {
                            let mode = self.document.axis_mode();
                            let axes_before = connector.axes;
                            ui.horizontal_wrapped(|ui| {
                                ui.label("坐标轴");
                                egui::ComboBox::from_id_salt((
                                    "annotation-connector-axis-binding",
                                    artist_id,
                                    index,
                                ))
                                .selected_text(axis_binding_name(connector.axes))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut connector.axes,
                                        AxisBinding::PRIMARY,
                                        "X1 / Y1",
                                    );
                                    if mode == AxisMode::DualY {
                                        ui.selectable_value(
                                            &mut connector.axes,
                                            AxisBinding {
                                                x: XAxisSlot::X1,
                                                y: YAxisSlot::Y2,
                                            },
                                            "X1 / Y2",
                                        );
                                    }
                                    if mode == AxisMode::DualX {
                                        ui.selectable_value(
                                            &mut connector.axes,
                                            AxisBinding {
                                                x: XAxisSlot::X2,
                                                y: YAxisSlot::Y1,
                                            },
                                            "X2 / Y1",
                                        );
                                    }
                                });
                                let mut arrow_mode =
                                    match (connector.start_arrow, connector.end_arrow) {
                                        (false, false) => 0,
                                        (false, true) => 1,
                                        (true, false) => 2,
                                        (true, true) => 3,
                                    };
                                ui.label("方向");
                                egui::ComboBox::from_id_salt((
                                    "annotation-arrow-mode",
                                    artist_id,
                                    index,
                                ))
                                .selected_text(
                                    ["无箭头", "末端箭头", "起点箭头", "双向箭头"][arrow_mode],
                                )
                                .show_ui(ui, |ui| {
                                    for (candidate, name) in
                                        ["无箭头", "末端箭头", "起点箭头", "双向箭头"]
                                            .into_iter()
                                            .enumerate()
                                    {
                                        changed |= ui
                                            .selectable_value(&mut arrow_mode, candidate, name)
                                            .changed();
                                    }
                                });
                                (connector.start_arrow, connector.end_arrow) = match arrow_mode {
                                    1 => (false, true),
                                    2 => (true, false),
                                    3 => (true, true),
                                    _ => (false, false),
                                };
                            });
                            changed |= connector.axes != axes_before;
                            if !connector.axes.is_enabled_in(mode) {
                                ui.weak("所绑定的副轴当前已关闭；此连接线暂时隐藏。");
                            }
                            ui.horizontal_wrapped(|ui| {
                                ui.label("目标");
                                let x_response = ui.add_sized(
                                    [112.0, control_height],
                                    egui::DragValue::new(&mut connector.target_x).prefix("X: "),
                                );
                                let y_response = ui.add_sized(
                                    [112.0, control_height],
                                    egui::DragValue::new(&mut connector.target_y).prefix("Y: "),
                                );
                                for response in [x_response, y_response] {
                                    let edit = continuous_edit(&response);
                                    changed |= edit.changed;
                                    continuous_change |= edit.continuous;
                                    finish_coalescing |= edit.finish;
                                }
                            });
                            ui.horizontal_wrapped(|ui| {
                                ui.label("箭头");
                                egui::ComboBox::from_id_salt((
                                    "annotation-arrow-head",
                                    artist_id,
                                    index,
                                ))
                                .selected_text(match connector.arrow_head {
                                    ArrowHead::Open => "空心箭头",
                                    ArrowHead::Filled => "实心箭头",
                                })
                                .show_ui(ui, |ui| {
                                    changed |= ui
                                        .selectable_value(
                                            &mut connector.arrow_head,
                                            ArrowHead::Open,
                                            "空心箭头",
                                        )
                                        .changed();
                                    changed |= ui
                                        .selectable_value(
                                            &mut connector.arrow_head,
                                            ArrowHead::Filled,
                                            "实心箭头",
                                        )
                                        .changed();
                                });
                                let response = ui.add_sized(
                                    [120.0, control_height],
                                    egui::DragValue::new(&mut connector.arrow_size_pt)
                                        .range(2.0..=18.0)
                                        .prefix("大小 (pt): "),
                                );
                                let edit = continuous_edit(&response);
                                changed |= edit.changed;
                                continuous_change |= edit.continuous;
                                finish_coalescing |= edit.finish;
                            });
                            let connector_id = format!("{artist_id}-connector-{index}");
                            let edit = stroke_editor(
                                ui,
                                self.language,
                                &connector_id,
                                &mut connector.stroke,
                                &palette,
                            );
                            changed |= edit.changed;
                            continuous_change |= edit.continuous;
                            finish_coalescing |= edit.finish;
                        });
                    }
                    if let Some(index) = remove_connector {
                        let open_states = (0..connectors.len())
                            .map(|state_index| {
                                let state_id = ui.make_persistent_id((
                                    "annotation-connector",
                                    artist_id,
                                    state_index,
                                ));
                                egui::collapsing_header::CollapsingState::load(ui.ctx(), state_id)
                                    .map(|state| state.is_open())
                                    .unwrap_or(state_index + 1 == connectors.len())
                            })
                            .collect::<Vec<_>>();
                        for state_index in 0..connectors.len() {
                            let state_id = ui.make_persistent_id((
                                "annotation-connector",
                                artist_id,
                                state_index,
                            ));
                            if let Some(state) =
                                egui::collapsing_header::CollapsingState::load(ui.ctx(), state_id)
                            {
                                state.remove(ui.ctx());
                            }
                        }
                        connectors.remove(index);
                        for new_index in 0..connectors.len() {
                            let old_index = if new_index < index {
                                new_index
                            } else {
                                new_index + 1
                            };
                            let state_id = ui.make_persistent_id((
                                "annotation-connector",
                                artist_id,
                                new_index,
                            ));
                            let mut state =
                                egui::collapsing_header::CollapsingState::load_with_default_open(
                                    ui.ctx(),
                                    state_id,
                                    false,
                                );
                            state.set_open(open_states[old_index]);
                            state.store(ui.ctx());
                        }
                        changed = true;
                    }
                    if ui.button("＋连接线").clicked() {
                        let state_id = ui.make_persistent_id((
                            "annotation-connector",
                            artist_id,
                            connectors.len(),
                        ));
                        let mut state =
                            egui::collapsing_header::CollapsingState::load_with_default_open(
                                ui.ctx(),
                                state_id,
                                true,
                            );
                        state.set_open(true);
                        state.store(ui.ctx());
                        let axes = &self.document.project().figure.axes[0];
                        connectors.push(AnnotationConnectorRecord {
                            target_x: instplot_layout::finite_midpoint(
                                axes.x.minimum,
                                axes.x.maximum,
                            )
                            .unwrap_or(axes.x.minimum),
                            target_y: instplot_layout::finite_midpoint(
                                axes.y.minimum,
                                axes.y.maximum,
                            )
                            .unwrap_or(axes.y.minimum),
                            axes: AxisBinding::PRIMARY,
                            stroke: StrokeStyle {
                                color_id: ANNOTATION_CONNECTOR_DEFAULT_COLOR_ID.to_owned(),
                                width_pt: ANNOTATION_CONNECTOR_DEFAULT_WIDTH_PT,
                                dash_pt: Vec::new(),
                            },
                            start_arrow: false,
                            end_arrow: true,
                            arrow_head: ArrowHead::Open,
                            arrow_size_pt: 5.0,
                        });
                        changed = true;
                    }
                }
                ArtistProperties::Legend {
                    entries,
                    x_pt,
                    y_pt,
                    placement,
                    position_custom,
                    grid,
                } => {
                    let previous_placement = *placement;
                    let legend_bounds = self.resolved.layout.result.legend;
                    let axes_bounds = self.resolved.layout.result.axes;
                    ui.horizontal(|ui| {
                        ui.label(self.language.text(Text::LegendPlacement));
                        for (candidate, text) in [
                            (LegendPlacement::Auto, Text::Auto),
                            (LegendPlacement::Inside, Text::LegendInside),
                            (LegendPlacement::Above, Text::LegendAbove),
                            (LegendPlacement::Right, Text::LegendRight),
                        ] {
                            let placement_changed = ui
                                .selectable_value(placement, candidate, self.language.text(text))
                                .changed();
                            changed |= placement_changed;
                            if placement_changed {
                                *position_custom = false;
                            }
                        }
                    });
                    let mode_changed = *placement != previous_placement;
                    ui.label(self.language.text(Text::LegendOriginHint));
                    if mode_changed
                        && *placement == LegendPlacement::Inside
                        && let Some(bounds) = legend_bounds
                    {
                        let base_width = canvas_width_mm * 72.0 / 25.4;
                        let base_height = canvas_height_mm * 72.0 / 25.4;
                        let outside_above = bounds.bottom() < axes_bounds.y;
                        let top_extension = if outside_above {
                            f64::from(self.resolved.display.height) - base_height
                        } else {
                            0.0
                        };
                        *x_pt = bounds.x.clamp(0.0, (base_width - bounds.width).max(0.0));
                        *y_pt = (bounds.y - top_extension)
                            .clamp(0.0, (base_height - bounds.height).max(0.0));
                    }
                    if !mode_changed && let Some(bounds) = legend_bounds {
                        let output_width = f64::from(self.resolved.display.width) * 25.4 / 72.0;
                        let output_height = f64::from(self.resolved.display.height) * 25.4 / 72.0;
                        let mut shown_x = bounds.x;
                        let mut shown_y = bounds.y;
                        let edit = position_editor(
                            ui,
                            self.language,
                            &mut shown_x,
                            &mut shown_y,
                            output_width,
                            output_height,
                        );
                        if edit.changed {
                            if *placement == LegendPlacement::Auto {
                                *placement = if bounds.bottom() < axes_bounds.y {
                                    LegendPlacement::Above
                                } else if bounds.x > axes_bounds.right() {
                                    LegendPlacement::Right
                                } else {
                                    LegendPlacement::Inside
                                };
                            }
                            if *placement == LegendPlacement::Right {
                                shown_x = shown_x.max(axes_bounds.right() + 6.0);
                            }
                            *x_pt = shown_x;
                            *y_pt = shown_y;
                            *position_custom = true;
                        }
                        changed |= edit.changed;
                        continuous_change |= edit.continuous;
                        finish_coalescing |= edit.finish;
                    }
                    let visible_entries = entries
                        .iter()
                        .filter(|entry| {
                            entry.visible
                                && self.document.project().figure.artists.iter().any(|artist| {
                                    artist.id == entry.artist_id
                                        && self
                                            .document
                                            .project()
                                            .artist_effectively_visible(artist)
                                })
                        })
                        .count()
                        .max(1);
                    let max_grid = visible_entries.min(usize::from(u8::MAX));
                    let rendered_rows = self
                        .resolved
                        .layout
                        .result
                        .legend
                        .map(|bounds| ((bounds.height - 6.0) / 12.0).round().max(1.0) as usize)
                        .unwrap_or(visible_entries)
                        .clamp(1, visible_entries);
                    let mut rows = match grid {
                        LegendGrid::Rows(value) => usize::from(*value).clamp(1, visible_entries),
                        LegendGrid::Columns(value) => {
                            visible_entries.div_ceil(usize::from(*value).max(1))
                        }
                        LegendGrid::Auto => rendered_rows,
                    };
                    let mut columns = match grid {
                        LegendGrid::Columns(value) => usize::from(*value).clamp(1, visible_entries),
                        LegendGrid::Rows(value) => {
                            visible_entries.div_ceil(usize::from(*value).max(1))
                        }
                        LegendGrid::Auto => visible_entries.div_ceil(rendered_rows),
                    };
                    ui.horizontal(|ui| {
                        ui.label(self.language.text(Text::LegendGrid));
                        if ui
                            .selectable_label(
                                matches!(grid, LegendGrid::Auto),
                                self.language.text(Text::Auto),
                            )
                            .clicked()
                        {
                            *grid = LegendGrid::Auto;
                            changed = true;
                        }
                    });
                    ui.horizontal(|ui| {
                        let row_edit = ui.add_sized(
                            [132.0, control_height],
                            egui::DragValue::new(&mut rows)
                                .range(1..=max_grid)
                                .prefix(format!("{}: ", self.language.text(Text::LegendRows))),
                        );
                        if row_edit.changed() {
                            *grid = LegendGrid::Rows(rows as u8);
                            changed = true;
                        }
                        let column_edit = ui.add_sized(
                            [132.0, control_height],
                            egui::DragValue::new(&mut columns)
                                .range(1..=max_grid)
                                .prefix(format!("{}: ", self.language.text(Text::LegendColumns))),
                        );
                        if column_edit.changed() {
                            *grid = LegendGrid::Columns(columns as u8);
                            changed = true;
                        }
                    });
                    let output_width = f64::from(self.resolved.display.width) * 25.4 / 72.0;
                    let output_height = f64::from(self.resolved.display.height) * 25.4 / 72.0;
                    if *placement != LegendPlacement::Inside {
                        ui.weak(format!(
                            "{}: {output_width:.2} × {output_height:.2} mm",
                            self.language.text(Text::OutputCanvasSize)
                        ));
                    }
                    ui.label(self.language.text(Text::LegendEntries));
                    let mut move_entry = None;
                    let entry_count = entries.len();
                    for (index, entry) in entries.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            changed |= ui.checkbox(&mut entry.visible, "").changed();
                            let mut order = index + 1;
                            if ui
                                .add(
                                    egui::DragValue::new(&mut order)
                                        .range(1..=entry_count)
                                        .speed(0.1)
                                        .prefix(format!("{}: ", self.language.text(Text::Order))),
                                )
                                .changed()
                            {
                                move_entry = Some((index, order - 1));
                            }
                            let name = self
                                .document
                                .semantic_label_nodes(&entry.label_id)
                                .map(label_input::display_text)
                                .unwrap_or_else(|| entry.artist_id.clone());
                            ui.label(name);
                        });
                        if compact {
                            egui::CollapsingHeader::new(format!(
                                "{} {}",
                                self.language.text(Text::EditEntry),
                                index + 1
                            ))
                            .id_salt(("legend-entry-label", &entry.label_id))
                            .default_open(false)
                            .show(ui, |ui| {
                                self.quick_semantic_label_editor(ui, &entry.label_id);
                            });
                        }
                        if !compact {
                            semantic_labels.push(entry.label_id.clone());
                        }
                    }
                    if let Some((from, to)) = move_entry {
                        move_legend_entry(entries, from, to);
                        changed = true;
                    }
                }
            }
        };
        if compact {
            draw_properties(ui);
        } else {
            ui.separator();
            egui::CollapsingHeader::new(properties_title)
                .default_open(true)
                .show(ui, draw_properties);
        }
        if delete_drawing_object {
            if self.execute_document_edit(
                EditCommand::DeleteDrawingObject {
                    artist_id: artist_id.to_owned(),
                },
                "删除绘图对象",
            ) {
                self.context_editor_targets
                    .retain(|target| target.project_id != artist_id);
                if self
                    .context_editor_focus_target
                    .as_ref()
                    .is_some_and(|target| target.project_id == artist_id)
                {
                    self.context_editor_focus_target = None;
                }
            }
            self.selected_canvas_node = None;
            self.selected_canvas_role = None;
            return;
        }
        if let Some(nodes) = measurement_label_change {
            self.execute_document_edit(
                EditCommand::SetMeasurementArrowLabel {
                    artist_id: artist_id.to_owned(),
                    nodes,
                },
                "编辑测量标签",
            );
            return;
        }
        if let Some(style) = series_style_change {
            if self.execute_document_edit(
                EditCommand::SetSeriesStyle {
                    artist_id: artist_id.to_owned(),
                    style,
                },
                "更改图形类型",
            ) {
                ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
            }
            return;
        }
        if let Some(axes) = axis_binding_change {
            self.execute_document_edit(
                EditCommand::SetSeriesAxisBinding {
                    artist_id: artist_id.to_owned(),
                    axes,
                },
                "更改曲线坐标轴",
            );
        }
        let refresh_reference_autoscale = changed
            && (reference_autoscale_before
                || matches!(
                    &record.properties,
                    ArtistProperties::ReferenceLine {
                        include_in_autoscale: true,
                        ..
                    }
                ));
        let edit_group = (continuous_change || refresh_reference_autoscale)
            .then_some(EditGroup::ArtistProperties);
        if changed
            && self.execute_document_edit_with_group(
                EditCommand::SetArtistRecord(record),
                self.language.text(Text::ArtistProperties),
                edit_group,
            )
        {
            if refresh_reference_autoscale {
                self.execute_document_edit_with_group(
                    EditCommand::RefreshAutoscale,
                    "更新自动范围",
                    edit_group,
                );
            }
            ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
        }
        if finish_coalescing {
            self.edit_history.finish_coalescing();
        }
        if let Some(size_pt) = apply_marker_size_to_all {
            self.execute_document_edit(
                EditCommand::SetAllMarkerSizes { size_pt },
                self.language.text(Text::ApplyMarkerToAll),
            );
        }
        if let Some(interval) = apply_marker_interval_to_all {
            self.execute_document_edit(
                EditCommand::SetAllMarkerIntervals { interval },
                self.language.text(Text::ApplyMarkerToAll),
            );
        }
        if let Some(filled) = apply_marker_fill_to_all {
            self.execute_document_edit(
                EditCommand::SetAllMarkerFilled { filled },
                self.language.text(Text::ApplyMarkerToAll),
            );
        }
        for label_id in semantic_labels {
            self.semantic_label_editor(ui, &label_id);
        }
    }

    pub(crate) fn quick_semantic_label_editor(&mut self, ui: &mut egui::Ui, label_id: &str) {
        self.semantic_label_editor(ui, label_id);
    }

    pub(crate) fn semantic_label_editor(&mut self, ui: &mut egui::Ui, label_id: &str) {
        let Some(nodes) = self.document.semantic_label_nodes(label_id) else {
            return;
        };
        let initial = label_input::format(nodes)
            .or_else(|| {
                self.label_inputs
                    .get(label_id)
                    .filter(|state| state.source_nodes == nodes)
                    .map(|state| state.text.clone())
            })
            .unwrap_or_else(|| label_input::display_text(nodes));
        self.label_input_editor(
            ui,
            label_id,
            nodes.to_vec(),
            initial,
            LabelInputTarget::Semantic(label_id),
        );
    }

    pub(crate) fn label_input_editor(
        &mut self,
        ui: &mut egui::Ui,
        key: &str,
        current: Vec<LabelNode>,
        initial: String,
        target: LabelInputTarget<'_>,
    ) {
        let annotation_artist = match target {
            LabelInputTarget::Semantic(label_id) => self.annotation_artist_for_label(label_id),
            LabelInputTarget::Axis(_) => None,
        };
        let measurement_artist = match target {
            LabelInputTarget::Semantic(label_id) => self.measurement_artist_for_label(label_id),
            LabelInputTarget::Axis(_) => None,
        };
        let allows_hard_line_breaks = annotation_artist.is_some() || measurement_artist.is_some();
        let state = self
            .label_inputs
            .entry(key.to_owned())
            .or_insert_with(|| LabelInputState {
                source_nodes: current.clone(),
                text: initial.clone(),
            });
        if state.source_nodes != current {
            state.source_nodes = current.clone();
            state.text = initial;
        }
        let mut text = state.text.clone();
        let before = text.clone();
        let mut output = egui::ScrollArea::vertical()
            .max_height(150.0)
            .show(ui, |ui| {
                egui::TextEdit::multiline(&mut text)
                    .id_salt(("label-input", key))
                    .desired_width(ui.available_width())
                    .desired_rows(2)
                    .hint_text("例如：$T$ <= 300 K；H_{DL} (mT)")
                    .show(ui)
            })
            .inner;
        text_input::auto_pair_brackets(
            ui,
            &before,
            &mut text,
            &mut output,
            text_input::BracketMode::FigureText,
        );
        let mut changed = output.response.changed();
        let mut picked: Option<(String, bool)> = None;
        ui.horizontal(|ui| {
            let button_width = 64.0;
            let total_width = button_width * 4.0 + ui.spacing().item_spacing.x * 3.0;
            ui.add_space(((ui.available_width() - total_width) * 0.5).max(0.0));
            for (caption, snippet, inside, help) in [
                ("斜体", "$$", true, "用 $…$ 标记斜体变量"),
                ("下标", "_{}", true, "插入下标 _{…}"),
                ("上标", "^{}", true, "插入上标 ^{…}"),
                ("正体", "\\mathrm{}", true, "在数学区域内强制使用正体"),
            ] {
                if studio_label_format_button(ui, caption)
                    .on_hover_text(help)
                    .clicked()
                {
                    picked = Some((snippet.to_owned(), inside));
                }
            }
        });
        ui.horizontal(|ui| {
            let symbol_width = 176.0;
            let help_width = 96.0;
            let total_width = symbol_width + help_width + ui.spacing().item_spacing.x;
            ui.add_space(((ui.available_width() - total_width) * 0.5).max(0.0));
            let symbol_button = egui::Button::new(
                egui::RichText::new("数学符号与希腊字母")
                    .size(16.0)
                    .line_height(Some(20.0)),
            )
            .min_size(egui::vec2(symbol_width, 40.0));
            egui::containers::menu::MenuButton::from_button(symbol_button).ui(ui, |ui| {
                ui.set_min_width(370.0);
                egui::ScrollArea::vertical()
                    .max_height(390.0)
                    .show(ui, |ui| {
                        for (heading, range) in [
                            ("希腊字母 · 小写与变体", 0..label_input::GREEK_LOWER_END),
                            (
                                "希腊字母 · 大写",
                                label_input::GREEK_LOWER_END..label_input::GREEK_UPPER_END,
                            ),
                            (
                                "数学运算与关系",
                                label_input::GREEK_UPPER_END..label_input::MATH_RELATIONS_END,
                            ),
                            (
                                "其他科学符号",
                                label_input::MATH_RELATIONS_END..label_input::SYMBOLS.len(),
                            ),
                        ] {
                            ui.label(heading);
                            let greek = range.start < label_input::GREEK_UPPER_END;
                            egui::Grid::new(("label-symbols", key, heading))
                                .num_columns(8)
                                .show(ui, |ui| {
                                    for (index, (symbol, command)) in
                                        label_input::SYMBOLS[range].iter().enumerate()
                                    {
                                        if studio_symbol_button(ui, symbol, greek)
                                            .on_hover_text(*command)
                                            .clicked()
                                        {
                                            picked = Some(((*symbol).to_owned(), false));
                                            ui.close();
                                        }
                                        if index % 8 == 7 {
                                            ui.end_row();
                                        }
                                    }
                                });
                            ui.add_space(8.0);
                        }
                    });
            });
            let help_button = egui::Button::new(
                egui::RichText::new("输入帮助")
                    .size(16.0)
                    .line_height(Some(20.0)),
            )
            .min_size(egui::vec2(help_width, 40.0));
            egui::containers::menu::MenuButton::from_button(help_button).ui(ui, |ui| {
                ui.label("普通区域一律正体；拉丁字母和希腊字母可直接输入");
                ui.label("斜体变量写在一对 $...$ 内，例如 $R$、$α$、$R_x^y$");
                ui.label("比较符号：<=、>=、!=，或直接输入 ≤、≥、≠");
                ui.label("正体上下标：R_x^y；组合上下标会排在同一基字符旁");
                ui.label("数学上下标：$R_x^y$ 中 R、x、y 均为斜体变量");
                ui.label("数学区内强制正体：$R_{\\mathrm{eff}}$ 或使用“正体”按钮");
                ui.label("符号可点选、粘贴，或输入 \\sigma、\\sum、<=");
                ui.label("星号必须转义：输入 \\* 可显示 *");
                ui.label("“斜体”按钮会给所选内容加上一对 $；普通文字不需要 $。");
            });
        });
        if let Some((snippet, inside)) = picked {
            insert_label_snippet(&mut text, &mut output, &snippet, inside, ui.ctx());
            changed = true;
        }
        changed |= normalize_label_editor_line_breaks(&mut text, allows_hard_line_breaks);
        self.label_inputs.get_mut(key).unwrap().text = text.clone();
        let empty_annotation = text.trim().is_empty() && annotation_artist.is_some();
        let empty_measurement = text.trim().is_empty() && measurement_artist.is_some();
        if empty_annotation {
            ui.weak("内容已清空；关闭窗口后将删除此标注");
        } else if empty_measurement {
            ui.weak("内容暂未应用；请继续输入，或使用“移除标签”明确删除");
        } else {
            match label_input::parse(&text).and_then(|nodes| {
                label_input::validate_glyphs(&nodes)?;
                Ok(nodes)
            }) {
                Ok(nodes) => {
                    ui.weak("预览（画布为准）：");
                    ui.label(label_preview_job(&nodes))
                        .on_hover_text(label_input::display_text(&nodes));
                    if changed && nodes != current {
                        let command = match target {
                            LabelInputTarget::Axis(identity) => {
                                EditCommand::SetAxisLabelByIdentity {
                                    identity,
                                    nodes: nodes.clone(),
                                }
                            }
                            LabelInputTarget::Semantic(label_id) => EditCommand::SetSemanticLabel {
                                label_id: label_id.to_owned(),
                                nodes: nodes.clone(),
                            },
                        };
                        if self.execute_document_edit_with_group(
                            command,
                            self.language.text(Text::ApplyLabel),
                            Some(EditGroup::SemanticLabel),
                        ) {
                            self.label_inputs.get_mut(key).unwrap().source_nodes = nodes;
                        }
                    }
                }
                Err(error) => {
                    ui.colored_label(egui::Color32::from_rgb(180, 75, 50), error);
                }
            }
        }
        if output.response.lost_focus() {
            self.edit_history.finish_coalescing();
        }
    }
}
