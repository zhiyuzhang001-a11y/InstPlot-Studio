use super::*;

#[derive(Clone)]
struct FontSelection {
    selected: [bool; 6],
}

impl Default for FontSelection {
    fn default() -> Self {
        Self {
            selected: [true; 6],
        }
    }
}

fn selected_sizes(current: [f64; 6], selected: [bool; 6], delta: f64) -> [f64; 6] {
    std::array::from_fn(|index| {
        if selected[index] {
            (current[index] + delta).clamp(4.0, 48.0)
        } else {
            current[index]
        }
    })
}

impl StudioApp {
    pub(crate) fn plot_fonts_editor(&mut self, ui: &mut egui::Ui) {
        let id = egui::Id::new("plot-font-selection");
        let mut state = ui
            .ctx()
            .data_mut(|data| data.get_temp::<FontSelection>(id).unwrap_or_default());
        ui.label("图中文字字号（pt）");
        ui.weak("仅调整当前图；预览与导出一致，可撤销。");
        ui.horizontal(|ui| {
            if ui.button("全选").clicked() {
                state.selected = [true; 6];
            }
            if ui.button("取消全选").clicked() {
                state.selected = [false; 6];
            }
        });
        let current = self.document.project().typography.font_sizes_pt;
        for (index, label) in [
            "X 轴标题（含 X2）",
            "Y 轴标题（含 Y2）",
            "X 轴刻度",
            "Y 轴刻度",
            "图例",
            "标注 / 测量文字",
        ]
        .iter()
        .enumerate()
        {
            ui.checkbox(
                &mut state.selected[index],
                format!("{label}  ·  {} pt", current[index]),
            );
        }
        ui.separator();
        let enabled = state.selected.iter().any(|selected| *selected);
        let mut requested = None;
        ui.add_enabled_ui(enabled, |ui| {
            ui.horizontal(|ui| {
                if ui.button("−1 pt").clicked() {
                    requested = Some(selected_sizes(current, state.selected, -1.0));
                }
                if ui.button("+1 pt").clicked() {
                    requested = Some(selected_sizes(current, state.selected, 1.0));
                }
            });
        });
        ui.ctx().data_mut(|data| data.insert_temp(id, state));
        if let Some(sizes_pt) = requested {
            self.execute_document_edit(EditCommand::SetPlotFontSizes { sizes_pt }, "调整字号");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_selection_defaults_to_all_and_preserves_unselected_categories() {
        assert_eq!(FontSelection::default().selected, [true; 6]);
        assert_eq!(
            selected_sizes(
                [9.0, 9.0, 8.0, 8.0, 8.0, 8.0],
                [true, false, true, false, false, false],
                1.0
            ),
            [10.0, 9.0, 9.0, 8.0, 8.0, 8.0]
        );
        assert_eq!(selected_sizes([11.0; 6], [true; 6], -1.0), [10.0; 6]);
        assert_eq!(selected_sizes([4.0; 6], [true; 6], -1.0), [4.0; 6]);
        assert_eq!(selected_sizes([48.0; 6], [true; 6], 1.0), [48.0; 6]);
    }
}
