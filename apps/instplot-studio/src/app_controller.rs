use super::*;

mod artist_editor;
mod axis_editor;
mod coordination;
mod data_workflow;
mod export_workflow;
mod interaction;
mod lifecycle;
mod selection_windows;
mod tool_windows;

pub(super) fn export_confirmation_text(language: UiLanguage, error_count: usize) -> &'static str {
    language.text(if error_count > 0 {
        Text::ExportAnyway
    } else {
        Text::ExportNow
    })
}

fn axis_dimension(identity: AxisIdentity) -> AxisDimension {
    match identity {
        AxisIdentity::X1 | AxisIdentity::X2 => AxisDimension::X,
        AxisIdentity::Y1 | AxisIdentity::Y2 => AxisDimension::Y,
    }
}

fn extend_axis_identities(identities: &mut Vec<AxisIdentity>, binding: AxisBinding) {
    let candidates = [
        match binding.x {
            XAxisSlot::X1 => AxisIdentity::X1,
            XAxisSlot::X2 => AxisIdentity::X2,
        },
        match binding.y {
            YAxisSlot::Y1 => AxisIdentity::Y1,
            YAxisSlot::Y2 => AxisIdentity::Y2,
        },
    ];
    for identity in candidates {
        if !identities.contains(&identity) {
            identities.push(identity);
        }
    }
}

fn axis_title(language: UiLanguage, identity: AxisIdentity) -> String {
    let suffix = match language {
        UiLanguage::Chinese => "轴",
        UiLanguage::English => "Axis",
    };
    match identity {
        AxisIdentity::X1 => format!("X1 {suffix}"),
        AxisIdentity::X2 => format!("X2 {suffix}"),
        AxisIdentity::Y1 => format!("Y1 {suffix}"),
        AxisIdentity::Y2 => format!("Y2 {suffix}"),
    }
}

fn axis_binding_name(binding: AxisBinding) -> &'static str {
    match (binding.x, binding.y) {
        (XAxisSlot::X1, YAxisSlot::Y1) => "X1 / Y1",
        (XAxisSlot::X2, YAxisSlot::Y1) => "X2 / Y1",
        (XAxisSlot::X1, YAxisSlot::Y2) => "X1 / Y2",
        (XAxisSlot::X2, YAxisSlot::Y2) => "不支持",
    }
}

fn coordinates_for_binding(
    coordinates: HoverDataCoordinates,
    binding: AxisBinding,
) -> Option<(f64, f64)> {
    let x = match binding.x {
        XAxisSlot::X1 => coordinates.x1,
        XAxisSlot::X2 => coordinates.x2?,
    };
    let y = match binding.y {
        YAxisSlot::Y1 => coordinates.y1,
        YAxisSlot::Y2 => coordinates.y2?,
    };
    Some((x, y))
}

fn axis_binding_editor(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    mode: AxisMode,
    binding: &mut AxisBinding,
) -> bool {
    let before = *binding;
    ui.horizontal(|ui| {
        ui.label("坐标轴");
        egui::ComboBox::from_id_salt(id_salt)
            .selected_text(axis_binding_name(*binding))
            .show_ui(ui, |ui| {
                ui.selectable_value(binding, AxisBinding::PRIMARY, "X1 / Y1");
                if mode == AxisMode::DualY {
                    ui.selectable_value(
                        binding,
                        AxisBinding {
                            x: XAxisSlot::X1,
                            y: YAxisSlot::Y2,
                        },
                        "X1 / Y2",
                    );
                }
                if mode == AxisMode::DualX {
                    ui.selectable_value(
                        binding,
                        AxisBinding {
                            x: XAxisSlot::X2,
                            y: YAxisSlot::Y1,
                        },
                        "X2 / Y1",
                    );
                }
            });
    });
    if !binding.is_enabled_in(mode) {
        ui.weak("所绑定的副轴当前已关闭；此数据坐标对象暂时隐藏。");
    }
    *binding != before
}

fn reference_axis_binding_editor(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    mode: AxisMode,
    orientation: ReferenceOrientation,
    binding: &mut AxisBinding,
) -> bool {
    let before = *binding;
    ui.horizontal(|ui| {
        ui.label("坐标轴");
        egui::ComboBox::from_id_salt(id_salt)
            .selected_text(match orientation {
                ReferenceOrientation::Vertical => match binding.x {
                    XAxisSlot::X1 => "X1",
                    XAxisSlot::X2 => "X2",
                },
                ReferenceOrientation::Horizontal => match binding.y {
                    YAxisSlot::Y1 => "Y1",
                    YAxisSlot::Y2 => "Y2",
                },
            })
            .show_ui(ui, |ui| match orientation {
                ReferenceOrientation::Vertical => {
                    ui.selectable_value(&mut binding.x, XAxisSlot::X1, "X1");
                    if mode == AxisMode::DualX {
                        ui.selectable_value(&mut binding.x, XAxisSlot::X2, "X2");
                    }
                    binding.y = YAxisSlot::Y1;
                }
                ReferenceOrientation::Horizontal => {
                    ui.selectable_value(&mut binding.y, YAxisSlot::Y1, "Y1");
                    if mode == AxisMode::DualY {
                        ui.selectable_value(&mut binding.y, YAxisSlot::Y2, "Y2");
                    }
                    binding.x = XAxisSlot::X1;
                }
            });
    });
    if !binding.is_enabled_in(mode) {
        ui.weak("所绑定的副轴当前已关闭；此参考线暂时隐藏。");
    }
    *binding != before
}

fn explicit_axis_unit_conflicts(document: &FigureDocument, selected: AxisBinding) -> Vec<String> {
    let mut x_units = BTreeSet::new();
    let mut y_units = BTreeSet::new();
    for series in document.series() {
        let Some(binding) = series.binding.as_ref() else {
            continue;
        };
        let Some(axes) = series.axes else {
            continue;
        };
        if axes.x == selected.x
            && let Some(unit) = explicit_column_unit(&binding.x_column)
        {
            x_units.insert(unit.to_owned());
        }
        if axes.y == selected.y
            && let Some(unit) = explicit_column_unit(&binding.y_column)
        {
            y_units.insert(unit.to_owned());
        }
    }
    let mut warnings = Vec::new();
    if x_units.len() > 1 {
        warnings.push(format!(
            "{} 上存在明确且不一致的单位：{}；请检查曲线分配。",
            match selected.x {
                XAxisSlot::X1 => "X1",
                XAxisSlot::X2 => "X2",
            },
            x_units.into_iter().collect::<Vec<_>>().join("、")
        ));
    }
    if y_units.len() > 1 {
        warnings.push(format!(
            "{} 上存在明确且不一致的单位：{}；请检查曲线分配。",
            match selected.y {
                YAxisSlot::Y1 => "Y1",
                YAxisSlot::Y2 => "Y2",
            },
            y_units.into_iter().collect::<Vec<_>>().join("、")
        ));
    }
    warnings
}

pub(super) fn explicit_column_unit(column: &str) -> Option<&str> {
    let trimmed = column.trim();
    for (open, close) in [('(', ')'), ('[', ']')] {
        let Some(start) = trimmed.rfind(open) else {
            continue;
        };
        if trimmed.ends_with(close) && start + open.len_utf8() < trimmed.len() - close.len_utf8() {
            return Some(&trimmed[start + open.len_utf8()..trimmed.len() - close.len_utf8()]);
        }
    }
    None
}

fn managed_format_from_path(path: &Path) -> Option<ManagedDataFormat> {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("csv") => Some(ManagedDataFormat::Csv),
        Some("tsv") => Some(ManagedDataFormat::Tsv),
        Some("txt") => Some(ManagedDataFormat::Txt),
        Some("dat") => Some(ManagedDataFormat::Dat),
        Some("xlsx") => Some(ManagedDataFormat::Xlsx),
        _ => None,
    }
}

fn managed_save_conflict_explanation(diagnostics: &[String]) -> Option<String> {
    let details = diagnostics.join("；");
    if details.contains("changed outside Studio") {
        Some("关联的手动数据文件已被其他程序修改。覆盖会用当前项目数据替换外部更改；另存为可保留两份；重新打开项目会放弃本次未保存修改。".to_owned())
    } else if details.contains("managed manual data file is missing") {
        Some("关联的手动数据文件已移动或删除。覆盖会按原路径重新创建；另存为可选择新位置；重新打开项目会放弃本次未保存修改。".to_owned())
    } else if details.contains("refusing to overwrite an unrelated file") {
        Some(
            "目标位置已有不属于当前项目的文件。覆盖会替换该文件；另存为可选择安全的新位置。"
                .to_owned(),
        )
    } else {
        None
    }
}
