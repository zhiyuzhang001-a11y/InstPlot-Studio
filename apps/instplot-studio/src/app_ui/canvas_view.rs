use super::*;

impl StudioApp {
    pub(crate) fn show_canvas(&mut self, ui: &mut egui::Ui, context: &egui::Context) {
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(studio_surface(context.theme() == egui::Theme::Dark)))
            .show(ui, |ui| {
                let viewport = ui.available_size();
                if self.first_frame {
                    self.fit_canvas(viewport);
                }
                let old_zoom = self.canvas_zoom;
                let drawing_tool = self.drawing_tool;
                let output = egui::ScrollArea::both()
                    .id_salt("figure-canvas-scroll")
                    .scroll_offset(self.canvas_scroll)
                    .scroll_source(egui::scroll_area::ScrollSource::SCROLL_BAR)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let formal_figure_size =
                            egui::vec2(self.resolved.display.width, self.resolved.display.height)
                                * old_zoom;
                        let empty_secondary = self.document.empty_active_secondary_axis();
                        let placeholder_color = egui::Color32::from_gray(130);
                        let placeholder_font = egui::FontId::proportional(
                            secondary_axis_placeholder_font_size(old_zoom),
                        );
                        let placeholder_galley = empty_secondary.map(|identity| {
                            let text = match identity {
                                AxisIdentity::X2 => "X2 label",
                                AxisIdentity::Y2 => "Y2 label",
                                AxisIdentity::X1 | AxisIdentity::Y1 => "",
                            };
                            ui.painter().layout_no_wrap(
                                text.to_owned(),
                                placeholder_font.clone(),
                                placeholder_color,
                            )
                        });
                        let configured_placeholder_pad_pt = empty_secondary
                            .and_then(|identity| self.document.axis_record_by_identity(identity))
                            .map_or(SECONDARY_AXIS_PLACEHOLDER_MIN_PAD_PT, |axis| {
                                axis.appearance.label_tick_pad_pt as f32
                            });
                        let placeholder_pad_pt = empty_secondary
                            .zip(placeholder_galley.as_ref())
                            .map_or(configured_placeholder_pad_pt, |(identity, galley)| {
                                secondary_axis_placeholder_pad(
                                    identity,
                                    self.resolved.layout.result.axes,
                                    self.resolved.layout.result.legend,
                                    galley.size(),
                                    old_zoom,
                                    configured_placeholder_pad_pt,
                                )
                            });
                        let placeholder_gutter = empty_secondary
                            .zip(placeholder_galley.as_ref())
                            .map_or_else(
                                SecondaryAxisPlaceholderGutter::default,
                                |(identity, galley)| {
                                    secondary_axis_placeholder_gutter(
                                        identity,
                                        self.resolved.layout.result.axes,
                                        formal_figure_size,
                                        galley.size(),
                                        old_zoom,
                                        placeholder_pad_pt,
                                    )
                                },
                            );
                        let preview_size = egui::vec2(
                            formal_figure_size.x + placeholder_gutter.right,
                            formal_figure_size.y + placeholder_gutter.top,
                        );
                        let content_size = egui::vec2(
                            (preview_size.x + 64.0).max(viewport.x),
                            (preview_size.y + 84.0).max(viewport.y),
                        );
                        let (content_rect, _) =
                            ui.allocate_exact_size(content_size, egui::Sense::hover());
                        let preview_origin = content_rect.min
                            + egui::vec2(
                                ((content_size.x - preview_size.x) * 0.5).max(32.0),
                                ((content_size.y - preview_size.y) * 0.5).max(32.0),
                            );
                        let page_origin = preview_origin + egui::vec2(0.0, placeholder_gutter.top);
                        let origin = page_origin;
                        let axes = self.resolved.layout.result.axes;
                        let figure_center_in_content = origin
                            + egui::vec2(
                                (axes.x + axes.width / 2.0) as f32,
                                (axes.y + axes.height / 2.0) as f32,
                            ) * old_zoom
                            - content_rect.min;
                        let preview_rect = egui::Rect::from_min_size(preview_origin, preview_size);
                        let response = ui.interact(
                            preview_rect.expand(14.0),
                            ui.id().with("figure-canvas"),
                            egui::Sense::click_and_drag(),
                        );
                        ui.painter().rect_filled(
                            preview_rect.expand(6.0).translate(egui::vec2(0.0, 4.0)),
                            4.0,
                            egui::Color32::from_black_alpha(
                                if context.theme() == egui::Theme::Dark {
                                    38
                                } else {
                                    18
                                },
                            ),
                        );
                        ui.painter()
                            .rect_filled(preview_rect, 0.0, egui::Color32::WHITE);
                        ui.painter().rect_stroke(
                            preview_rect,
                            0.0,
                            egui::Stroke::new(1.0, egui::Color32::GRAY),
                            egui::StrokeKind::Outside,
                        );
                        self.preview.paint(
                            ui.painter(),
                            &self.resolved.display,
                            origin,
                            old_zoom,
                            context.pixels_per_point(),
                        );
                        if let Some((identity, galley)) = empty_secondary.zip(placeholder_galley) {
                            let pad = SECONDARY_AXIS_PLACEHOLDER_MIN_PAD_PT.max(placeholder_pad_pt)
                                * old_zoom;
                            match identity {
                                AxisIdentity::X2 => {
                                    let position = origin
                                        + egui::vec2(
                                            (axes.x + axes.width / 2.0) as f32,
                                            axes.y as f32,
                                        ) * old_zoom
                                        + egui::vec2(0.0, -pad);
                                    ui.painter().galley(
                                        position
                                            - egui::vec2(galley.size().x / 2.0, galley.size().y),
                                        galley,
                                        placeholder_color,
                                    );
                                }
                                AxisIdentity::Y2 => {
                                    let center = origin
                                        + egui::vec2(
                                            (axes.x + axes.width) as f32,
                                            (axes.y + axes.height / 2.0) as f32,
                                        ) * old_zoom
                                        + egui::vec2(pad + galley.size().y / 2.0, 0.0);
                                    let position = center - galley.size() / 2.0;
                                    ui.painter().add(
                                        egui::epaint::TextShape::new(
                                            position,
                                            galley,
                                            placeholder_color,
                                        )
                                        .with_angle_and_anchor(
                                            -std::f32::consts::FRAC_PI_2,
                                            egui::Align2::CENTER_CENTER,
                                        ),
                                    );
                                }
                                AxisIdentity::X1 | AxisIdentity::Y1 => {}
                            }
                        }
                        if let Some(draft) = self.tool_draft
                            && let Some((start, end)) = data_point_to_local(
                                &self.document,
                                self.resolved.layout.result.axes,
                                draft.axes,
                                draft.start,
                            )
                            .zip(data_point_to_local(
                                &self.document,
                                self.resolved.layout.result.axes,
                                draft.axes,
                                draft.end,
                            ))
                        {
                            let start =
                                origin + egui::vec2(start.0 as f32, start.1 as f32) * old_zoom;
                            let end = origin + egui::vec2(end.0 as f32, end.1 as f32) * old_zoom;
                            let stroke = egui::Stroke::new(1.5, egui::Color32::BLACK);
                            ui.painter().line_segment([start, end], stroke);
                            ui.painter().circle_filled(start, 3.0, egui::Color32::BLACK);
                            ui.painter().circle_filled(end, 3.0, egui::Color32::BLACK);
                        }
                        if let Some(draft) = self.reference_draft.as_ref() {
                            let x_identity = match draft.axes.x {
                                XAxisSlot::X1 => AxisIdentity::X1,
                                XAxisSlot::X2 => AxisIdentity::X2,
                            };
                            let y_identity = match draft.axes.y {
                                YAxisSlot::Y1 => AxisIdentity::Y1,
                                YAxisSlot::Y2 => AxisIdentity::Y2,
                            };
                            let midpoint = |axis: &instplot_studio::AxisRecord| {
                                if axis.scale == AxisScale::Log10
                                    && axis.minimum > 0.0
                                    && axis.maximum > 0.0
                                {
                                    (axis.minimum * axis.maximum).sqrt()
                                } else {
                                    (axis.minimum + axis.maximum) / 2.0
                                }
                            };
                            if let Some((x_axis, y_axis)) = self
                                .document
                                .axis_record_by_identity(x_identity)
                                .zip(self.document.axis_record_by_identity(y_identity))
                            {
                                let point = match draft.orientation {
                                    ReferenceOrientation::Vertical => {
                                        (draft.value, midpoint(&y_axis))
                                    }
                                    ReferenceOrientation::Horizontal => {
                                        (midpoint(&x_axis), draft.value)
                                    }
                                };
                                if let Some(mapped) = data_point_to_local(
                                    &self.document,
                                    self.resolved.layout.result.axes,
                                    draft.axes,
                                    point,
                                ) {
                                    let axes = self.resolved.layout.result.axes;
                                    let (start, end) = match draft.orientation {
                                        ReferenceOrientation::Vertical => {
                                            ((mapped.0, axes.y), (mapped.0, axes.bottom()))
                                        }
                                        ReferenceOrientation::Horizontal => {
                                            ((axes.x, mapped.1), (axes.right(), mapped.1))
                                        }
                                    };
                                    let color = self
                                        .document
                                        .palette_colors()
                                        .iter()
                                        .find(|color| color.id == draft.stroke.color_id)
                                        .map(|color| {
                                            let [r, g, b, a] = color.rgba;
                                            egui::Color32::from_rgba_unmultiplied(r, g, b, a)
                                        })
                                        .unwrap_or(egui::Color32::BLACK);
                                    let points = [
                                        origin
                                            + egui::vec2(start.0 as f32, start.1 as f32) * old_zoom,
                                        origin + egui::vec2(end.0 as f32, end.1 as f32) * old_zoom,
                                    ];
                                    let stroke = egui::Stroke::new(
                                        (draft.stroke.width_pt as f32 * old_zoom).max(1.0),
                                        color,
                                    );
                                    if draft.stroke.dash_pt.is_empty() {
                                        ui.painter().line_segment(points, stroke);
                                    } else {
                                        let dashes = draft
                                            .stroke
                                            .dash_pt
                                            .iter()
                                            .step_by(2)
                                            .map(|length| *length as f32 * old_zoom)
                                            .collect::<Vec<_>>();
                                        let gaps = draft
                                            .stroke
                                            .dash_pt
                                            .iter()
                                            .skip(1)
                                            .step_by(2)
                                            .map(|length| *length as f32 * old_zoom)
                                            .collect::<Vec<_>>();
                                        for shape in egui::Shape::dashed_line_with_offset(
                                            &points, stroke, &dashes, &gaps, 0.0,
                                        ) {
                                            ui.painter().add(shape);
                                        }
                                    }
                                }
                            }
                        }

                        if let Some(selected) = self.selected_canvas_node.as_deref() {
                            let dragging_legend = self
                                .active_artist_drag
                                .as_ref()
                                .filter(|drag| drag.id == selected && drag.legend.is_some());
                            let path_highlighted = dragging_legend.is_none() && {
                                let mut painted = false;
                                for linked in self.document.linked_series_ids(selected) {
                                    painted |= paint_object_path_overlay(
                                        ui.painter(),
                                        &self.resolved,
                                        &linked,
                                        self.selected_canvas_role,
                                        origin,
                                        old_zoom,
                                        true,
                                    );
                                }
                                painted
                            };
                            let selection_bounds =
                                dragging_legend.map(|drag| drag.preview_bounds).or_else(|| {
                                    selected_hit_bounds_for_role(
                                        &self.resolved,
                                        selected,
                                        self.selected_canvas_role,
                                    )
                                });
                            if !path_highlighted && let Some(bounds) = selection_bounds {
                                let overlay = egui::Rect::from_min_max(
                                    origin
                                        + egui::vec2(bounds.0 as f32, bounds.1 as f32) * old_zoom,
                                    origin
                                        + egui::vec2(bounds.2 as f32, bounds.3 as f32) * old_zoom,
                                )
                                .expand(4.0);
                                let accent = egui::Color32::from_rgb(56, 145, 225);
                                ui.painter().rect_filled(
                                    overlay,
                                    4.0,
                                    egui::Color32::from_rgba_unmultiplied(56, 145, 225, 10),
                                );
                                ui.painter().rect_stroke(
                                    overlay.expand(2.0),
                                    5.0,
                                    egui::Stroke::new(
                                        4.0,
                                        egui::Color32::from_rgba_unmultiplied(56, 145, 225, 45),
                                    ),
                                    egui::StrokeKind::Outside,
                                );
                                ui.painter().rect_stroke(
                                    overlay,
                                    4.0,
                                    egui::Stroke::new(1.8, accent),
                                    egui::StrokeKind::Outside,
                                );
                                if matches!(
                                    self.selected_canvas_role,
                                    Some(SelectableRole::Legend | SelectableRole::Annotation)
                                ) {
                                    for corner in [
                                        overlay.left_top(),
                                        overlay.right_top(),
                                        overlay.left_bottom(),
                                        overlay.right_bottom(),
                                    ] {
                                        ui.painter().circle_filled(corner, 3.0, accent);
                                    }
                                }
                                if self.selected_canvas_role == Some(SelectableRole::Legend) {
                                    for center in [overlay.right_center(), overlay.center_bottom()]
                                    {
                                        ui.painter().rect_filled(
                                            egui::Rect::from_center_size(
                                                center,
                                                egui::vec2(10.0, 10.0),
                                            ),
                                            2.0,
                                            egui::Color32::WHITE,
                                        );
                                        ui.painter().rect_stroke(
                                            egui::Rect::from_center_size(
                                                center,
                                                egui::vec2(10.0, 10.0),
                                            ),
                                            2.0,
                                            egui::Stroke::new(1.5, accent),
                                            egui::StrokeKind::Outside,
                                        );
                                    }
                                    if let Some(drag) = dragging_legend {
                                        let target = drag
                                            .candidate_placement
                                            .unwrap_or(LegendPlacement::Inside);
                                        let mode = match drag.mode {
                                            ArtistDragMode::Move => match target {
                                                LegendPlacement::Above => {
                                                    self.language.text(Text::LegendAbove)
                                                }
                                                LegendPlacement::Right => {
                                                    self.language.text(Text::LegendRight)
                                                }
                                                _ => self.language.text(Text::LegendInside),
                                            }
                                            .to_owned(),
                                            ArtistDragMode::ResizeColumns
                                            | ArtistDragMode::ResizeRows => {
                                                let context =
                                                    drag.legend.expect("legend drag context");
                                                let columns = ((bounds.2 - bounds.0)
                                                    / context.cell_width)
                                                    .round()
                                                    .max(1.0)
                                                    as usize;
                                                format!(
                                                    "{columns} × {}",
                                                    context.entries.div_ceil(columns)
                                                )
                                            }
                                        };
                                        ui.painter().text(
                                            overlay.left_top() + egui::vec2(6.0, -7.0),
                                            egui::Align2::LEFT_BOTTOM,
                                            mode,
                                            egui::FontId::proportional(12.0),
                                            accent,
                                        );
                                    }
                                }
                            }
                            if self.active_artist_drag.as_ref().is_some_and(|drag| {
                                drag.id == selected && drag.role == SelectableRole::ReferenceLine
                            }) && let Some(record) = self.document.artist_record(selected)
                                && let ArtistProperties::ReferenceLine { value, .. } =
                                    record.properties
                                && let Some(bounds) = selected_hit_bounds_for_role(
                                    &self.resolved,
                                    selected,
                                    Some(SelectableRole::ReferenceLine),
                                )
                            {
                                let anchor = origin
                                    + egui::vec2(bounds.0 as f32, bounds.1 as f32) * old_zoom
                                    + egui::vec2(7.0, -7.0);
                                ui.painter().text(
                                    anchor,
                                    egui::Align2::LEFT_BOTTOM,
                                    format!("{value:.6}"),
                                    egui::FontId::monospace(12.0),
                                    egui::Color32::from_rgb(56, 145, 225),
                                );
                            }
                        }

                        let quick_drag =
                            context.input(|input| same_frame_primary_drag(&input.raw.events));
                        let press_origin = quick_drag.map(|(start, _)| start).or_else(|| {
                            context.input(|input| {
                                input.pointer.press_origin().or_else(|| {
                                    input.raw.events.iter().find_map(|event| {
                                        if let egui::Event::PointerButton {
                                            pos,
                                            button: egui::PointerButton::Primary,
                                            pressed: true,
                                            ..
                                        } = event
                                        {
                                            Some(*pos)
                                        } else {
                                            None
                                        }
                                    })
                                })
                            })
                        });
                        let pointer = quick_drag
                            .map(|(_, end)| end)
                            .or_else(|| response.interact_pointer_pos())
                            .or_else(|| context.input(|input| input.pointer.interact_pos()))
                            .or_else(|| context.input(|input| input.pointer.latest_pos()));
                        let hovered = pointer
                            .and_then(|point| hit_project(&self.resolved, point, origin, old_zoom));
                        let pressed = press_origin
                            .and_then(|point| hit_project(&self.resolved, point, origin, old_zoom));
                        let reference_object_interaction = reference_tool_allows_object_interaction(
                            drawing_tool,
                            hovered.as_ref().map(|hit| hit.role),
                            pressed.as_ref().map(|hit| hit.role),
                            self.active_artist_drag.as_ref().map(|drag| drag.role),
                        );
                        if let Some(hovered) = hovered.as_ref() {
                            ui.output_mut(|output| {
                                output.cursor_icon = egui::CursorIcon::PointingHand
                            });
                            if (self.selected_canvas_node.as_deref()
                                != Some(hovered.project_id.as_str())
                                || self.selected_canvas_role != Some(hovered.role))
                                && !paint_object_path_overlay(
                                    ui.painter(),
                                    &self.resolved,
                                    &hovered.project_id,
                                    Some(hovered.role),
                                    origin,
                                    old_zoom,
                                    false,
                                )
                                && let Some(bounds) = selected_hit_bounds_for_role(
                                    &self.resolved,
                                    &hovered.project_id,
                                    Some(hovered.role),
                                )
                            {
                                let overlay = egui::Rect::from_min_max(
                                    origin
                                        + egui::vec2(bounds.0 as f32, bounds.1 as f32) * old_zoom,
                                    origin
                                        + egui::vec2(bounds.2 as f32, bounds.3 as f32) * old_zoom,
                                )
                                .expand(3.0);
                                ui.painter().rect_stroke(
                                    overlay,
                                    3.0,
                                    egui::Stroke::new(1.5, egui::Color32::from_rgb(76, 157, 255)),
                                    egui::StrokeKind::Outside,
                                );
                            }
                        }
                        let clicked =
                            (quick_drag.is_none() && response.clicked()).then(|| hovered.clone());
                        let foreground_hovered = hovered
                            .as_ref()
                            .filter(|hit| hit.role != SelectableRole::Axes);
                        let edit_requested = quick_drag.is_none()
                            && response.double_clicked()
                            && foreground_hovered.is_some();
                        let mut drag_event = None;
                        let active_handle =
                            if self.selected_canvas_role == Some(SelectableRole::Legend) {
                                self.selected_canvas_node
                                    .as_deref()
                                    .and_then(|selected| {
                                        selected_hit_bounds_for_role(
                                            &self.resolved,
                                            selected,
                                            Some(SelectableRole::Legend),
                                        )
                                    })
                                    .and_then(|bounds| {
                                        pointer.and_then(|point| {
                                            legend_resize_handle_at(point, bounds, origin, old_zoom)
                                        })
                                    })
                            } else {
                                None
                            };
                        if let Some(handle) = active_handle {
                            ui.output_mut(|output| {
                                output.cursor_icon = match handle {
                                    ArtistDragMode::ResizeColumns => {
                                        egui::CursorIcon::ResizeHorizontal
                                    }
                                    ArtistDragMode::ResizeRows => egui::CursorIcon::ResizeVertical,
                                    ArtistDragMode::Move => egui::CursorIcon::Grab,
                                }
                            });
                        }
                        if active_handle.is_none()
                            && matches!(
                                hovered.as_ref().map(|hit| hit.role),
                                Some(
                                    SelectableRole::Legend
                                        | SelectableRole::Annotation
                                        | SelectableRole::ReferenceLine
                                        | SelectableRole::MeasurementArrow
                                        | SelectableRole::MeasurementArrowStart
                                        | SelectableRole::MeasurementArrowEnd
                                        | SelectableRole::MeasurementArrowLabel,
                                )
                            )
                        {
                            ui.output_mut(|output| output.cursor_icon = egui::CursorIcon::Grab);
                        }
                        if (drawing_tool == DrawingTool::Select || reference_object_interaction)
                            && (response.drag_started()
                                || (response.drag_stopped() && self.active_artist_drag.is_none())
                                || (quick_drag.is_some() && self.active_artist_drag.is_none()))
                        {
                            let handle_hit = press_origin.and_then(|point| {
                                self.selected_canvas_node.as_deref().and_then(|selected| {
                                    selected_hit_bounds_for_role(
                                        &self.resolved,
                                        selected,
                                        Some(SelectableRole::Legend),
                                    )
                                    .and_then(|bounds| {
                                        legend_resize_handle_at(point, bounds, origin, old_zoom)
                                            .map(|mode| {
                                                (
                                                    CanvasHit {
                                                        project_id: selected.to_owned(),
                                                        role: SelectableRole::Legend,
                                                        data_index: None,
                                                    },
                                                    mode,
                                                )
                                            })
                                    })
                                })
                            });
                            let pressed_hit = handle_hit.or_else(|| {
                                press_origin
                                    .and_then(|point| {
                                        hit_project(&self.resolved, point, origin, old_zoom)
                                    })
                                    .map(|hit| (hit, ArtistDragMode::Move))
                            });
                            if let Some((hit, mode)) = pressed_hit.filter(|hit| {
                                matches!(
                                    hit.0.role,
                                    SelectableRole::Legend
                                        | SelectableRole::Annotation
                                        | SelectableRole::AnnotationConnector
                                        | SelectableRole::ReferenceLine
                                        | SelectableRole::MeasurementArrow
                                        | SelectableRole::MeasurementArrowStart
                                        | SelectableRole::MeasurementArrowEnd
                                        | SelectableRole::MeasurementArrowLabel
                                )
                            }) && let Some(bounds) = selected_hit_bounds_for_role(
                                &self.resolved,
                                &hit.project_id,
                                Some(hit.role),
                            ) {
                                drag_event = Some(CanvasDragEvent {
                                    id: hit.project_id,
                                    role: hit.role,
                                    bounds,
                                    delta: response.drag_delta(),
                                    pointer: pointer.map(|point| {
                                        (
                                            f64::from((point.x - origin.x) / old_zoom),
                                            f64::from((point.y - origin.y) / old_zoom),
                                        )
                                    }),
                                    press_pointer: press_origin.map(|point| {
                                        (
                                            f64::from((point.x - origin.x) / old_zoom),
                                            f64::from((point.y - origin.y) / old_zoom),
                                        )
                                    }),
                                    mode,
                                    started: true,
                                    stopped: response.drag_stopped() || quick_drag.is_some(),
                                    data_index: hit.data_index,
                                    shift: context.input(|input| input.modifiers.shift),
                                });
                            }
                        } else if (response.dragged() || response.drag_stopped())
                            && let Some(drag) = self.active_artist_drag.as_ref()
                        {
                            drag_event = Some(CanvasDragEvent {
                                id: drag.id.clone(),
                                role: drag.role,
                                bounds: drag.bounds,
                                delta: response.drag_delta(),
                                pointer: pointer.map(|point| {
                                    (
                                        f64::from((point.x - origin.x) / old_zoom),
                                        f64::from((point.y - origin.y) / old_zoom),
                                    )
                                }),
                                press_pointer: None,
                                mode: drag.mode,
                                started: false,
                                stopped: response.drag_stopped(),
                                data_index: drag.connector_index,
                                shift: context.input(|input| input.modifiers.shift),
                            });
                        }
                        let axes = self.resolved.layout.result.axes;
                        let axes_rect = egui::Rect::from_min_max(
                            origin + egui::vec2(axes.x as f32, axes.y as f32) * old_zoom,
                            origin
                                + egui::vec2(axes.right() as f32, axes.bottom() as f32) * old_zoom,
                        );
                        let axis_visibility_requested =
                            opens_axis_visibility_from_blank_double_click(
                                drawing_tool,
                                quick_drag.is_none() && response.double_clicked(),
                                hovered.as_ref().map(|hit| hit.role),
                                pointer.is_some_and(|point| axes_rect.contains(point)),
                            );
                        let (wheel, pan, next_trackpad_scroll_active) = context.input(|input| {
                            let (trackpad, next_active) = trackpad_scroll_state(
                                &input.raw.events,
                                self.trackpad_scroll_active,
                            );
                            if !response.hovered() {
                                return (0.0, egui::Vec2::ZERO, next_active);
                            }
                            let delta = input.smooth_scroll_delta();
                            let pinch = input.zoom_delta();
                            let zoom = if (pinch - 1.0).abs() > f32::EPSILON {
                                pinch.ln() / 0.002
                            } else if trackpad {
                                0.0
                            } else {
                                delta.y
                            };
                            let pan = if trackpad { delta } else { egui::Vec2::ZERO };
                            (zoom, pan, next_active)
                        });
                        let zoom_anchor = (wheel.abs() > f32::EPSILON)
                            .then(|| pointer.map(|point| (point, origin, wheel)))
                            .flatten();
                        let candidates = if edit_requested {
                            pointer.map_or_else(Vec::new, |point| {
                                hit_project_candidates(&self.resolved, point, origin, old_zoom)
                            })
                        } else {
                            Vec::new()
                        };
                        let data_coordinates = pointer.and_then(|point| {
                            active_data_coordinates_at(
                                &self.resolved,
                                &self.document,
                                point,
                                origin,
                                old_zoom,
                            )
                        });
                        let press_screen = quick_drag
                            .map(|(start, _)| start)
                            .or_else(|| context.input(|input| input.pointer.press_origin()));
                        let shift = context.input(|input| input.modifiers.shift);
                        let tool_pointer = if shift {
                            press_screen
                                .zip(pointer)
                                .map(|(start, end)| snap_pointer_to_special_angle(start, end))
                                .or(pointer)
                        } else {
                            pointer
                        };
                        let tool_event = (drawing_tool != DrawingTool::Select
                            && !reference_object_interaction
                            && (!matches!(drawing_tool, DrawingTool::Reference { .. })
                                || self.reference_draft.is_none()))
                        .then(|| CanvasToolEvent {
                            clicked: response.clicked(),
                            started: response.drag_started() || quick_drag.is_some(),
                            stopped: response.drag_stopped() || quick_drag.is_some(),
                            press: press_screen.and_then(|point| {
                                active_data_coordinates_at(
                                    &self.resolved,
                                    &self.document,
                                    point,
                                    origin,
                                    old_zoom,
                                )
                            }),
                            current: tool_pointer.and_then(|point| {
                                active_data_coordinates_at(
                                    &self.resolved,
                                    &self.document,
                                    point,
                                    origin,
                                    old_zoom,
                                )
                            }),
                            shift,
                        });
                        (
                            clicked,
                            zoom_anchor,
                            pointer,
                            axis_visibility_requested,
                            drag_event,
                            candidates,
                            edit_requested,
                            pan,
                            next_trackpad_scroll_active,
                            data_coordinates,
                            tool_event,
                            figure_center_in_content,
                        )
                    });
                self.canvas_scroll = output.state.offset;
                self.trackpad_scroll_active = output.inner.8;
                self.hover_data_coordinates = output.inner.9;
                if output.inner.7 != egui::Vec2::ZERO {
                    self.canvas_scroll =
                        (self.canvas_scroll - output.inner.7).max(egui::Vec2::ZERO);
                }
                let figure_center = output.inner.11;
                if let Some(previous) = self.last_canvas_figure_center {
                    let delta = figure_center - previous;
                    if delta != egui::Vec2::ZERO {
                        self.canvas_scroll = (self.canvas_scroll + delta).max(egui::Vec2::ZERO);
                    }
                }
                self.last_canvas_figure_center = Some(figure_center);
                if let Some(tool_event) = output.inner.10 {
                    self.handle_drawing_tool_event(tool_event);
                }
                if let Some(event) = output.inner.4
                    && (drawing_tool == DrawingTool::Select
                        || event.role == SelectableRole::ReferenceLine)
                {
                    self.handle_artist_drag(event, old_zoom);
                }
                if let Some(clicked) = output.inner.0
                    && (drawing_tool == DrawingTool::Select
                        || matches!(
                            clicked.as_ref().map(|hit| hit.role),
                            Some(SelectableRole::ReferenceLine)
                        ))
                {
                    self.selection_candidates = output.inner.5;
                    self.selected_canvas_node = clicked.as_ref().map(|hit| hit.project_id.clone());
                    self.selected_canvas_role = clicked.as_ref().map(|hit| hit.role);
                    if output.inner.6
                        && self.selection_candidates.len() <= 1
                        && let Some(clicked) = clicked.clone()
                    {
                        self.open_context_editor(clicked);
                    }
                    self.selected_series = clicked.as_ref().and_then(|hit| {
                        self.document
                            .series()
                            .into_iter()
                            .find(|series| series.id == hit.project_id)
                            .map(|series| series.id)
                    });
                    if let Some(selected) = self.selected_series.clone()
                        && let Some(series) = self
                            .document
                            .series()
                            .into_iter()
                            .find(|series| series.id == selected)
                    {
                        self.select_series_for_editing(&series);
                    }
                }
                if let Some((pointer, origin, wheel)) = output.inner.1 {
                    (self.canvas_scroll, self.canvas_zoom) =
                        zoom_about_pointer(self.canvas_scroll, pointer, origin, old_zoom, wheel);
                }
                if output.inner.3 {
                    self.request_axis_visibility_window();
                }
            });
    }
}
