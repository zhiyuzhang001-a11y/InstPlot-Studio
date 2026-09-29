use super::*;

fn resolved_axis_exponent(resolved: &ResolvedFigure, identity: AxisIdentity) -> Option<i32> {
    let result = &resolved.layout.result;
    match identity {
        AxisIdentity::X1 => Some(&result.x_axis),
        AxisIdentity::X2 => result.x2_axis.as_ref(),
        AxisIdentity::Y1 => Some(&result.y_axis),
        AxisIdentity::Y2 => result.y2_axis.as_ref(),
    }
    .map(|axis| axis.shared_exponent.unwrap_or(0))
}

fn effective_axis_input_exponent(
    record: &AxisRecord,
    resolved: &ResolvedFigure,
    identity: AxisIdentity,
) -> Option<i32> {
    match record.display_scale {
        AxisDisplayScaleRecord::None => Some(0),
        AxisDisplayScaleRecord::ManualFactor { exponent }
        | AxisDisplayScaleRecord::ManualIncorporated { exponent } => Some(exponent.get()),
        AxisDisplayScaleRecord::AutoFactor => resolved_axis_exponent(resolved, identity),
    }
}

fn quick_axis_numeric_keys(identity: AxisIdentity) -> [String; 4] {
    [
        format!("quick-axis-{identity:?}-minimum"),
        format!("quick-axis-{identity:?}-maximum"),
        format!("quick-axis-{identity:?}-major-interval"),
        format!("axis-{identity:?}-minor-interval"),
    ]
}

impl StudioApp {
    pub(crate) fn clear_axis_numeric_drafts(&mut self, identities: &[AxisIdentity]) {
        for identity in identities {
            for key in quick_axis_numeric_keys(*identity) {
                self.numeric_inputs.remove(&key);
            }
            self.numeric_inputs.remove(&format!(
                "axis-{:?}-major-interval",
                axis_dimension(*identity)
            ));
            match identity {
                AxisIdentity::X1 => {
                    self.numeric_inputs.remove("axis-range-x-min");
                    self.numeric_inputs.remove("axis-range-x-max");
                }
                AxisIdentity::Y1 => {
                    self.numeric_inputs.remove("axis-range-y-min");
                    self.numeric_inputs.remove("axis-range-y-max");
                }
                AxisIdentity::X2 | AxisIdentity::Y2 => {}
            }
        }
        self.axis_numeric_scale_sessions
            .retain(|session| !identities.contains(&session.identity));
    }

    pub(crate) fn clear_all_axis_numeric_drafts(&mut self) {
        self.clear_axis_numeric_drafts(&[
            AxisIdentity::X1,
            AxisIdentity::Y1,
            AxisIdentity::X2,
            AxisIdentity::Y2,
        ]);
    }

    pub(crate) fn figure_size_editor(&mut self, ui: &mut egui::Ui) {
        ui.label(self.language.text(Text::FigureSize));
        let (mut width, mut height) = self.document.figure_size_mm();
        let mut changed_group = None;
        let mut finish = false;
        let control_height = ui.spacing().interact_size.y;
        ui.horizontal_wrapped(|ui| {
            let response = ui.add_sized(
                [144.0, control_height],
                egui::DragValue::new(&mut width)
                    .range(20.0..=500.0)
                    .speed(0.5)
                    .prefix(format!("{}: ", self.language.text(Text::WidthMm))),
            );
            if response.changed() {
                changed_group = Some(EditGroup::FigureWidth);
            }
            finish |= response.drag_stopped() || response.lost_focus();
            let response = ui.add_sized(
                [144.0, control_height],
                egui::DragValue::new(&mut height)
                    .range(20.0..=500.0)
                    .speed(0.5)
                    .prefix(format!("{}: ", self.language.text(Text::HeightMm))),
            );
            if response.changed() {
                changed_group = Some(EditGroup::FigureHeight);
            }
            finish |= response.drag_stopped() || response.lost_focus();
        });
        let output_width = f64::from(self.resolved.display.width) * 25.4 / 72.0;
        let output_height = f64::from(self.resolved.display.height) * 25.4 / 72.0;
        if (output_width - width).abs() > 0.01 || (output_height - height).abs() > 0.01 {
            ui.weak(format!(
                "{}: {output_width:.2} × {output_height:.2} mm",
                self.language.text(Text::OutputCanvasSize)
            ));
        }
        if let Some(group) = changed_group {
            self.execute_document_edit_with_group(
                EditCommand::SetFigureSize {
                    width_mm: width,
                    height_mm: height,
                },
                self.language.text(Text::ApplySize),
                Some(group),
            );
        }
        if finish {
            self.edit_history.finish_coalescing();
        }
    }

    pub(crate) fn fit_canvas(&mut self, available: egui::Vec2) {
        let padding = egui::vec2(64.0, 96.0);
        let available = (available - padding).max(egui::vec2(1.0, 1.0));
        self.canvas_zoom = (available.x / self.resolved.display.width)
            .min(available.y / self.resolved.display.height)
            .clamp(0.1, 3.0);
        self.canvas_scroll = egui::Vec2::ZERO;
        self.last_canvas_figure_center = None;
    }

    pub(crate) fn axis_ranges_editor(&mut self, ui: &mut egui::Ui) {
        ui.label(self.language.text(Text::AxesRanges));
        let mut ranges = self.document.axis_ranges();
        let mut changed_group = None;
        let mut finish_coalescing = false;
        egui::Grid::new("axis_ranges")
            .num_columns(4)
            .show(ui, |ui| {
                ui.label(self.language.text(Text::XMin));
                let edit = deferred_f64_editor(
                    ui,
                    &mut self.numeric_inputs,
                    "axis-range-x-min",
                    &mut ranges.x_min,
                    f64::NEG_INFINITY..=f64::INFINITY,
                    78.0,
                );
                if edit.changed {
                    changed_group = Some(EditGroup::AxisXMinimum);
                }
                finish_coalescing |= edit.finish;
                ui.label(self.language.text(Text::XMax));
                let edit = deferred_f64_editor(
                    ui,
                    &mut self.numeric_inputs,
                    "axis-range-x-max",
                    &mut ranges.x_max,
                    f64::NEG_INFINITY..=f64::INFINITY,
                    78.0,
                );
                if edit.changed {
                    changed_group = Some(EditGroup::AxisXMaximum);
                }
                finish_coalescing |= edit.finish;
                ui.end_row();
                ui.label(self.language.text(Text::YMin));
                let edit = deferred_f64_editor(
                    ui,
                    &mut self.numeric_inputs,
                    "axis-range-y-min",
                    &mut ranges.y_min,
                    f64::NEG_INFINITY..=f64::INFINITY,
                    78.0,
                );
                if edit.changed {
                    changed_group = Some(EditGroup::AxisYMinimum);
                }
                finish_coalescing |= edit.finish;
                ui.label(self.language.text(Text::YMax));
                let edit = deferred_f64_editor(
                    ui,
                    &mut self.numeric_inputs,
                    "axis-range-y-max",
                    &mut ranges.y_max,
                    f64::NEG_INFINITY..=f64::INFINITY,
                    78.0,
                );
                if edit.changed {
                    changed_group = Some(EditGroup::AxisYMaximum);
                }
                finish_coalescing |= edit.finish;
                ui.end_row();
            });
        if let Some(group) = changed_group {
            self.apply_ranges(ranges, group);
        }
        if finish_coalescing {
            self.edit_history.finish_coalescing();
        }
    }

    #[allow(dead_code)]
    pub(crate) fn axis_editor(&mut self, ui: &mut egui::Ui, dimension: AxisDimension) {
        let title = match dimension {
            AxisDimension::X => self.language.text(Text::XAxis),
            AxisDimension::Y => self.language.text(Text::YAxis),
        };
        let mut record = self.document.axis_record(dimension);
        let mut changed = false;
        let mut continuous_change = false;
        let mut finish_coalescing = false;
        let mut fixed_ticks = match dimension {
            AxisDimension::X => self.x_fixed_ticks.clone(),
            AxisDimension::Y => self.y_fixed_ticks.clone(),
        };
        let mut fixed_tick_error = None;
        egui::CollapsingHeader::new(title)
            .default_open(true)
            .show(ui, |ui| {
                changed |= ui
                    .checkbox(&mut record.autoscale, self.language.text(Text::Autoscale))
                    .changed();
                ui.label(self.language.text(Text::Scale));
                egui::ComboBox::from_id_salt(("scale", dimension))
                    .selected_text(match record.scale {
                        AxisScale::Linear => self.language.text(Text::Linear),
                        AxisScale::Log10 => self.language.text(Text::Log10),
                    })
                    .show_ui(ui, |ui| {
                        changed |= ui
                            .selectable_value(
                                &mut record.scale,
                                AxisScale::Linear,
                                self.language.text(Text::Linear),
                            )
                            .changed();
                        changed |= ui
                            .selectable_value(
                                &mut record.scale,
                                AxisScale::Log10,
                                self.language.text(Text::Log10),
                            )
                            .changed();
                    });
                if record.scale == AxisScale::Log10
                    && matches!(record.locator, LocatorSpec::Interval { .. })
                {
                    record.locator = LocatorSpec::Auto { target_count: 6 };
                    changed = true;
                }
                if record.scale == AxisScale::Log10 && record.minor_interval.take().is_some() {
                    changed = true;
                }

                let locator_mode = match record.locator {
                    LocatorSpec::Auto { .. } => 0,
                    LocatorSpec::Interval { .. } => 1,
                    LocatorSpec::Fixed { .. } => 2,
                };
                ui.label(self.language.text(Text::Locator));
                egui::ComboBox::from_id_salt(("locator", dimension))
                    .selected_text(match locator_mode {
                        0 => self.language.text(Text::Auto),
                        1 => self.language.text(Text::Interval),
                        _ => self.language.text(Text::Fixed),
                    })
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(locator_mode == 0, self.language.text(Text::Auto))
                            .clicked()
                        {
                            record.locator = LocatorSpec::Auto { target_count: 6 };
                            changed = true;
                        }
                        if record.scale == AxisScale::Linear
                            && ui
                                .selectable_label(
                                    locator_mode == 1,
                                    self.language.text(Text::Interval),
                                )
                                .clicked()
                        {
                            record.locator = LocatorSpec::Interval {
                                step: instplot_layout::finite_span(record.minimum, record.maximum)
                                    .unwrap_or(5.0)
                                    / 5.0,
                            };
                            changed = true;
                        }
                        if ui
                            .selectable_label(locator_mode == 2, self.language.text(Text::Fixed))
                            .clicked()
                        {
                            let values = parse_fixed_ticks(&fixed_ticks)
                                .unwrap_or_else(|_| vec![record.minimum, record.maximum]);
                            record.locator = LocatorSpec::Fixed { values };
                            changed = true;
                        }
                    });
                match &mut record.locator {
                    LocatorSpec::Auto { target_count } => {
                        let response =
                            ui.add(egui::DragValue::new(target_count).range(2..=20).prefix(
                                format!("{}: ", self.language.text(Text::TargetTickCount)),
                            ));
                        changed |= response.changed();
                        continuous_change |= response.changed();
                        finish_coalescing |= response.drag_stopped() || response.lost_focus();
                    }
                    LocatorSpec::Interval { step } => {
                        ui.label(self.language.text(Text::MajorTickInterval));
                        let edit = deferred_f64_editor(
                            ui,
                            &mut self.numeric_inputs,
                            format!("axis-{dimension:?}-major-interval"),
                            step,
                            f64::MIN_POSITIVE..=f64::INFINITY,
                            132.0,
                        );
                        changed |= edit.changed;
                        finish_coalescing |= edit.finish;
                    }
                    LocatorSpec::Fixed { values } => {
                        ui.label(self.language.text(Text::FixedTickValues));
                        let before = fixed_ticks.clone();
                        let mut output = egui::TextEdit::singleline(&mut fixed_ticks)
                            .id_salt(("fixed-ticks", dimension))
                            .show(ui);
                        text_input::auto_pair_brackets(
                            ui,
                            &before,
                            &mut fixed_ticks,
                            &mut output,
                            text_input::BracketMode::Literal,
                        );
                        if ui.button(self.language.text(Text::Apply)).clicked() {
                            match parse_fixed_ticks(&fixed_ticks) {
                                Ok(parsed) => {
                                    *values = parsed;
                                    changed = true;
                                }
                                Err(error) => fixed_tick_error = Some(error),
                            }
                        }
                    }
                }
                let minor_edit = minor_interval_editor(
                    ui,
                    self.language,
                    match dimension {
                        AxisDimension::X => AxisIdentity::X1,
                        AxisDimension::Y => AxisIdentity::Y1,
                    },
                    &mut record,
                    &mut self.numeric_inputs,
                );
                changed |= minor_edit.changed;
                continuous_change |= minor_edit.continuous;
                finish_coalescing |= minor_edit.finish;

                let formatter_kind = match record.formatter {
                    FormatterSpec::Auto => 0,
                    FormatterSpec::Decimal { .. } => 1,
                    FormatterSpec::Scientific { .. } => 2,
                };
                ui.label(self.language.text(Text::Formatter));
                egui::ComboBox::from_id_salt(("formatter", dimension))
                    .selected_text(match formatter_kind {
                        0 => self.language.text(Text::Auto),
                        1 => self.language.text(Text::Decimal),
                        _ => self.language.text(Text::Scientific),
                    })
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(formatter_kind == 0, self.language.text(Text::Auto))
                            .clicked()
                        {
                            record.formatter = FormatterSpec::Auto;
                            changed = true;
                        }
                        if ui
                            .selectable_label(
                                formatter_kind == 1,
                                self.language.text(Text::Decimal),
                            )
                            .clicked()
                        {
                            record.formatter = FormatterSpec::Decimal { precision: 2 };
                            changed = true;
                        }
                        if ui
                            .selectable_label(
                                formatter_kind == 2,
                                self.language.text(Text::Scientific),
                            )
                            .clicked()
                        {
                            record.formatter = FormatterSpec::Scientific { precision: 2 };
                            changed = true;
                        }
                    });
                if let FormatterSpec::Decimal { precision }
                | FormatterSpec::Scientific { precision } = &mut record.formatter
                {
                    let response = ui.add(
                        egui::DragValue::new(precision)
                            .range(0..=15)
                            .prefix(format!("{}: ", self.language.text(Text::Precision))),
                    );
                    changed |= response.changed();
                    continuous_change |= response.changed();
                    finish_coalescing |= response.drag_stopped() || response.lost_focus();
                }

                ui.separator();
                changed |= ui
                    .checkbox(
                        &mut record.appearance.near_spine,
                        self.language.text(Text::NearSpine),
                    )
                    .changed();
                changed |= ui
                    .checkbox(
                        &mut record.appearance.far_spine,
                        self.language.text(Text::FarSpine),
                    )
                    .changed();
                changed |= ui
                    .checkbox(
                        &mut record.appearance.near_ticks,
                        self.language.text(Text::NearTicks),
                    )
                    .changed();
                changed |= ui
                    .checkbox(
                        &mut record.appearance.far_ticks,
                        self.language.text(Text::FarTicks),
                    )
                    .changed();
                changed |= ui
                    .checkbox(
                        &mut record.appearance.near_tick_labels,
                        self.language.text(Text::NearTickLabels),
                    )
                    .changed();
                changed |= ui
                    .checkbox(
                        &mut record.appearance.far_tick_labels,
                        self.language.text(Text::FarTickLabels),
                    )
                    .changed();
                changed |= ui
                    .checkbox(
                        &mut record.appearance.major_ticks,
                        self.language.text(Text::MajorTicks),
                    )
                    .changed();
                changed |= ui
                    .checkbox(
                        &mut record.appearance.minor_ticks,
                        self.language.text(Text::MinorTicks),
                    )
                    .changed();
            });
        match dimension {
            AxisDimension::X => self.x_fixed_ticks = fixed_ticks,
            AxisDimension::Y => self.y_fixed_ticks = fixed_ticks,
        }
        if let Some(error) = fixed_tick_error {
            self.push_error("fixed-ticks", error);
        }
        if changed {
            let group = continuous_change.then_some(match dimension {
                AxisDimension::X => EditGroup::AxisXSettings,
                AxisDimension::Y => EditGroup::AxisYSettings,
            });
            self.execute_document_edit_with_group(
                EditCommand::SetAxisRecord { dimension, record },
                title,
                group,
            );
        }
        if finish_coalescing {
            self.edit_history.finish_coalescing();
        }
        self.axis_label_editor(ui, dimension);
    }

    pub(crate) fn axis_quick_editor_by_identity(
        &mut self,
        ui: &mut egui::Ui,
        identity: AxisIdentity,
    ) {
        let Some(mut record) = self.document.axis_record_by_identity(identity) else {
            return;
        };
        let current_exponent = effective_axis_input_exponent(&record, &self.resolved, identity);
        let numeric_keys = quick_axis_numeric_keys(identity);
        let has_focused_input = numeric_keys.iter().any(|key| {
            ui.memory(|memory| memory.has_focus(egui::Id::new(("deferred-number", key))))
        });
        let has_invalid_draft = numeric_keys.iter().any(|key| {
            self.numeric_inputs
                .get(key)
                .is_some_and(|input| input.error.is_some())
        });
        if !has_focused_input && !has_invalid_draft {
            self.axis_numeric_scale_sessions
                .retain(|session| session.identity != identity);
        }
        let frozen_exponent = self
            .axis_numeric_scale_sessions
            .iter()
            .find(|session| session.identity == identity)
            .map(|session| session.exponent)
            .or(current_exponent);
        if let Some(exponent) = frozen_exponent
            && !self
                .axis_numeric_scale_sessions
                .iter()
                .any(|session| session.identity == identity)
        {
            self.axis_numeric_scale_sessions
                .push(AxisNumericScaleSession { identity, exponent });
        }
        let dimension = axis_dimension(identity);
        let title = axis_title(self.language, identity);
        ui.strong(self.language.text(Text::AxesRanges));
        if let Some(exponent) = frozen_exponent
            && exponent != 0
        {
            ui.weak(format!("输入尺度：×10^{exponent}"));
        }
        ui.add_space(2.0);
        let mut changed = ui
            .checkbox(&mut record.autoscale, self.language.text(Text::Autoscale))
            .changed();
        let mut continuous = false;
        let mut finish = false;
        let mut scale_error = None;
        ui.add_enabled_ui(!record.autoscale, |ui| {
            ui.horizontal(|ui| {
                ui.label(self.language.text(Text::Minimum));
                let mut minimum_value = frozen_exponent
                    .and_then(|exponent| raw_to_axis_display(record.minimum, exponent).ok())
                    .unwrap_or(record.minimum);
                let minimum = deferred_f64_editor(
                    ui,
                    &mut self.numeric_inputs,
                    format!("quick-axis-{identity:?}-minimum"),
                    &mut minimum_value,
                    f64::NEG_INFINITY..=f64::INFINITY,
                    92.0,
                );
                if minimum.changed {
                    match frozen_exponent
                        .ok_or_else(|| "当前坐标轴倍率尚未解析".to_owned())
                        .and_then(|exponent| axis_display_to_raw(minimum_value, exponent))
                    {
                        Ok(value) => {
                            record.minimum = value;
                            changed = true;
                        }
                        Err(error) => scale_error = Some(error),
                    }
                }
                finish |= minimum.finish;
                ui.label(self.language.text(Text::Maximum));
                let mut maximum_value = frozen_exponent
                    .and_then(|exponent| raw_to_axis_display(record.maximum, exponent).ok())
                    .unwrap_or(record.maximum);
                let maximum = deferred_f64_editor(
                    ui,
                    &mut self.numeric_inputs,
                    format!("quick-axis-{identity:?}-maximum"),
                    &mut maximum_value,
                    f64::NEG_INFINITY..=f64::INFINITY,
                    92.0,
                );
                if maximum.changed {
                    match frozen_exponent
                        .ok_or_else(|| "当前坐标轴倍率尚未解析".to_owned())
                        .and_then(|exponent| axis_display_to_raw(maximum_value, exponent))
                    {
                        Ok(value) => {
                            record.maximum = value;
                            changed = true;
                        }
                        Err(error) => scale_error = Some(error),
                    }
                }
                finish |= maximum.finish;
            });
        });
        if record.scale == AxisScale::Linear {
            ui.add_space(6.0);
            ui.separator();
            ui.add_space(4.0);
            let mode = match record.locator {
                LocatorSpec::Auto { .. } => 0,
                LocatorSpec::Interval { .. } => 1,
                LocatorSpec::Fixed { .. } => 2,
            };
            ui.horizontal_wrapped(|ui| {
                ui.strong(self.language.text(Text::MajorTicks));
                egui::ComboBox::from_id_salt(("quick-locator", identity))
                    .selected_text(match mode {
                        0 => self.language.text(Text::Auto),
                        1 => self.language.text(Text::Interval),
                        _ => self.language.text(Text::Fixed),
                    })
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(mode == 0, self.language.text(Text::Auto))
                            .clicked()
                        {
                            record.locator = LocatorSpec::Auto { target_count: 6 };
                            changed = true;
                        }
                        if ui
                            .selectable_label(mode == 1, self.language.text(Text::Interval))
                            .clicked()
                        {
                            record.locator = LocatorSpec::Interval {
                                step: instplot_layout::finite_span(record.minimum, record.maximum)
                                    .unwrap_or(5.0)
                                    / 5.0,
                            };
                            changed = true;
                        }
                    });
                if let LocatorSpec::Interval { step } = &mut record.locator {
                    ui.label(self.language.text(Text::MajorTickInterval));
                    let mut display_step = frozen_exponent
                        .and_then(|exponent| raw_to_axis_display(*step, exponent).ok())
                        .unwrap_or(*step);
                    let edit = deferred_f64_editor(
                        ui,
                        &mut self.numeric_inputs,
                        format!("quick-axis-{identity:?}-major-interval"),
                        &mut display_step,
                        f64::MIN_POSITIVE..=f64::INFINITY,
                        112.0,
                    );
                    if edit.changed {
                        match frozen_exponent
                            .ok_or_else(|| "当前坐标轴倍率尚未解析".to_owned())
                            .and_then(|exponent| axis_display_to_raw(display_step, exponent))
                        {
                            Ok(value) if value > 0.0 => {
                                *step = value;
                                changed = true;
                            }
                            Ok(_) => scale_error = Some("刻度间隔必须大于 0".to_owned()),
                            Err(error) => scale_error = Some(error),
                        }
                    }
                    finish |= edit.finish;
                }
            });
            ui.add_space(5.0);
            let mut display_record = record.clone();
            if let Some(exponent) = frozen_exponent {
                display_record.minimum =
                    raw_to_axis_display(record.minimum, exponent).unwrap_or(record.minimum);
                display_record.maximum =
                    raw_to_axis_display(record.maximum, exponent).unwrap_or(record.maximum);
                display_record.minor_interval = record
                    .minor_interval
                    .and_then(|value| raw_to_axis_display(value, exponent).ok());
                if let LocatorSpec::Interval { step } = &mut display_record.locator {
                    *step = raw_to_axis_display(*step, exponent).unwrap_or(*step);
                }
            }
            let minor_edit = minor_interval_editor(
                ui,
                self.language,
                identity,
                &mut display_record,
                &mut self.numeric_inputs,
            );
            if minor_edit.changed {
                match display_record.minor_interval {
                    None => {
                        record.minor_interval = None;
                        changed = true;
                    }
                    Some(value) => match frozen_exponent
                        .ok_or_else(|| "当前坐标轴倍率尚未解析".to_owned())
                        .and_then(|exponent| axis_display_to_raw(value, exponent))
                    {
                        Ok(value) if value > 0.0 => {
                            record.minor_interval = Some(value);
                            changed = true;
                        }
                        Ok(_) => scale_error = Some("副刻度间隔必须大于 0".to_owned()),
                        Err(error) => scale_error = Some(error),
                    },
                }
            }
            continuous |= minor_edit.continuous;
            finish |= minor_edit.finish;
        }
        let mode = self.document.axis_mode();
        let spine_color_visible = matches!(
            (mode, identity),
            (AxisMode::DualX, AxisIdentity::X1 | AxisIdentity::X2)
                | (AxisMode::DualY, AxisIdentity::Y1 | AxisIdentity::Y2)
        );
        if spine_color_visible {
            ui.add_space(6.0);
            ui.separator();
            ui.strong(match identity {
                AxisIdentity::X1 => "下方 X1 spine",
                AxisIdentity::X2 => "上方 X2 spine",
                AxisIdentity::Y1 => "左侧 Y1 spine",
                AxisIdentity::Y2 => "右侧 Y2 spine",
            });
            let palette = self.document.palette_colors().to_vec();
            changed |= color_editor(
                ui,
                self.language,
                &format!("axis-spine-{identity:?}"),
                &mut record.appearance.spine_color_id,
                &palette,
            );
        }
        if changed {
            self.execute_document_edit_with_group(
                EditCommand::SetAxisRecordByIdentity { identity, record },
                &title,
                continuous.then_some(match dimension {
                    AxisDimension::X => EditGroup::AxisXSettings,
                    AxisDimension::Y => EditGroup::AxisYSettings,
                }),
            );
        }
        if let Some(error) = scale_error {
            self.push_error("axis-display-scale-input", error);
        }
        if finish {
            self.edit_history.finish_coalescing();
        }
    }

    pub(crate) fn axis_label_editor(&mut self, ui: &mut egui::Ui, dimension: AxisDimension) {
        let identity = match dimension {
            AxisDimension::X => AxisIdentity::X1,
            AxisDimension::Y => AxisIdentity::Y1,
        };
        self.axis_label_editor_by_identity(ui, identity);
    }

    pub(crate) fn axis_label_editor_by_identity(
        &mut self,
        ui: &mut egui::Ui,
        identity: AxisIdentity,
    ) {
        self.axis_display_scale_editor(ui, identity);
        let current = self.document.axis_label_by_identity(identity).to_vec();
        let key = match identity {
            AxisIdentity::X1 => "axis-label-x1",
            AxisIdentity::X2 => "axis-label-x2",
            AxisIdentity::Y1 => "axis-label-y1",
            AxisIdentity::Y2 => "axis-label-y2",
        };
        let initial = label_input::format(&current)
            .or_else(|| {
                self.label_inputs
                    .get(key)
                    .filter(|state| state.source_nodes == current)
                    .map(|state| state.text.clone())
            })
            .unwrap_or_else(|| label_input::display_text(&current));
        self.label_input_editor(ui, key, current, initial, LabelInputTarget::Axis(identity));
    }

    fn axis_display_scale_editor(&mut self, ui: &mut egui::Ui, identity: AxisIdentity) {
        let Some(mut record) = self.document.axis_record_by_identity(identity) else {
            return;
        };
        let current_nodes = self.document.axis_label_by_identity(identity);
        let has_slot = label_has_scale_slot(current_nodes);
        let mut changed = false;
        let mut start_incorporated = None;
        let mode = match record.display_scale {
            AxisDisplayScaleRecord::AutoFactor => 0,
            AxisDisplayScaleRecord::None => 1,
            AxisDisplayScaleRecord::ManualFactor { .. } => 2,
            AxisDisplayScaleRecord::ManualIncorporated { .. } => 3,
        };
        let resolved_automatic = resolved_axis_exponent(&self.resolved, identity);
        let automatic = resolved_automatic.unwrap_or(0);
        ui.horizontal_wrapped(|ui| {
            ui.label("显示倍率");
            egui::ComboBox::from_id_salt(("axis-display-scale", identity))
                .selected_text(match mode {
                    0 => "自动",
                    1 => "无",
                    2 => "自定义倍率",
                    _ => "已写入单位",
                })
                .show_ui(ui, |ui| {
                    if ui.selectable_label(mode == 0, "自动").clicked() {
                        record.display_scale = AxisDisplayScaleRecord::AutoFactor;
                        changed = true;
                    }
                    if ui.selectable_label(mode == 1, "无").clicked() && !has_slot {
                        record.display_scale = AxisDisplayScaleRecord::None;
                        changed = true;
                    }
                    if ui.selectable_label(mode == 2, "自定义倍率").clicked() {
                        let exponent = record.display_scale.exponent().unwrap_or(automatic);
                        record.display_scale =
                            AxisDisplayScaleRecord::manual_factor(if exponent == 0 {
                                -3
                            } else {
                                exponent
                            })
                            .expect("the UI supplies a non-zero exponent");
                        changed = true;
                    }
                    ui.add_enabled_ui(
                        automatic != 0 || record.display_scale.exponent().is_some(),
                        |ui| {
                            if ui.selectable_label(mode == 3, "已写入单位").clicked() {
                                start_incorporated =
                                    Some(record.display_scale.exponent().unwrap_or(automatic));
                            }
                        },
                    );
                });
            if let Some(mut exponent) = record.display_scale.exponent() {
                ui.label("10^");
                let edit = deferred_i32_editor(
                    ui,
                    &mut self.numeric_inputs,
                    format!("axis-display-exponent-{identity:?}"),
                    &mut exponent,
                    -323..=308,
                    54.0,
                );
                if edit.changed {
                    record.display_scale = if exponent == 0 {
                        AxisDisplayScaleRecord::None
                    } else if matches!(
                        record.display_scale,
                        AxisDisplayScaleRecord::ManualIncorporated { .. }
                    ) {
                        AxisDisplayScaleRecord::manual_incorporated(exponent).unwrap()
                    } else {
                        AxisDisplayScaleRecord::manual_factor(exponent).unwrap()
                    };
                    changed = true;
                }
            }
        });
        if let Some(exponent) = start_incorporated
            && !self
                .axis_scale_transitions
                .iter()
                .any(|draft| draft.identity == identity)
        {
            let nodes = remove_scale_slots(current_nodes);
            let label_text =
                label_input::format(&nodes).unwrap_or_else(|| label_input::display_text(&nodes));
            self.axis_scale_transitions.push(AxisScaleTransitionDraft {
                identity,
                exponent,
                label_text,
                error: None,
            });
        }
        let draft_index = self
            .axis_scale_transitions
            .iter()
            .position(|draft| draft.identity == identity);
        let mut apply_transition = None;
        let mut cancel_transition = false;
        if let Some(index) = draft_index {
            let draft = &mut self.axis_scale_transitions[index];
            ui.group(|ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label("写入单位后的标签");
                    ui.add_sized(
                        [220.0, ui.spacing().interact_size.y],
                        egui::TextEdit::singleline(&mut draft.label_text),
                    );
                    if ui.button("应用").clicked() {
                        apply_transition = Some((draft.exponent, draft.label_text.clone()));
                    }
                    if ui.button("取消").clicked() {
                        cancel_transition = true;
                    }
                });
                ui.weak("请先把 Ω 等单位改成 mΩ、μA、kPa 等对应写法；应用后与倍率状态一次撤销。");
                if let Some(error) = &draft.error {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                }
            });
        }
        if has_slot {
            ui.weak("{scale} 指定 ×10ⁿ 的位置；切换到“已写入单位”时会在同一步删除它。");
        } else if matches!(
            record.display_scale,
            AxisDisplayScaleRecord::ManualIncorporated { .. }
        ) {
            ui.weak("倍率已写入单位；修改指数后请同步检查轴标签单位。");
        } else {
            ui.weak("可在标签中输入 {scale}；未输入时，非零倍率会追加到标签末尾。");
        }
        if cancel_transition {
            self.axis_scale_transitions
                .retain(|draft| draft.identity != identity);
        }
        if let Some((exponent, text)) = apply_transition {
            match label_input::parse(&text).map(|nodes| remove_scale_slots(&nodes)) {
                Ok(nodes) if !nodes.is_empty() => {
                    record.display_scale =
                        AxisDisplayScaleRecord::manual_incorporated(exponent).unwrap();
                    if self.execute_document_edit_with_group(
                        EditCommand::SetAxisScaleAndLabel {
                            identity,
                            record: record.clone(),
                            nodes,
                        },
                        "倍率和轴标签已在同一步写入单位",
                        None,
                    ) {
                        self.axis_scale_transitions
                            .retain(|draft| draft.identity != identity);
                    }
                }
                Ok(_) => {
                    if let Some(draft) = self
                        .axis_scale_transitions
                        .iter_mut()
                        .find(|draft| draft.identity == identity)
                    {
                        draft.error = Some("轴标签不能为空".to_owned());
                    }
                }
                Err(error) => {
                    if let Some(draft) = self
                        .axis_scale_transitions
                        .iter_mut()
                        .find(|draft| draft.identity == identity)
                    {
                        draft.error = Some(error);
                    }
                }
            }
        } else if changed {
            self.axis_scale_transitions
                .retain(|draft| draft.identity != identity);
            self.clear_axis_numeric_drafts(&[identity]);
            let incorporated_exponent_changed = matches!(
                record.display_scale,
                AxisDisplayScaleRecord::ManualIncorporated { .. }
            );
            self.execute_document_edit_with_group(
                EditCommand::SetAxisRecordByIdentity { identity, record },
                if incorporated_exponent_changed {
                    "已修改倍率指数；请同步检查轴标签单位"
                } else {
                    "修改轴显示倍率"
                },
                None,
            );
        }
    }

    pub(crate) fn sync_axis_editors(&mut self) {
        self.x_fixed_ticks = fixed_ticks_text(&self.document.axis_record(AxisDimension::X));
        self.y_fixed_ticks = fixed_ticks_text(&self.document.axis_record(AxisDimension::Y));
    }
}

fn label_has_scale_slot(nodes: &[LabelNode]) -> bool {
    nodes.iter().any(|node| match node {
        LabelNode::ScaleFactorSlot => true,
        LabelNode::DescriptiveSubscript(nodes)
        | LabelNode::VariableSubscript(nodes)
        | LabelNode::Superscript(nodes) => label_has_scale_slot(nodes),
        _ => false,
    })
}

fn remove_scale_slots(nodes: &[LabelNode]) -> Vec<LabelNode> {
    nodes
        .iter()
        .filter_map(|node| match node {
            LabelNode::ScaleFactorSlot => None,
            LabelNode::DescriptiveSubscript(children) => Some(LabelNode::DescriptiveSubscript(
                remove_scale_slots(children),
            )),
            LabelNode::VariableSubscript(children) => {
                Some(LabelNode::VariableSubscript(remove_scale_slots(children)))
            }
            LabelNode::Superscript(children) => {
                Some(LabelNode::Superscript(remove_scale_slots(children)))
            }
            node => Some(node.clone()),
        })
        .collect()
}
