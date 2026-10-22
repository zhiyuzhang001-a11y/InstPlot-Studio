use super::data_workflows::combined_column_fixture;
use super::*;
use instplot_studio::lite_edit::{LiteSourceUpdate, make_request};

#[test]
fn selecting_a_line_component_keeps_the_correct_dataset_for_lite_editing() {
    let (mut app, update) = fixture();
    let members = app.document.linked_series_ids(&update.artist_id);
    app.selected_series = members.into_iter().find(|id| id != &update.artist_id);
    assert!(app.selected_series.is_some());
    let group = data_navigation_groups(app.session.datasets(), UiLanguage::Chinese).remove(0);
    let context = egui::Context::default();
    let mut output = context.run_ui(egui::RawInput::default(), |ui| {
        app.column_controls_for_group(ui, &group)
    });
    output.textures_delta.clear();
    let selected = app.selected_series.as_ref().unwrap();
    let (request, _) = make_request(&app.document, selected, "a".repeat(64)).unwrap();
    assert_eq!(request.target_id, "source-3");
}

fn fixture() -> (StudioApp, LiteSourceUpdate) {
    let creation = eframe::CreationContext::_new_kittest(egui::Context::default());
    let mut app = StudioApp::new(&creation, Instant::now(), None);
    let (mut document, datasets) = combined_column_fixture();
    let ids = document
        .logical_series()
        .into_iter()
        .map(|s| s.id)
        .collect::<Vec<_>>();
    for id in &ids {
        document
            .set_series_style(id, SeriesCreationStyle::LineAndMarker)
            .unwrap();
    }
    for dimension in [AxisDimension::X, AxisDimension::Y] {
        let mut axis = document.axis_record(dimension);
        axis.autoscale = false;
        axis.minimum = -0.02;
        axis.maximum = 0.22;
        axis.locator = LocatorSpec::Interval { step: 0.05 };
        axis.minor_interval = Some(0.025);
        document.set_axis_record(dimension, axis).unwrap();
    }
    document
        .set_axis_label(
            AxisDimension::Y,
            vec![LabelNode::Text("My final label".to_owned())],
        )
        .unwrap();
    let (mut result, binding_before) = make_request(&document, &ids[3], "a".repeat(64)).unwrap();
    result.columns[1].values = vec![Some(0.01), Some(0.02), Some(0.03)];
    let update = LiteSourceUpdate {
        artist_id: ids[3].clone(),
        binding_before,
        result,
    };
    app.document = document;
    app.session.replace_datasets(datasets);
    app.edit_history = EditHistory::new(&app.document, true);
    (app, update)
}

#[test]
fn unchanged_lite_return_never_changes_manual_axes_or_adds_an_undo_step() {
    let (mut app, mut update) = fixture();
    update.result = make_request(&app.document, &update.artist_id, "a".repeat(64))
        .unwrap()
        .0;
    let before = app.document.clone();
    app.execute_document_edit(
        EditCommand::ApplyLiteData {
            update: Box::new(update),
            keep_manual_axes: false,
        },
        "Lite data",
    );
    assert_eq!(app.document, before);
    assert!(app.edit_history.undo(&mut app.document).is_none());
}

#[test]
fn lite_update_only_changes_target_data_preserves_manual_plot_and_has_atomic_session_undo_redo() {
    let (mut app, update) = fixture();
    let before = app.document.clone();
    assert!(app.execute_document_edit(
        EditCommand::ApplyLiteData {
            update: Box::new(update),
            keep_manual_axes: true
        },
        "Lite data"
    ));
    assert_eq!(app.document.project().figure, before.project().figure);
    assert_eq!(
        app.document.project().semantic_registry,
        before.project().semantic_registry
    );
    for old in &before.project().data_sources {
        if old.id != "source-3" {
            assert_eq!(
                app.document
                    .project()
                    .data_sources
                    .iter()
                    .find(|s| s.id == old.id),
                Some(old)
            );
        }
    }
    assert_eq!(
        app.session
            .datasets()
            .iter()
            .find(|s| s.plot_id == "source-3")
            .unwrap()
            .columns[1]
            .values,
        vec![0.01, 0.02, 0.03]
    );
    let after = app.document.clone();
    let outcome = ApplicationController::execute(
        AppTransactionState {
            document: &app.document,
            session: &app.session,
            workspace: &app.workspace,
            edit_history: &app.edit_history,
        },
        AppAction::Undo,
    )
    .unwrap();
    app.commit_app_outcome(outcome);
    assert_eq!(app.document, before);
    assert_eq!(
        app.session
            .datasets()
            .iter()
            .find(|s| s.plot_id == "source-3")
            .unwrap()
            .columns[1]
            .values,
        vec![203.0; 3]
    );
    let outcome = ApplicationController::execute(
        AppTransactionState {
            document: &app.document,
            session: &app.session,
            workspace: &app.workspace,
            edit_history: &app.edit_history,
        },
        AppAction::Redo,
    )
    .unwrap();
    app.commit_app_outcome(outcome);
    assert_eq!(app.document, after);
    let restored = FigureDocument::from_project(
        serde_json::from_slice(&serde_json::to_vec(after.project()).unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(restored.project(), after.project());
}

#[test]
fn lite_update_conflicts_and_missing_bound_columns_leave_original_document_and_history_untouched() {
    for failure in 0..3 {
        let (mut app, mut update) = fixture();
        match failure {
            0 => update.result.base_sha256 = "b".repeat(64),
            1 => update.binding_before.y_column = "change".to_owned(),
            _ => {
                update.result.columns.remove(1);
                update.result.y_column = "change".to_owned();
            }
        }
        let before = app.document.clone();
        assert!(!app.execute_document_edit(
            EditCommand::ApplyLiteData {
                update: Box::new(update),
                keep_manual_axes: true
            },
            "Lite data"
        ));
        assert_eq!(app.document, before);
        assert!(app.edit_history.undo(&mut app.document).is_none());
    }
}

#[test]
fn lite_update_can_select_a_derived_column_or_explicitly_refresh_manual_axes() {
    let (mut app, mut update) = fixture();
    update.result.y_column = "change".to_owned();
    let old_axis = app.document.axis_record(AxisDimension::Y);
    assert!(app.execute_document_edit(
        EditCommand::ApplyLiteData {
            update: Box::new(update),
            keep_manual_axes: true
        },
        "Lite data"
    ));
    assert_eq!(app.document.axis_record(AxisDimension::Y), old_axis);
    let bindings = app
        .document
        .logical_series()
        .into_iter()
        .map(|s| s.binding.unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        bindings.iter().filter(|b| b.y_column == "change").count(),
        1
    );
    let (mut app, update) = fixture();
    assert!(app.execute_document_edit(
        EditCommand::ApplyLiteData {
            update: Box::new(update),
            keep_manual_axes: false
        },
        "Lite data"
    ));
    assert!(app.document.axis_record(AxisDimension::Y).autoscale);
    assert!(app.document.axis_record(AxisDimension::Y).maximum > 200.0);
}
