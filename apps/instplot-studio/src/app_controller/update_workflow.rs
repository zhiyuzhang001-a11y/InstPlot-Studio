use super::*;

impl StudioApp {
    /// Read-only inventory; saving the project must not silently discard editor drafts.
    pub(crate) fn pending_update_drafts(&self) -> Vec<String> {
        let mut pending = Vec::new();
        if self.manual_data.open {
            let groups = self
                .document
                .project()
                .data_sources
                .iter()
                .filter_map(|source| source.manual_recipe.as_ref())
                .map(ManualDataGroupInput::from_recipe)
                .collect::<Vec<_>>();
            let baseline = if groups.is_empty() {
                ManualDataInput::default()
            } else {
                ManualDataInput { groups }
            };
            if self.manual_data.input != baseline {
                pending
                    .push("录入数据仍有未应用的修改，请先绘图/应用，或在该窗口取消。".to_owned());
            }
        }
        if self.numeric_inputs.values().any(|input| {
            input.error.is_some()
                || input.text.trim().parse::<f64>().map_or(true, |value| {
                    !value.is_finite() || value != input.source_value
                })
        }) {
            pending.push("数值输入仍未应用，请输入有效数值后按 Enter，或按 Esc 放弃。".to_owned());
        }
        if self.label_inputs.values().any(|input| {
            label_input::parse(&input.text).map_or(true, |nodes| nodes != input.source_nodes)
        }) {
            pending.push("文字输入仍未应用，请先修正或明确放弃该编辑。".to_owned());
        }
        if !self.axis_scale_transitions.is_empty() {
            pending.push("坐标轴缩放编辑仍待确认。".to_owned());
        }
        if self.tool_draft.is_some()
            || self.reference_draft.is_some()
            || self.active_artist_drag.is_some()
        {
            pending.push("绘图工具仍在编辑，请先完成或取消当前操作。".to_owned());
        }
        if self.pending_managed_save_conflict.is_some() || self.pending_action.is_some() {
            pending.push("请先处理现有的保存冲突或退出确认。".to_owned());
        }
        pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> StudioApp {
        let creation = eframe::CreationContext::_new_kittest(egui::Context::default());
        StudioApp::new(&creation, Instant::now(), None)
    }

    #[test]
    fn project_clean_does_not_hide_unapplied_manual_data() {
        let mut app = app();
        assert!(app.pending_update_drafts().is_empty());
        app.prepare_manual_data_window();
        assert!(app.pending_update_drafts().is_empty());
        app.manual_data.input.groups[0].x.measurements[0] = "1\n2".into();
        assert!(!app.edit_history.is_dirty(&app.document));
        assert!(
            app.pending_update_drafts()
                .iter()
                .any(|text| text.contains("录入"))
        );
        assert_eq!(app.manual_data.input.groups[0].x.measurements[0], "1\n2");
    }

    #[test]
    fn pending_numbers_text_and_tool_drafts_are_preserved() {
        let mut app = app();
        app.numeric_inputs.insert(
            "test".into(),
            DeferredNumericInput {
                source_value: 1.0,
                text: "".into(),
                error: None,
            },
        );
        app.label_inputs.insert(
            "test".into(),
            LabelInputState {
                source_nodes: label_input::parse("label").unwrap(),
                text: "${".into(),
            },
        );
        app.reference_draft = Some(ReferenceDraft {
            orientation: ReferenceOrientation::Vertical,
            value: 1.0,
            axes: AxisBinding::default(),
            stroke: StrokeStyle {
                color_id: "black".into(),
                width_pt: 0.9,
                dash_pt: vec![],
            },
            include_in_autoscale: false,
        });
        let pending = app.pending_update_drafts();
        assert!(pending.iter().any(|text| text.contains("数值")));
        assert!(pending.iter().any(|text| text.contains("文字")));
        assert!(pending.iter().any(|text| text.contains("工具")));
        assert_eq!(app.numeric_inputs["test"].text, "");
        assert!(app.reference_draft.is_some());
    }

    #[test]
    fn failed_project_save_keeps_restart_pending_and_old_application_open() {
        let mut app = app();
        let mut random = [0_u8; 16];
        getrandom::fill(&mut random).unwrap();
        let missing_parent = std::env::temp_dir().join(format!(
            "studio-update-missing-{:032x}",
            u128::from_le_bytes(random)
        ));
        assert!(!missing_parent.exists());
        assert!(app.execute_document_edit(
            EditCommand::SetFigureSize {
                width_mm: 91.0,
                height_mm: 71.0,
            },
            "update save protection",
        ));
        app.request_replacement(PendingAction::RestartForUpdate);
        assert_eq!(app.pending_action, Some(PendingAction::RestartForUpdate));
        assert!(!app.execute_project_save(missing_parent.join("project.instplot"), false));
        assert_eq!(app.pending_action, Some(PendingAction::RestartForUpdate));
        assert!(app.edit_history.is_dirty(&app.document));
        assert!(!app.allow_close);
        assert!(!app.update.is_launching());
        assert!(!app.update.take_close_request());
        assert!(!missing_parent.exists());
    }

    #[test]
    fn restart_rechecks_drafts_and_conflicts_without_discarding_work() {
        let mut app = app();
        app.numeric_inputs.insert(
            "update-draft".into(),
            DeferredNumericInput {
                source_value: 2.0,
                text: "3.5".into(),
                error: None,
            },
        );
        // This is the action taken after saving or explicitly discarding the
        // project; neither choice is permission to discard editor drafts.
        app.perform_action(PendingAction::RestartForUpdate);
        assert_eq!(app.numeric_inputs["update-draft"].text, "3.5");
        assert!(!app.allow_close);
        assert!(!app.update.is_launching());
        assert!(!app.update.take_close_request());

        app.numeric_inputs.clear();
        app.pending_managed_save_conflict = Some(ManagedSaveConflict {
            project_path: PathBuf::from("conflicted-project.instplot"),
            explanation: "external data changed".into(),
        });
        app.perform_action(PendingAction::RestartForUpdate);
        assert!(app.pending_managed_save_conflict.is_some());
        assert!(!app.allow_close);
        assert!(!app.update.is_launching());
        assert!(!app.update.take_close_request());
    }
}
