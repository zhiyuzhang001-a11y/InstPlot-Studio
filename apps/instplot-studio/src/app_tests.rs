use super::*;
use instplot_export::ResolvedItem;
use instplot_render::{Color, DisplayItem, NodeId};

fn remove_app_data_sources(app: &mut StudioApp, ids: Vec<String>) {
    let state = AppTransactionState {
        document: &app.document,
        session: &app.session,
        workspace: &app.workspace,
        edit_history: &app.edit_history,
    };
    let outcome = ApplicationController::execute(state, AppAction::RemoveDataSources(ids)).unwrap();
    let effect = app.commit_app_outcome(outcome);
    assert!(matches!(effect, AppEffect::RemovedData));
}

#[test]
fn publication_product_copy_localizes_rules_without_internal_ids() {
    let finding = instplot_studio::PublicationFinding {
        rule_id: "color_only_encoding".to_owned(),
        severity: CheckSeverity::Warning,
        node_id: Some("handoff-artist-internal".to_owned()),
        message: "artists differ only by colour".to_owned(),
        impact: "readers may not distinguish them".to_owned(),
        remediation: "add markers".to_owned(),
        overridden: false,
        override_reason: None,
    };
    let copy = localized_publication_finding(UiLanguage::Chinese, &finding);
    assert_eq!(copy.0, "仅用颜色区分");
    assert!(
        copy.1
            .chars()
            .any(|character| ('\u{4e00}'..='\u{9fff}').contains(&character))
    );
    assert!(
        ![copy.0, copy.1, copy.2, copy.3]
            .join(" ")
            .contains(&finding.rule_id)
    );
}

#[test]
fn imported_data_navigation_groups_sections_by_file_without_copying_numeric_columns() {
    let fixture_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures");
    let mut session = StudioSession::default();
    session
        .import_data_file(&fixture_dir.join("lite-source-fit.txt"))
        .unwrap();
    session
        .import_data_file(&fixture_dir.join("smoke.csv"))
        .unwrap();
    let groups = data_navigation_groups(session.datasets(), UiLanguage::Chinese);
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].title, "lite-source-fit.txt");
    assert_eq!(groups[0].items.len(), 2);
    assert_eq!(groups[0].items[0].kind, DataSetKind::Source);
    assert_eq!(groups[0].items[1].kind, DataSetKind::Fit);
    assert_eq!(groups[1].title, "smoke.csv");
    assert_eq!(groups[1].items[0].kind, DataSetKind::Source);
}

#[test]
fn same_named_files_from_different_folders_have_distinct_group_titles() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/smoke.csv");
    let mut first = instplot_io::read_data_file(&fixture).unwrap().remove(0);
    let mut second = first.clone();
    first.source = PathBuf::from("/run-a/smoke.csv");
    second.source = PathBuf::from("/run-b/smoke.csv");
    second.plot_id = "second-run".to_owned();
    let groups = data_navigation_groups(&[first, second], UiLanguage::Chinese);
    assert_eq!(groups[0].title, "smoke.csv · run-a");
    assert_eq!(groups[1].title, "smoke.csv · run-b");
}

#[test]
fn imported_file_groups_survive_project_round_trip_and_remove_as_one_undo_step() {
    let fixture_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures");
    let mut session = StudioSession::default();
    session
        .import_data_file(&fixture_dir.join("lite-source-fit.txt"))
        .unwrap();
    session
        .import_data_file(&fixture_dir.join("smoke.csv"))
        .unwrap();
    let mut document = FigureDocument::from_datasets(session.datasets()).unwrap();
    let edited_artist = document
        .project()
        .figure
        .artists
        .iter()
        .find(|artist| artist.kind == instplot_studio::ArtistKind::Line)
        .cloned()
        .unwrap();
    document.set_artist_record(edited_artist).unwrap();
    let encoded = serde_json::to_vec(document.project()).unwrap();
    document = FigureDocument::from_project(serde_json::from_slice(&encoded).unwrap()).unwrap();
    let (restored, warnings) = StudioSession::from_project(document.project());
    assert!(warnings.is_empty());
    let groups = data_navigation_groups(restored.datasets(), UiLanguage::Chinese);
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].title, "lite-source-fit.txt");
    assert_eq!(groups[0].items.len(), 2);
    let ids = groups[0]
        .items
        .iter()
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();
    let before = document.clone();
    let mut history = EditHistory::new(&document, true);
    history
        .execute(
            &mut document,
            EditCommand::DeleteDataSources {
                data_source_ids: ids,
            },
            None,
        )
        .unwrap();
    assert_eq!(document.project().data_sources.len(), 1);
    assert_eq!(document.project().data_sources[0].label, "smoke.csv");
    document.project().validate().unwrap();
    assert!(document.project().overrides.iter().all(|record| {
        document
            .project()
            .figure
            .artists
            .iter()
            .any(|artist| artist.id == record.target_id)
    }));
    history.undo(&mut document).unwrap();
    assert_eq!(document, before);
}

#[test]
fn clearing_all_data_is_undoable_and_leaves_a_valid_empty_figure() {
    let fixture_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures");
    let mut session = StudioSession::default();
    session
        .import_data_file(&fixture_dir.join("lite-source-fit.txt"))
        .unwrap();
    let mut document = FigureDocument::from_datasets(session.datasets()).unwrap();
    let edited_artists = document
        .project()
        .figure
        .artists
        .iter()
        .filter(|artist| {
            matches!(
                artist.kind,
                instplot_studio::ArtistKind::Line
                    | instplot_studio::ArtistKind::Scatter
                    | instplot_studio::ArtistKind::Legend
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    for artist in edited_artists {
        document.set_artist_record(artist).unwrap();
    }
    let before = document.clone();
    let ids = session
        .datasets()
        .iter()
        .map(|dataset| dataset.plot_id.clone())
        .collect();
    let mut history = EditHistory::new(&document, true);
    history
        .execute(
            &mut document,
            EditCommand::DeleteDataSources {
                data_source_ids: ids,
            },
            None,
        )
        .unwrap();
    assert!(document.project().data_sources.is_empty());
    assert!(
        StudioSession::from_project(document.project())
            .0
            .datasets()
            .is_empty()
    );
    document.project().validate().unwrap();
    assert!(document.project().overrides.is_empty());
    history.undo(&mut document).unwrap();
    assert_eq!(document, before);
}

#[test]
fn importing_after_clear_all_rebuilds_visible_series_and_autoscales() {
    let creation = eframe::CreationContext::_new_kittest(egui::Context::default());
    let mut app = StudioApp::new(&creation, Instant::now(), None);
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("numeric-column-headers.csv");
    app.load_data_paths(vec![path.clone()]);
    app.document
        .set_axis_label(
            AxisDimension::X,
            vec![LabelNode::Text("Old X label".to_owned())],
        )
        .unwrap();
    app.document
        .set_axis_label(
            AxisDimension::Y,
            vec![LabelNode::Text("Old Y label".to_owned())],
        )
        .unwrap();
    let ids = app
        .document
        .project()
        .data_sources
        .iter()
        .map(|source| source.id.clone())
        .collect();
    remove_app_data_sources(&mut app, ids);
    assert!(app.session.datasets().is_empty());

    app.load_data_paths(vec![path]);

    assert_eq!(app.session.dataset_count(), 1);
    assert_eq!(app.document.project().data_sources.len(), 1);
    assert_eq!(
        app.document.axis_label(AxisDimension::X),
        &[LabelNode::Text("Diameter (nm)".to_owned())]
    );
    assert_eq!(
        app.document.axis_label(AxisDimension::Y),
        &[LabelNode::Text("0.00083".to_owned())]
    );
    assert_eq!(
        app.document
            .series()
            .iter()
            .filter(|series| series.binding.is_some())
            .count(),
        1
    );
    assert!(app.messages.iter().all(|message| message.code != "import"));
    let x_axis = app.document.axis_record(AxisDimension::X);
    let y_axis = app.document.axis_record(AxisDimension::Y);
    assert!(x_axis.autoscale, "reimport must restore X autoscale");
    assert!(y_axis.autoscale, "reimport must restore Y autoscale");
    assert!(
        x_axis.minimum <= 0.1 && x_axis.maximum >= 0.3,
        "reimported X data must fit inside the visible range: {x_axis:?}"
    );
    assert!(
        y_axis.minimum <= 1.1 && y_axis.maximum >= 1.3,
        "reimported Y data must fit inside the visible range: {y_axis:?}"
    );

    let ids = app
        .document
        .project()
        .data_sources
        .iter()
        .map(|source| source.id.clone())
        .collect();
    remove_app_data_sources(&mut app, ids);
    let fixture_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures");
    app.load_data_paths(vec![
        fixture_dir.join("numeric-column-headers.csv"),
        fixture_dir.join("smoke.csv"),
    ]);
    assert_eq!(app.session.dataset_count(), 2);
    assert_eq!(app.document.project().data_sources.len(), 2);
    assert_eq!(
        app.document
            .series()
            .iter()
            .filter(|series| series.binding.is_some())
            .count(),
        2
    );
}

#[test]
fn application_controller_rejects_failed_import_without_mutating_live_state() {
    let document = FigureDocument::showcase();
    let (session, _) = StudioSession::from_project(document.project());
    let workspace = WorkspaceState::new("Untitled");
    let history = EditHistory::new(&document, true);
    let before_document = document.clone();
    let before_workspace = workspace.clone();
    let before_count = session.dataset_count();

    let result = ApplicationController::execute(
        AppTransactionState {
            document: &document,
            session: &session,
            workspace: &workspace,
            edit_history: &history,
        },
        AppAction::ImportFiles {
            paths: vec![PathBuf::from("/definitely-missing/instplot-phase2.csv")],
            preferred_columns: None,
        },
    );

    assert!(result.is_err());
    assert_eq!(document, before_document);
    assert_eq!(workspace, before_workspace);
    assert_eq!(session.dataset_count(), before_count);
}

#[test]
fn reusable_shell_svg_event_reaches_the_atomic_export_transaction() {
    let document = FigureDocument::fixed();
    let (session, _) = StudioSession::from_project(document.project());
    let workspace = WorkspaceState::new("Untitled");
    let history = EditHistory::new(&document, true);
    // Rust test names contain `::`, which is not a valid Windows filename.
    // The process id is sufficient because this test writes only once per test binary.
    let path = std::env::temp_dir().join(format!("instplot-shell-svg-{}.svg", std::process::id()));
    let outcome = ApplicationController::execute(
        AppTransactionState {
            document: &document,
            session: &session,
            workspace: &workspace,
            edit_history: &history,
        },
        AppAction::ExportFigure {
            path: path.clone(),
            format: FigureExport::Svg,
        },
    )
    .unwrap();
    assert!(matches!(outcome.effect, AppEffect::Exported { .. }));
    assert!(std::fs::read_to_string(&path).unwrap().starts_with("<svg "));
    std::fs::remove_file(path).unwrap();
}

#[test]
fn application_controller_removal_commits_document_session_and_undo_together() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/smoke.csv");
    let mut session = StudioSession::default();
    session.import_data_file(&fixture).unwrap();
    let document = FigureDocument::from_datasets(session.datasets()).unwrap();
    let before = document.clone();
    let workspace = WorkspaceState::from_lite("Untitled");
    let history = EditHistory::new(&document, true);
    let ids = document
        .project()
        .data_sources
        .iter()
        .map(|source| source.id.clone())
        .collect();

    let mut outcome = ApplicationController::execute(
        AppTransactionState {
            document: &document,
            session: &session,
            workspace: &workspace,
            edit_history: &history,
        },
        AppAction::RemoveDataSources(ids),
    )
    .unwrap();

    assert!(outcome.document.project().data_sources.is_empty());
    assert!(outcome.session.datasets().is_empty());
    outcome.edit_history.undo(&mut outcome.document).unwrap();
    assert_eq!(outcome.document, before);
}

#[test]
fn application_controller_save_and_open_share_one_state_transition() {
    use std::time::{SystemTime, UNIX_EPOCH};

    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/smoke.csv");
    let mut session = StudioSession::default();
    session.import_data_file(&fixture).unwrap();
    let document = FigureDocument::from_datasets(session.datasets()).unwrap();
    let workspace = WorkspaceState::from_lite("Untitled");
    let history = EditHistory::new(&document, false);
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "instplot-phase2-{}-{nonce}.instplot",
        std::process::id()
    ));

    let saved = ApplicationController::execute(
        AppTransactionState {
            document: &document,
            session: &session,
            workspace: &workspace,
            edit_history: &history,
        },
        AppAction::SaveProject(path.clone()),
    )
    .unwrap();
    assert_eq!(saved.workspace.project_path(), Some(path.as_path()));
    assert!(!saved.edit_history.is_dirty(&saved.document));

    let opened = ApplicationController::execute(
        AppTransactionState {
            document: &FigureDocument::showcase(),
            session: &StudioSession::default(),
            workspace: &WorkspaceState::new("Untitled"),
            edit_history: &EditHistory::new(&FigureDocument::showcase(), true),
        },
        AppAction::OpenProject(path.clone()),
    )
    .unwrap();
    assert_eq!(opened.document, document);
    assert_eq!(opened.session.dataset_count(), session.dataset_count());
    assert_eq!(opened.workspace.project_path(), Some(path.as_path()));
    assert!(!opened.edit_history.is_dirty(&opened.document));

    std::fs::remove_file(path).unwrap();
}

#[test]
fn application_controller_undo_and_redo_keep_derived_state_in_sync() {
    let mut document = FigureDocument::showcase();
    let original = document.clone();
    let (session, _) = StudioSession::from_project(document.project());
    let workspace = WorkspaceState::new("Untitled");
    let mut history = EditHistory::new(&document, true);
    history
        .execute(
            &mut document,
            EditCommand::SetAxisLabel {
                dimension: AxisDimension::X,
                nodes: vec![LabelNode::Text("changed".to_owned())],
            },
            None,
        )
        .unwrap();
    let edited = document.clone();

    let undone = ApplicationController::execute(
        AppTransactionState {
            document: &document,
            session: &session,
            workspace: &workspace,
            edit_history: &history,
        },
        AppAction::Undo,
    )
    .unwrap();
    assert_eq!(undone.document, original);
    assert_eq!(
        undone.session.dataset_count(),
        undone.document.project().data_sources.len()
    );

    let redone = ApplicationController::execute(
        AppTransactionState {
            document: &undone.document,
            session: &undone.session,
            workspace: &undone.workspace,
            edit_history: &undone.edit_history,
        },
        AppAction::Redo,
    )
    .unwrap();
    assert_eq!(redone.document, edited);
    assert_eq!(
        redone.session.dataset_count(),
        redone.document.project().data_sources.len()
    );
}

#[test]
fn deleting_the_only_file_then_reimporting_it_fits_every_value() {
    let creation = eframe::CreationContext::_new_kittest(egui::Context::default());
    let mut app = StudioApp::new(&creation, Instant::now(), None);
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("smoke.csv");
    app.load_data_paths(vec![path.clone()]);

    let ids = app
        .document
        .project()
        .data_sources
        .iter()
        .map(|source| source.id.clone())
        .collect::<Vec<_>>();
    remove_app_data_sources(&mut app, ids);
    assert_eq!(app.session.dataset_count(), 0);

    app.load_data_paths(vec![path]);

    let x_axis = app.document.axis_record(AxisDimension::X);
    let y_axis = app.document.axis_record(AxisDimension::Y);
    assert!(x_axis.autoscale && y_axis.autoscale);
    assert!(x_axis.minimum < 1.0 && x_axis.maximum > 3.0, "{x_axis:?}");
    assert!(y_axis.minimum < 2.0 && y_axis.maximum > 8.0, "{y_axis:?}");
    assert_eq!(app.session.dataset_count(), 1);
    assert_eq!(
        app.document
            .series()
            .iter()
            .filter(|series| series.binding.is_some())
            .count(),
        1
    );
}

#[test]
fn data_drawer_renders_both_demo_and_imported_states() {
    let context = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(context.clone());
    let mut app = StudioApp::new(&creation, Instant::now(), None);
    let mut output = context.run_ui(egui::RawInput::default(), |ui| app.data_source_controls(ui));
    output.textures_delta.clear();
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("lite-source-fit.txt");
    app.load_data_paths(vec![path]);
    let mut output = context.run_ui(egui::RawInput::default(), |ui| app.data_source_controls(ui));
    output.textures_delta.clear();
    assert_eq!(app.session.dataset_count(), 2);
    assert!(app.selected_dataset.is_some());
}

#[test]
fn file_sidebar_keeps_xy_controls_inline() {
    let context = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(context.clone());
    let mut app = StudioApp::new(&creation, Instant::now(), None);
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("lite-source-fit.txt");
    app.load_data_paths(vec![path]);
    app.selected_dataset = None;
    let mut output = context.run_ui(egui::RawInput::default(), |ui| app.data_source_controls(ui));
    output.textures_delta.clear();
    assert_eq!(app.session.dataset_count(), 2);
}

#[test]
fn numeric_header_file_imports_and_renders_inline_xy_editor() {
    let context = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(context.clone());
    let mut app = StudioApp::new(&creation, Instant::now(), None);
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("numeric-column-headers.csv");
    app.load_data_paths(vec![path]);
    assert_eq!(app.session.dataset_count(), 1);
    assert_eq!(app.binding_x, "Diameter (nm)");
    assert_eq!(app.binding_y, "0.00083");
    let mut output = context.run_ui(egui::RawInput::default(), |ui| app.data_source_controls(ui));
    output.textures_delta.clear();
    assert_eq!(
        app.document
            .series()
            .iter()
            .filter(|series| series.binding.is_some())
            .count(),
        1
    );
    assert_eq!(
        app.document.axis_label(AxisDimension::X),
        &[LabelNode::Text("Diameter (nm)".to_owned())]
    );
    assert_eq!(
        app.document.axis_label(AxisDimension::Y),
        &[LabelNode::Text("0.00083".to_owned())]
    );
}

#[test]
fn app_import_replaces_edited_demo_then_adds_later_files() {
    let creation = eframe::CreationContext::_new_kittest(egui::Context::default());
    let mut app = StudioApp::new(&creation, Instant::now(), None);
    app.edit_history
        .execute(
            &mut app.document,
            EditCommand::SetFigureSize {
                width_mm: 89.0,
                height_mm: 65.0,
            },
            None,
        )
        .unwrap();
    assert!(app.edit_history.is_dirty(&app.document));
    app.selected_canvas_node = Some("showcase-artist".to_owned());
    let first = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("smoke.csv");
    app.load_data_paths(vec![first.clone()]);
    assert!(!app.workspace.should_replace_showcase_on_import());
    assert_eq!(app.session.dataset_count(), 1);
    assert_eq!(app.document.project().data_sources.len(), 1);
    assert_eq!(
        app.document
            .series()
            .iter()
            .filter(|series| series.binding.is_some())
            .count(),
        1
    );
    assert_eq!(app.selected_canvas_node, None);
    assert!(app.selected_dataset.is_some());
    assert!(app.document.axis_record(AxisDimension::X).autoscale);

    let second = std::env::temp_dir().join(format!(
        "instplot-studio-second-import-{}.csv",
        std::process::id()
    ));
    std::fs::copy(first, &second).unwrap();
    app.load_data_paths(vec![second.clone()]);
    std::fs::remove_file(second).unwrap();
    assert_eq!(app.session.dataset_count(), 2);
    assert_eq!(app.document.project().data_sources.len(), 2);
    assert_eq!(
        app.document
            .series()
            .iter()
            .filter(|series| series.binding.is_some())
            .count(),
        2
    );
}

#[test]
fn manual_repeated_measurements_replace_demo_and_create_mean_error_series() {
    let creation = eframe::CreationContext::_new_kittest(egui::Context::default());
    let mut app = StudioApp::new(&creation, Instant::now(), None);
    app.manual_data.input = ManualDataInput {
        groups: vec![ManualDataGroupInput {
            group_id: "manual-repeated".to_owned(),
            source_name: "Repeated trial".to_owned(),
            x: ManualAxisInput {
                name: "Field".to_owned(),
                measurements: vec!["0 1".to_owned()],
            },
            y: ManualAxisInput {
                name: "Signal".to_owned(),
                measurements: vec!["1 2".to_owned(), "2 4".to_owned(), "3 6".to_owned()],
            },
            error_statistic: ErrorStatistic::StandardDeviation,
            plot_style: ManualPlotStyle::LineAndMarker,
        }],
    };

    app.insert_manual_data().unwrap();

    assert_eq!(app.session.dataset_count(), 1);
    assert_eq!(app.document.project().data_sources.len(), 1);
    assert_eq!(
        app.document
            .series()
            .iter()
            .filter(|series| series.kind == SeriesKind::Line)
            .count(),
        1
    );
    assert_eq!(
        app.document
            .series()
            .iter()
            .filter(|series| series.kind == SeriesKind::Scatter)
            .count(),
        1
    );
    assert_eq!(
        app.document
            .series()
            .iter()
            .filter(|series| series.kind == SeriesKind::ErrorBar)
            .count(),
        1
    );
    assert_eq!(
        app.document.axis_label(AxisDimension::X),
        &[LabelNode::Text("Field".to_owned())]
    );
    assert_eq!(
        app.document.axis_label(AxisDimension::Y),
        &[LabelNode::Text("Signal".to_owned())]
    );
    assert!(app.resolved.layout.result.legend.is_some());

    let project_path = std::env::temp_dir().join(format!(
        "instplot-manual-data-round-trip-{}.instplot",
        std::process::id()
    ));
    app.document.save(&project_path).unwrap();
    let (reopened, _) = FigureDocument::open(&project_path).unwrap();
    std::fs::remove_file(&project_path).unwrap();
    let instplot_studio::DataSourcePayload::Embedded { columns, .. } =
        &reopened.project().data_sources[0].payload
    else {
        panic!("manual data must remain embedded in the saved project")
    };
    assert_eq!(
        reopened.project().data_sources[0].origin,
        instplot_studio::DataSourceOrigin::Manual
    );
    let recipe = reopened.project().data_sources[0]
        .manual_recipe
        .as_ref()
        .expect("manual source must preserve its editable recipe");
    assert_eq!(recipe.x.measurements.len(), 1);
    assert_eq!(recipe.y.measurements.len(), 3);
    for expected in [
        "Signal",
        "Signal · SD",
        "Signal · 测量 1",
        "Signal · 测量 2",
        "Signal · 测量 3",
    ] {
        assert!(
            columns.iter().any(|column| column.name == expected),
            "saved project is missing {expected}"
        );
    }
    assert_eq!(
        reopened
            .series()
            .iter()
            .filter(|series| series.kind == SeriesKind::ErrorBar)
            .count(),
        1
    );

    let error_id = app
        .document
        .project()
        .figure
        .artists
        .iter()
        .find(|artist| artist.kind == instplot_studio::ArtistKind::ErrorBar)
        .map(|artist| artist.id.clone())
        .unwrap();
    let mut error_record = app.document.artist_record(&error_id).unwrap();
    let ArtistProperties::ErrorBar {
        cap_width_pt,
        stroke,
        ..
    } = &mut error_record.properties
    else {
        unreachable!()
    };
    *cap_width_pt = 6.5;
    stroke.width_pt = 1.25;
    assert!(app.execute_document_edit(EditCommand::SetArtistRecord(error_record), "编辑误差棒",));
    let edited_error = app.document.artist_record(&error_id).unwrap();
    let ArtistProperties::ErrorBar {
        cap_width_pt,
        stroke,
        ..
    } = edited_error.properties
    else {
        unreachable!()
    };
    assert_eq!(cap_width_pt, 6.5);
    assert_eq!(stroke.width_pt, 1.25);

    let groups = data_navigation_groups(app.session.datasets(), UiLanguage::Chinese);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].title, "Repeated trial");
    assert_eq!(groups[0].items.len(), 1);

    let ids = app
        .document
        .project()
        .data_sources
        .iter()
        .map(|source| source.id.clone())
        .collect::<Vec<_>>();
    remove_app_data_sources(&mut app, ids);
    assert!(app.session.datasets().is_empty());
    app.manual_data.input.groups[0].source_name = "Repeated trial after clear".to_owned();
    app.manual_data.input.groups[0].group_id = "manual-repeated-after-clear".to_owned();

    app.insert_manual_data().unwrap();

    assert_eq!(app.session.dataset_count(), 1);
    assert_eq!(
        app.document
            .series()
            .iter()
            .filter(|series| series.kind == SeriesKind::ErrorBar)
            .count(),
        1
    );
    let groups = data_navigation_groups(app.session.datasets(), UiLanguage::Chinese);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].title, "Repeated trial after clear");
}

#[test]
fn manual_multiple_xy_groups_create_distinct_series_and_automatic_errors() {
    let creation = eframe::CreationContext::_new_kittest(egui::Context::default());
    let mut app = StudioApp::new(&creation, Instant::now(), None);
    app.manual_data.input = ManualDataInput {
        groups: vec![
            ManualDataGroupInput {
                group_id: "manual-experiment-a".to_owned(),
                source_name: "Experiment A".to_owned(),
                x: ManualAxisInput {
                    name: "Field A".to_owned(),
                    measurements: vec!["0 1".to_owned()],
                },
                y: ManualAxisInput {
                    name: "Signal A".to_owned(),
                    measurements: vec!["2 4".to_owned(), "4 8".to_owned()],
                },
                error_statistic: ErrorStatistic::StandardDeviation,
                plot_style: ManualPlotStyle::LineAndMarker,
            },
            ManualDataGroupInput {
                group_id: "manual-experiment-b".to_owned(),
                source_name: "Experiment B".to_owned(),
                x: ManualAxisInput {
                    name: "Field B".to_owned(),
                    measurements: vec!["10 20 30".to_owned()],
                },
                y: ManualAxisInput {
                    name: "Signal B".to_owned(),
                    measurements: vec!["5 6 7".to_owned()],
                },
                error_statistic: ErrorStatistic::StandardDeviation,
                plot_style: ManualPlotStyle::LineAndMarker,
            },
        ],
    };

    app.insert_manual_data().unwrap();

    assert_eq!(
        app.document
            .series()
            .iter()
            .filter(|series| series.kind == SeriesKind::Line)
            .count(),
        2
    );
    assert_eq!(
        app.document
            .series()
            .iter()
            .filter(|series| series.kind == SeriesKind::Scatter)
            .count(),
        2
    );
    assert_eq!(
        app.document
            .series()
            .iter()
            .filter(|series| series.kind == SeriesKind::ErrorBar)
            .count(),
        1
    );
    let marker_styles = app
        .document
        .project()
        .figure
        .artists
        .iter()
        .filter_map(|artist| match &artist.properties {
            ArtistProperties::Scatter { marker, .. } => {
                Some((marker.color_id.clone(), marker.shape))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(marker_styles.len(), 2);
    assert_ne!(marker_styles[0], marker_styles[1]);

    for palette_id in ["tol-bright-v1", "okabe-ito-v1"] {
        app.document.set_palette(palette_id).unwrap();
        let mut colors_by_y = BTreeMap::<String, BTreeSet<String>>::new();
        for artist in &app.document.project().figure.artists {
            let (binding, color) = match &artist.properties {
                ArtistProperties::Line { binding, stroke }
                | ArtistProperties::ErrorBar {
                    binding, stroke, ..
                } => (binding, &stroke.color_id),
                ArtistProperties::Scatter { binding, marker } => (binding, &marker.color_id),
                _ => continue,
            };
            colors_by_y
                .entry(binding.y_column.clone())
                .or_default()
                .insert(color.clone());
        }
        assert_eq!(colors_by_y.len(), 2);
        assert!(colors_by_y.values().all(|colors| colors.len() == 1));
        assert_ne!(colors_by_y["Signal A"], colors_by_y["Signal B"]);
    }
}

#[test]
fn saved_manual_group_reopens_read_only_and_updates_in_place() {
    let creation = eframe::CreationContext::_new_kittest(egui::Context::default());
    let mut app = StudioApp::new(&creation, Instant::now(), None);
    app.manual_data.input = ManualDataInput {
        groups: vec![ManualDataGroupInput {
            group_id: "manual-editable".to_owned(),
            source_name: "Editable".to_owned(),
            x: ManualAxisInput {
                name: "Field".to_owned(),
                measurements: vec!["0 1".to_owned()],
            },
            y: ManualAxisInput {
                name: "Signal".to_owned(),
                measurements: vec!["2 4".to_owned()],
            },
            error_statistic: ErrorStatistic::StandardDeviation,
            plot_style: ManualPlotStyle::LineAndMarker,
        }],
    };
    app.insert_manual_data().unwrap();
    assert_eq!(
        data_navigation_groups(app.session.datasets(), UiLanguage::Chinese)[0].title,
        "Editable"
    );
    let original_styles = app
        .document
        .project()
        .figure
        .artists
        .iter()
        .filter(|artist| {
            matches!(
                artist.properties,
                ArtistProperties::Line { .. } | ArtistProperties::Scatter { .. }
            )
        })
        .map(|artist| artist.id.clone())
        .collect::<BTreeSet<_>>();

    app.prepare_manual_data_window();
    assert_eq!(app.manual_data.input.groups.len(), 1);
    assert!(app.manual_data.editing_group_id.is_none());
    app.manual_data.editing_group_id = Some("manual-editable".to_owned());
    app.manual_data.input.groups[0].source_name = "Edited".to_owned();
    app.manual_data.input.groups[0].y.name = "Response".to_owned();
    app.manual_data.input.groups[0].y.measurements[0] = "3 9".to_owned();
    app.insert_manual_data().unwrap();

    assert_eq!(app.document.project().data_sources.len(), 1);
    assert_eq!(app.session.dataset_count(), 1);
    let source = &app.document.project().data_sources[0];
    assert_eq!(source.label, "Edited");
    assert_eq!(source.manual_recipe.as_ref().unwrap().y.name, "Response");
    let updated_styles = app
        .document
        .project()
        .figure
        .artists
        .iter()
        .filter(|artist| {
            matches!(
                artist.properties,
                ArtistProperties::Line { .. } | ArtistProperties::Scatter { .. }
            )
        })
        .map(|artist| artist.id.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(updated_styles, original_styles);
    let (reopened_session, warnings) = StudioSession::from_project(app.document.project());
    assert!(warnings.is_empty());
    assert_eq!(
        data_navigation_groups(reopened_session.datasets(), UiLanguage::Chinese)[0].title,
        "Edited"
    );
}

#[test]
fn opening_saved_manual_project_restores_data_sidebar() {
    let creation = eframe::CreationContext::_new_kittest(egui::Context::default());
    let mut app = StudioApp::new(&creation, Instant::now(), None);
    app.manual_data.input = ManualDataInput {
        groups: vec![ManualDataGroupInput {
            group_id: "manual-sidebar".to_owned(),
            source_name: "Repeat QA".to_owned(),
            x: ManualAxisInput {
                name: "Field".to_owned(),
                measurements: vec!["0 1".to_owned()],
            },
            y: ManualAxisInput {
                name: "Signal".to_owned(),
                measurements: vec!["2 4".to_owned()],
            },
            error_statistic: ErrorStatistic::StandardDeviation,
            plot_style: ManualPlotStyle::LineAndMarker,
        }],
    };
    app.insert_manual_data().unwrap();

    let project_path = std::env::temp_dir().join(format!(
        "instplot-manual-sidebar-round-trip-{}.instplot",
        std::process::id()
    ));
    app.document.save(&project_path).unwrap();
    let reopened_creation = eframe::CreationContext::_new_kittest(egui::Context::default());
    let mut reopened = StudioApp::new(&reopened_creation, Instant::now(), None);
    reopened.show_layers = false;
    reopened.open_paths(vec![project_path.clone()]);
    std::fs::remove_file(&project_path).unwrap();

    assert!(reopened.show_layers);
    assert_eq!(
        reopened.workspace.project_path(),
        Some(project_path.as_path())
    );
    assert_eq!(
        data_navigation_groups(reopened.session.datasets(), UiLanguage::Chinese)[0].title,
        "Repeat QA"
    );
}

#[test]
fn managed_manual_data_round_trips_all_supported_formats() {
    let creation = eframe::CreationContext::_new_kittest(egui::Context::default());
    let mut app = StudioApp::new(&creation, Instant::now(), None);
    app.manual_data.input = ManualDataInput {
        groups: vec![ManualDataGroupInput {
            group_id: "manual-managed".to_owned(),
            source_name: "Managed Trial".to_owned(),
            x: ManualAxisInput {
                name: "Field".to_owned(),
                measurements: vec!["0 1 2".to_owned()],
            },
            y: ManualAxisInput {
                name: "Signal".to_owned(),
                measurements: vec!["2 4 6".to_owned(), "4 6 8".to_owned()],
            },
            error_statistic: ErrorStatistic::StandardDeviation,
            plot_style: ManualPlotStyle::LineAndMarker,
        }],
    };
    app.insert_manual_data().unwrap();
    let directory =
        std::env::temp_dir().join(format!("instplot-managed-formats-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    for format in [
        ManagedDataFormat::Csv,
        ManagedDataFormat::Tsv,
        ManagedDataFormat::Txt,
        ManagedDataFormat::Dat,
        ManagedDataFormat::Xlsx,
    ] {
        let format_directory = directory.join(format.extension());
        let mut document = app.document.clone();
        document
            .configure_manual_data_files(&format_directory, format)
            .unwrap();
        let project_path = format_directory.join("figure.instplot");
        let saved = document
            .save_with_managed_manual_data(&project_path)
            .unwrap();
        let managed = saved.project().data_sources[0]
            .managed_file
            .as_ref()
            .unwrap();
        assert_eq!(managed.format, format);
        assert!(managed.fingerprint.is_some());
        let data_path = PathBuf::from(&managed.path);
        assert!(data_path.exists());
        let imported = DataImporter::read_file(&data_path).unwrap();
        assert!(!imported.is_empty());
        assert!(
            imported[0]
                .columns
                .iter()
                .any(|column| column.name == "Signal")
        );
        let export_directory = format_directory.join("export-copy");
        assert_eq!(
            saved
                .export_manual_data_files(&export_directory, format)
                .unwrap(),
            1
        );
        assert_eq!(
            saved.project().data_sources[0]
                .managed_file
                .as_ref()
                .unwrap()
                .path,
            managed.path,
            "exporting a copy must not change the managed save association"
        );
        let mut edited = saved.clone();
        let source = &saved.project().data_sources[0];
        edited
            .set_manual_source_metadata(
                &source.id,
                source.manual_recipe.clone().expect("manual recipe"),
            )
            .unwrap();
        assert_eq!(
            edited.project().data_sources[0].managed_file,
            source.managed_file,
            "editing manual values must keep their managed file association"
        );
        let mut removed = saved.clone();
        removed
            .delete_data_source(&source.id, true)
            .expect("removing a manual card should remove only the project object");
        assert!(
            data_path.exists(),
            "removing a manual card must not delete its managed disk file"
        );
        if format == ManagedDataFormat::Csv {
            saved
                .save_with_managed_manual_data(&project_path)
                .expect("an unchanged managed file should update without prompting");
            std::fs::write(&data_path, b"externally changed\n").unwrap();
            let error = saved
                .save_with_managed_manual_data(&project_path)
                .unwrap_err();
            assert!(error.to_string().contains("changed outside Studio"));
            let overwritten = saved
                .overwrite_managed_manual_data(&project_path)
                .expect("explicit overwrite should restore the managed data file");
            assert_eq!(DataImporter::read_file(&data_path).unwrap()[0].row_count, 3);
            assert!(
                overwritten.project().data_sources[0]
                    .managed_file
                    .as_ref()
                    .unwrap()
                    .fingerprint
                    .is_some()
            );
        }
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn external_manual_file_change_opens_an_explicit_save_conflict() {
    let creation = eframe::CreationContext::_new_kittest(egui::Context::default());
    let mut app = StudioApp::new(&creation, Instant::now(), None);
    app.manual_data.input = ManualDataInput {
        groups: vec![ManualDataGroupInput::new(1)],
    };
    app.manual_data.input.groups[0].x.measurements[0] = "0 1".to_owned();
    app.manual_data.input.groups[0].y.measurements[0] = "2 3".to_owned();
    app.insert_manual_data().unwrap();
    let directory =
        std::env::temp_dir().join(format!("instplot-save-conflict-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    app.document
        .configure_manual_data_files(&directory, ManagedDataFormat::Csv)
        .unwrap();
    let project_path = directory.join("figure.instplot");
    assert!(app.execute_project_save(project_path.clone(), false));
    let managed_path = PathBuf::from(
        &app.document.project().data_sources[0]
            .managed_file
            .as_ref()
            .unwrap()
            .path,
    );
    std::fs::write(&managed_path, b"external change\n").unwrap();

    assert!(!app.execute_project_save(project_path.clone(), false));
    let conflict = app
        .pending_managed_save_conflict
        .as_ref()
        .expect("a visible choice dialog should be pending");
    assert_eq!(conflict.project_path, project_path);
    assert!(conflict.explanation.contains("外部"));
    let conflict_path = conflict.project_path.clone();
    assert!(app.execute_project_save(conflict_path, true));
    assert_eq!(
        DataImporter::read_file(&managed_path).unwrap()[0].row_count,
        2
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn saving_manual_data_never_rewrites_an_imported_source_file() {
    let creation = eframe::CreationContext::_new_kittest(egui::Context::default());
    let mut app = StudioApp::new(&creation, Instant::now(), None);
    let imported_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/smoke.csv");
    let original_bytes = std::fs::read(&imported_path).unwrap();
    app.load_data_paths(vec![imported_path.clone()]);
    app.manual_data.input = ManualDataInput {
        groups: vec![ManualDataGroupInput::new(1)],
    };
    app.manual_data.input.groups[0].source_name = "Independent manual data".to_owned();
    app.manual_data.input.groups[0].x.measurements[0] = "0 1 2".to_owned();
    app.manual_data.input.groups[0].y.measurements[0] = "3 4 5".to_owned();
    app.insert_manual_data().unwrap();

    let directory =
        std::env::temp_dir().join(format!("instplot-import-readonly-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    app.document
        .configure_manual_data_files(&directory, ManagedDataFormat::Csv)
        .unwrap();
    let saved = app
        .document
        .save_with_managed_manual_data(&directory.join("figure.instplot"))
        .unwrap();

    assert_eq!(std::fs::read(&imported_path).unwrap(), original_bytes);
    let imported_source = saved
        .project()
        .data_sources
        .iter()
        .find(|source| source.origin == instplot_studio::DataSourceOrigin::Imported)
        .unwrap();
    assert!(imported_source.managed_file.is_none());
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn batch_import_accepts_valid_data_and_reports_the_bad_file() {
    let creation = eframe::CreationContext::_new_kittest(egui::Context::default());
    let mut app = StudioApp::new(&creation, Instant::now(), None);
    let mut imported = StudioSession::default();
    imported
        .import_data_file(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests")
                .join("fixtures")
                .join("lite-source-fit.txt"),
        )
        .unwrap();
    let mut orphan_fit = imported.datasets()[1].clone();
    orphan_fit.fit_link.as_mut().unwrap().parent_dataset_id = Some("missing".to_owned());
    let bad = std::env::temp_dir().join(format!(
        "instplot-studio-orphan-fit-{}.csv",
        std::process::id()
    ));
    instplot_io::save_text_combined(
        &bad,
        &[&orphan_fit],
        instplot_io::TextExportFormat::Csv,
        &[],
    )
    .unwrap();
    let good = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("smoke.csv");
    app.load_data_paths(vec![bad.clone(), good]);
    std::fs::remove_file(bad).unwrap();
    assert_eq!(app.session.dataset_count(), 1);
    assert_eq!(app.document.project().data_sources.len(), 1);
    assert!(
        app.messages
            .iter()
            .any(|message| message.code == "import-partial")
    );
    assert!(
        app.messages
            .iter()
            .filter(|message| message.code == "import-partial")
            .all(|message| !message.text.contains("changed_dataset_sections")
                && !message.text.contains("unsupported_aggregate_fit")),
        "stable internal diagnostic IDs must not appear in the user-facing import message"
    );
}

#[test]
fn batch_import_retries_a_fit_file_after_its_source_arrives() {
    let creation = eframe::CreationContext::_new_kittest(egui::Context::default());
    let mut app = StudioApp::new(&creation, Instant::now(), None);
    let mut imported = StudioSession::default();
    imported
        .import_data_file(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests")
                .join("fixtures")
                .join("lite-source-fit.txt"),
        )
        .unwrap();
    let source = std::env::temp_dir().join(format!(
        "instplot-studio-source-first-{}.csv",
        std::process::id()
    ));
    let fit = std::env::temp_dir().join(format!(
        "instplot-studio-fit-first-{}.csv",
        std::process::id()
    ));
    instplot_io::save_text_combined(
        &source,
        &[&imported.datasets()[0]],
        instplot_io::TextExportFormat::Csv,
        &[],
    )
    .unwrap();
    instplot_io::save_text_combined(
        &fit,
        &[&imported.datasets()[1]],
        instplot_io::TextExportFormat::Csv,
        &[],
    )
    .unwrap();
    app.load_data_paths(vec![fit.clone(), source.clone()]);
    std::fs::remove_file(fit).unwrap();
    std::fs::remove_file(source).unwrap();
    assert_eq!(app.session.dataset_count(), 2);
    assert_eq!(app.document.project().data_sources.len(), 2);
    assert!(
        !app.messages
            .iter()
            .any(|message| message.code == "import-partial")
    );
}

#[test]
fn first_file_import_replaces_even_edited_showcase_and_later_import_adds_a_curve() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("smoke.csv");
    let mut session = StudioSession::default();
    session.import_data_file(&path).unwrap();
    let first = session.datasets()[0].clone();
    let first_ids = BTreeSet::from([first.plot_id.clone()]);
    let mut showcase = FigureDocument::showcase();
    showcase.set_palette("tol-bright-v1").unwrap();
    let mut history = EditHistory::new(&showcase, true);
    history
        .execute(
            &mut showcase,
            EditCommand::SetFigureSize {
                width_mm: 89.0,
                height_mm: 65.0,
            },
            None,
        )
        .unwrap();
    assert!(history.is_dirty(&showcase));
    let workspace = WorkspaceState::new("Untitled");
    let document = document_with_imported_datasets(
        &showcase,
        std::slice::from_ref(&first),
        &first_ids,
        workspace.should_replace_showcase_on_import(),
        None,
    )
    .unwrap();
    assert_eq!(document.palette_id(), "tol-bright-v1");
    assert_eq!(document.project().data_sources.len(), 1);
    assert!(
        document
            .project()
            .data_sources
            .iter()
            .all(|source| !source.id.starts_with("showcase-"))
    );
    assert!(
        document
            .project()
            .figure
            .artists
            .iter()
            .all(|artist| !artist.id.starts_with("showcase-"))
    );
    assert_eq!(
        document
            .series()
            .iter()
            .filter(|series| series.binding.is_some())
            .count(),
        1
    );

    let mut second = first.clone();
    second.plot_id = "second-import".to_owned();
    second.label = Some("Second import".to_owned());
    second.columns[0].values = vec![100.0, 200.0, 300.0];
    second.columns[1].values = vec![10.0, 20.0, 30.0];
    let imported_ids = BTreeSet::from([second.plot_id.clone()]);
    let mut datasets = vec![first, second];
    let mut appended = document_with_imported_datasets(
        &document,
        &datasets,
        &imported_ids,
        false,
        Some(("field", "response")),
    )
    .unwrap();
    assert_eq!(appended.project().data_sources.len(), 2);
    assert_eq!(
        appended
            .series()
            .iter()
            .filter(|series| series.binding.is_some())
            .count(),
        2
    );
    assert!(appended.axis_record(AxisDimension::X).maximum >= 300.0);
    for index in 3..=4 {
        let mut next = datasets[0].clone();
        next.plot_id = format!("import-{index}");
        next.label = Some(format!("Import {index}"));
        next.source = PathBuf::from(format!("run-{index}.csv"));
        let imported_ids = BTreeSet::from([next.plot_id.clone()]);
        datasets.push(next);
        appended = document_with_imported_datasets(
            &appended,
            &datasets,
            &imported_ids,
            false,
            Some(("field", "response")),
        )
        .unwrap();
    }
    let colors = appended
        .project()
        .figure
        .artists
        .iter()
        .filter_map(|artist| match &artist.properties {
            ArtistProperties::Line { stroke, .. } | ArtistProperties::ErrorBar { stroke, .. } => {
                Some(stroke.color_id.as_str())
            }
            ArtistProperties::Scatter { marker, .. } => Some(marker.color_id.as_str()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(colors.len(), 4, "four imported files need four colors");
    appended.layout_figure().unwrap();
}

#[test]
fn palette_internal_ids_are_not_shown_as_color_names() {
    let showcase = FigureDocument::showcase();
    for color in showcase.palette_colors() {
        let chinese = palette_color_name(UiLanguage::Chinese, &color.id);
        let english = palette_color_name(UiLanguage::English, &color.id);
        assert!(!chinese.contains("object-"), "{}", color.id);
        assert!(!english.contains("object-"), "{}", color.id);
        assert_ne!(chinese, color.id);
    }
    for palette_id in USER_PALETTE_IDS {
        let registry = builtin_palette_registry(palette_id).unwrap();
        for color in registry.colors {
            let chinese = palette_color_name(UiLanguage::Chinese, &color.id);
            let english = palette_color_name(UiLanguage::English, &color.id);
            assert_ne!(chinese, color.id);
            assert_ne!(english, color.id);
        }
    }
}

#[test]
fn palette_window_names_cover_four_groups_without_internal_ids() {
    let groups = [
        (PaletteKind::Qualitative, "分类", "Categorical"),
        (PaletteKind::Sequential, "有序", "Ordered"),
        (PaletteKind::Diverging, "发散", "Diverging"),
        (PaletteKind::Neutral, "辅助", "Supporting"),
    ];
    for (kind, chinese, english) in groups {
        assert_eq!(palette_group_name(UiLanguage::Chinese, kind), chinese);
        assert_eq!(palette_group_name(UiLanguage::English, kind), english);
    }
    for palette_id in USER_PALETTE_IDS {
        assert_ne!(
            palette_scheme_name(UiLanguage::Chinese, palette_id),
            "Custom"
        );
        assert_ne!(
            palette_scheme_name(UiLanguage::English, palette_id),
            "Custom"
        );
        assert!(builtin_palette(palette_id).is_some());
    }
}

#[test]
fn showcase_keeps_fixed_fixture_separate_and_resolves() {
    let fixed = FigureDocument::fixed();
    let showcase = FigureDocument::showcase();
    assert_eq!(fixed.palette_colors().len(), 3);
    assert_eq!(showcase.palette_colors().len(), 9);
    assert!(showcase.project().figure.artists.len() > fixed.project().figure.artists.len());
    let lines = showcase
        .project()
        .figure
        .artists
        .iter()
        .filter(|artist| matches!(artist.properties, ArtistProperties::Line { .. }))
        .collect::<Vec<_>>();
    let markers = showcase
        .project()
        .figure
        .artists
        .iter()
        .filter_map(|artist| match &artist.properties {
            ArtistProperties::Scatter { marker, .. } => Some(marker),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(lines.len(), 7);
    assert_eq!(markers.len(), 7);
    assert!(
        markers
            .iter()
            .all(|marker| PRODUCT_MARKER_SHAPES.contains(&marker.shape))
    );
    assert_eq!(
        markers
            .iter()
            .map(|marker| marker.color_id.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        7
    );
    assert!(markers.iter().enumerate().all(|(index, marker)| {
        markers[..index]
            .iter()
            .all(|earlier| earlier.shape != marker.shape)
    }));
    for source in showcase
        .project()
        .data_sources
        .iter()
        .filter(|source| source.id.starts_with("showcase-line-"))
    {
        let instplot_studio::DataSourcePayload::Embedded {
            columns, row_count, ..
        } = &source.payload
        else {
            panic!("showcase curve is not embedded")
        };
        assert_eq!(*row_count, 57);
        let y = &columns
            .iter()
            .find(|column| column.name == "y")
            .unwrap()
            .values;
        assert!(
            y.windows(3)
                .any(|values| (values[0] - 2.0 * values[1] + values[2]).abs() > 0.001),
            "{} is not visibly curved",
            source.id
        );
    }
    assert!(
        showcase
            .project()
            .figure
            .artists
            .iter()
            .all(|artist| { !matches!(artist.properties, ArtistProperties::ReferenceLine { .. }) })
    );
    let resolved = resolved_preview(&showcase).unwrap();
    let report = check_publication(&showcase, &resolved, 300);
    assert_eq!(report.error_count(), 0, "{:#?}", report.findings);
    assert_eq!(report.warning_count(), 0, "{:#?}", report.findings);
    let pdf = instplot_studio::figure_pdf(&showcase).unwrap();
    let png = instplot_studio::figure_png(&showcase, 300).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
}

#[test]
fn all_product_marker_shapes_validate_and_resolve() {
    for shape in [
        MarkerShape::Circle,
        MarkerShape::Square,
        MarkerShape::Triangle,
        MarkerShape::TriangleDown,
        MarkerShape::Diamond,
        MarkerShape::Pentagon,
        MarkerShape::Star,
        MarkerShape::Plus,
        MarkerShape::Cross,
    ] {
        let mut project = FigureDocument::showcase().project().clone();
        let artist = project
            .figure
            .artists
            .iter_mut()
            .find(|artist| artist.id == "node-17")
            .unwrap();
        let ArtistProperties::Scatter { marker, .. } = &mut artist.properties else {
            panic!("showcase marker missing");
        };
        marker.shape = shape;
        project.validate().unwrap();
        let encoded = serde_json::to_vec(&project).unwrap();
        let decoded = serde_json::from_slice(&encoded).unwrap();
        let document = FigureDocument::from_project(decoded).unwrap();
        resolved_preview(&document).unwrap();
    }
}

#[test]
fn hollow_marker_choices_round_trip_and_reach_the_export_display_list() {
    for shape in PRODUCT_MARKER_SHAPES {
        let mut document = FigureDocument::showcase();
        let mut artist = document.artist_record("node-17").unwrap();
        let ArtistProperties::Scatter { marker, .. } = &mut artist.properties else {
            panic!("showcase marker missing")
        };
        marker.shape = shape;
        marker.filled = false;
        document.set_artist_record(artist).unwrap();
        let encoded = serde_json::to_vec(document.project()).unwrap();
        let reopened =
            FigureDocument::from_project(serde_json::from_slice(&encoded).unwrap()).unwrap();
        let ArtistProperties::Scatter { marker, .. } =
            reopened.artist_record("node-17").unwrap().properties
        else {
            panic!("showcase marker missing")
        };
        assert_eq!(marker.shape, shape);
        assert!(!marker.filled);
        let layout = reopened.layout_figure().unwrap();
        assert!(layout.result.display_list.items.iter().any(|item| {
            matches!(
                item,
                instplot_render::DisplayItem::Path {
                    source,
                    fill: None,
                    stroke: Some(_),
                    ..
                } if layout.project_ids.get(source).map(String::as_str) == Some("node-17")
            )
        }));
    }
}

#[test]
fn trackpad_phase_is_distinct_from_plain_wheel() {
    let event = |phase| egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, 20.0),
        phase,
        modifiers: egui::Modifiers::NONE,
    };
    assert_eq!(
        trackpad_scroll_state(&[event(egui::TouchPhase::Move)], false),
        (false, false)
    );
    assert_eq!(
        trackpad_scroll_state(&[event(egui::TouchPhase::Start)], false),
        (true, true)
    );
    assert_eq!(
        trackpad_scroll_state(&[event(egui::TouchPhase::Move)], true),
        (true, true)
    );
    assert_eq!(
        trackpad_scroll_state(&[event(egui::TouchPhase::End)], true),
        (true, false)
    );
}

#[test]
fn legend_and_annotation_drag_accumulate_frame_deltas() {
    for id in ["legend", "annotation"] {
        let mut drag = ArtistDrag {
            id: id.to_owned(),
            start_x: 40.0,
            start_y: 30.0,
            bounds: (40.0, 30.0, 80.0, 50.0),
            total_delta: egui::Vec2::ZERO,
            press_pointer: None,
            legend: None,
            mode: ArtistDragMode::Move,
            preview_bounds: (40.0, 30.0, 80.0, 50.0),
            candidate_grid: None,
            candidate_placement: None,
            connector_index: None,
        };
        assert_eq!(
            drag.position_after(egui::vec2(10.0, 4.0), 2.0, 200.0, 150.0),
            (45.0, 32.0)
        );
        assert_eq!(
            drag.position_after(egui::vec2(6.0, 8.0), 2.0, 200.0, 150.0),
            (48.0, 36.0)
        );
        assert_eq!(
            drag.position_after(egui::Vec2::ZERO, 2.0, 200.0, 150.0),
            (48.0, 36.0)
        );
    }
}

#[test]
fn annotation_connector_endpoint_drag_updates_data_coordinates() {
    let creation = eframe::CreationContext::_new_kittest(egui::Context::default());
    let mut app = StudioApp::new(&creation, Instant::now(), None);
    let annotation_id = app
        .document
        .add_annotation(vec![LabelNode::Text("target".to_owned())])
        .unwrap();
    let mut record = app.document.artist_record(&annotation_id).unwrap();
    let ArtistProperties::Annotation { connectors, .. } = &mut record.properties else {
        unreachable!()
    };
    connectors.push(AnnotationConnectorRecord {
        target_x: 0.0,
        target_y: 0.0,
        axes: AxisBinding::PRIMARY,
        stroke: StrokeStyle {
            color_id: "blue".to_owned(),
            width_pt: 0.7,
            dash_pt: Vec::new(),
        },
        start_arrow: false,
        end_arrow: true,
        arrow_size_pt: 5.0,
    });
    app.document.set_artist_record(record).unwrap();
    app.resolved = resolved_preview(&app.document).unwrap();
    let bounds = selected_hit_bounds_for_role(
        &app.resolved,
        &annotation_id,
        Some(SelectableRole::AnnotationConnector),
    )
    .unwrap();
    let axes = app.resolved.layout.result.axes;
    let target = (axes.x + axes.width * 0.75, axes.y + axes.height * 0.25);
    app.handle_artist_drag(
        CanvasDragEvent {
            id: annotation_id.clone(),
            role: SelectableRole::AnnotationConnector,
            bounds,
            delta: egui::Vec2::ZERO,
            pointer: Some(target),
            press_pointer: Some(((bounds.0 + bounds.2) / 2.0, (bounds.1 + bounds.3) / 2.0)),
            mode: ArtistDragMode::Move,
            started: true,
            stopped: true,
            data_index: Some(0),
        },
        1.0,
    );
    let record = app.document.artist_record(&annotation_id).unwrap();
    let ArtistProperties::Annotation { connectors, .. } = record.properties else {
        unreachable!()
    };
    let expected = data_coordinates_from_local(&app.document, axes, target.0, target.1).unwrap();
    assert!((connectors[0].target_x - expected.0).abs() < 1.0e-9);
    assert!((connectors[0].target_y - expected.1).abs() < 1.0e-9);
}

#[test]
fn annotation_editor_preserves_hard_line_breaks_but_axis_editor_does_not() {
    let mut annotation = "First line\r\nSecond line".to_owned();
    assert!(normalize_label_editor_line_breaks(&mut annotation, true));
    assert_eq!(annotation, "First line\nSecond line");

    let mut axis = "Field\n(mT)".to_owned();
    assert!(normalize_label_editor_line_breaks(&mut axis, false));
    assert_eq!(axis, "Field (mT)");
}

#[test]
fn new_annotation_connectors_use_black_at_point_nine_points() {
    let document = FigureDocument::showcase();
    let black = document
        .palette_colors()
        .iter()
        .find(|color| color.id == ANNOTATION_CONNECTOR_DEFAULT_COLOR_ID)
        .expect("connector black must be available in the editor palette");
    assert_eq!(black.rgba, [0, 0, 0, 255]);
    assert_eq!(ANNOTATION_CONNECTOR_DEFAULT_WIDTH_PT, 0.9);
}

#[test]
fn legend_drag_can_cross_into_and_out_of_the_outside_bands() {
    let context = LegendDragContext {
        axes_left: 40.0,
        axes_right: 220.0,
        axes_top: 100.0,
        axes_bottom: 280.0,
        canvas_top: 88.0,
        entries: 7,
        cell_width: 60.0,
    };
    assert_eq!(context.placement_at(250.0, 110.0), LegendPlacement::Right);
    assert_eq!(context.placement_at(200.0, 110.0), LegendPlacement::Inside);
    assert_eq!(context.placement_at(100.0, 90.0), LegendPlacement::Above);
    assert_eq!(context.placement_at(160.0, 110.0), LegendPlacement::Inside);
    assert_eq!(context.placement_at(220.0, 100.0), LegendPlacement::Inside);
    let far_right = (500.0, 120.0, 560.0, 140.0);
    let compact = context.preview_bounds(far_right, LegendPlacement::Right);
    assert_eq!(compact, (226.0, 120.0, 286.0, 140.0));
    assert_eq!(
        context.stored_position(compact, LegendPlacement::Right),
        (226.0, 32.0)
    );
    let crossing = (200.0, 120.0, 260.0, 140.0);
    assert_eq!(
        context.preview_bounds(crossing, LegendPlacement::Inside),
        (160.0, 120.0, 220.0, 140.0)
    );
    assert_eq!(
        context.stored_position(crossing, LegendPlacement::Inside),
        (160.0, 32.0)
    );
}

#[test]
fn legend_drag_across_all_placements_round_trips_and_exports() {
    let context = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(context);
    let mut app = StudioApp::new(&creation, Instant::now(), None);
    let legend_id = app
        .document
        .project()
        .figure
        .artists
        .iter()
        .find(|artist| artist.kind == instplot_studio::ArtistKind::Legend)
        .unwrap()
        .id
        .clone();
    let mut record = app.document.artist_record(&legend_id).unwrap();
    record.visible = true;
    assert!(app.execute_document_edit(EditCommand::SetArtistRecord(record), "show legend"));

    for expected in [
        LegendPlacement::Right,
        LegendPlacement::Above,
        LegendPlacement::Inside,
    ] {
        let axes = app.resolved.layout.result.axes;
        let bounds =
            selected_hit_bounds_for_role(&app.resolved, &legend_id, Some(SelectableRole::Legend))
                .unwrap();
        let press = ((bounds.0 + bounds.2) / 2.0, (bounds.1 + bounds.3) / 2.0);
        let target = match expected {
            LegendPlacement::Right => (axes.right() + 20.0, axes.y + 20.0),
            LegendPlacement::Above => (axes.x + 20.0, axes.y - 20.0),
            LegendPlacement::Inside => (axes.x + 20.0, axes.y + 20.0),
            LegendPlacement::Auto => unreachable!(),
        };
        app.handle_artist_drag(
            CanvasDragEvent {
                id: legend_id.clone(),
                role: SelectableRole::Legend,
                bounds,
                delta: egui::Vec2::ZERO,
                pointer: Some(target),
                press_pointer: Some(press),
                mode: ArtistDragMode::Move,
                started: true,
                stopped: true,
                data_index: None,
            },
            1.0,
        );
        let ArtistProperties::Legend { placement, .. } =
            app.document.artist_record(&legend_id).unwrap().properties
        else {
            panic!("legend artist missing");
        };
        assert_eq!(placement, expected);
        if expected == LegendPlacement::Inside {
            let legend = app.resolved.layout.result.legend.unwrap();
            let axes = app.resolved.layout.result.axes;
            assert!(legend.x >= axes.x - 1.0);
            assert!(legend.right() <= axes.right() + 1.0);
            assert!(legend.y >= axes.y - 1.0);
            assert!(legend.bottom() <= axes.bottom() + 1.0);
        }
        app.document.project().validate().unwrap();
        let encoded = serde_json::to_vec(app.document.project()).unwrap();
        let reopened =
            FigureDocument::from_project(serde_json::from_slice(&encoded).unwrap()).unwrap();
        let ArtistProperties::Legend {
            placement: reopened_placement,
            x_pt: reopened_x,
            y_pt: reopened_y,
            ..
        } = reopened.artist_record(&legend_id).unwrap().properties
        else {
            panic!("reopened legend artist missing");
        };
        let ArtistProperties::Legend { x_pt, y_pt, .. } =
            app.document.artist_record(&legend_id).unwrap().properties
        else {
            unreachable!();
        };
        assert_eq!(reopened_placement, expected);
        assert!((reopened_x - x_pt).abs() < 1e-8);
        assert!((reopened_y - y_pt).abs() < 1e-8);
        assert!(
            instplot_studio::figure_pdf(&reopened)
                .unwrap()
                .starts_with(b"%PDF-")
        );
        assert!(
            instplot_studio::figure_png(&reopened, 300)
                .unwrap()
                .starts_with(b"\x89PNG\r\n\x1a\n")
        );
        if matches!(expected, LegendPlacement::Above | LegendPlacement::Right) {
            let axes = app.resolved.layout.result.axes;
            let (axis_id, local) = if expected == LegendPlacement::Above {
                (
                    app.document.project().figure.axes[0].x.id.clone(),
                    egui::pos2((axes.x + axes.right()) as f32 * 0.5, axes.y as f32),
                )
            } else {
                (
                    app.document.project().figure.axes[0].y.id.clone(),
                    egui::pos2(axes.right() as f32, (axes.y + axes.bottom()) as f32 * 0.5),
                )
            };
            let origin = egui::pos2(73.0, 41.0);
            let zoom = 1.6;
            let point = origin + local.to_vec2() * zoom;
            let candidates = hit_project_candidates(&app.resolved, point, origin, zoom);
            assert!(candidates.iter().any(|hit| hit.project_id == axis_id));
            assert!(
                candidates
                    .iter()
                    .all(|hit| hit.role != SelectableRole::Legend)
            );
        }
    }
}

#[test]
fn legend_resize_handles_are_distinct_from_the_move_area() {
    let bounds = (10.0, 10.0, 110.0, 50.0);
    let origin = egui::Pos2::ZERO;
    assert_eq!(
        legend_resize_handle_at(egui::pos2(114.0, 30.0), bounds, origin, 1.0),
        Some(ArtistDragMode::ResizeColumns)
    );
    assert_eq!(
        legend_resize_handle_at(egui::pos2(60.0, 54.0), bounds, origin, 1.0),
        Some(ArtistDragMode::ResizeRows)
    );
    assert_eq!(
        legend_resize_handle_at(egui::pos2(60.0, 30.0), bounds, origin, 1.0),
        None
    );
}

#[test]
fn one_frame_drag_uses_release_pointer_even_without_intermediate_delta() {
    let drag = ArtistDrag {
        id: "legend".to_owned(),
        start_x: 20.0,
        start_y: 10.0,
        bounds: (20.0, 10.0, 80.0, 30.0),
        total_delta: egui::Vec2::ZERO,
        press_pointer: Some((40.0, 20.0)),
        legend: None,
        mode: ArtistDragMode::Move,
        preview_bounds: (20.0, 10.0, 80.0, 30.0),
        candidate_grid: None,
        candidate_placement: None,
        connector_index: None,
    };
    assert_eq!(
        drag.frame_delta(Some((50.0, 120.0)), egui::Vec2::ZERO, 1.5),
        egui::vec2(15.0, 150.0)
    );
}

#[test]
fn same_frame_drag_requires_a_real_primary_button_displacement() {
    let event = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    assert_eq!(
        same_frame_primary_drag(&[
            event(egui::pos2(10.0, 20.0), true),
            egui::Event::PointerMoved(egui::pos2(10.0, 100.0)),
            event(egui::pos2(10.0, 100.0), false),
        ]),
        Some((egui::pos2(10.0, 20.0), egui::pos2(10.0, 100.0)))
    );
    assert_eq!(
        same_frame_primary_drag(&[
            event(egui::pos2(10.0, 20.0), true),
            event(egui::pos2(12.0, 21.0), false),
        ]),
        None
    );
}

#[test]
fn canvas_click_does_not_close_an_existing_editor() {
    let creation = eframe::CreationContext::_new_kittest(egui::Context::default());
    let mut app = StudioApp::new(&creation, Instant::now(), None);
    let legend = CanvasHit {
        project_id: "legend".to_owned(),
        role: SelectableRole::Legend,
        data_index: None,
    };
    let axes = CanvasHit {
        project_id: "axes".to_owned(),
        role: SelectableRole::Axes,
        data_index: None,
    };
    app.open_context_editor(legend.clone());
    app.open_context_editor(axes.clone());
    app.open_context_editor(legend.clone());
    assert_eq!(
        app.context_editor_targets,
        [
            CanvasHit {
                project_id: "legend".to_owned(),
                role: SelectableRole::Legend,
                data_index: None,
            },
            axes,
        ]
    );
    assert_eq!(app.context_editor_focus_target, Some(legend));
}

#[test]
fn repeated_tool_commands_raise_instead_of_closing_or_resetting_the_window() {
    let creation = eframe::CreationContext::_new_kittest(egui::Context::default());
    let mut app = StudioApp::new(&creation, Instant::now(), None);

    app.request_palette_window();
    app.request_palette_window();
    assert!(app.show_palette && app.focus_palette);

    app.request_publication_window();
    app.request_publication_window();
    assert!(app.show_inspector && app.focus_inspector);

    app.prepare_manual_data_window();
    app.manual_data.input.groups[0].source_name = "Unsaved draft".to_owned();
    app.focus_manual_data = false;
    app.prepare_manual_data_window();
    assert!(app.manual_data.open && app.focus_manual_data);
    assert_eq!(app.manual_data.input.groups[0].source_name, "Unsaved draft");
}

#[test]
fn studio_interface_style_keeps_readable_controls_and_dialog_spacing() {
    let context = egui::Context::default();
    configure_interface_style(&context);
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let style = context.style_of(theme);
        assert_eq!(egui::TextStyle::Small.resolve(&style).size, 15.0);
        assert_eq!(egui::TextStyle::Body.resolve(&style).size, 16.0);
        assert_eq!(egui::TextStyle::Button.resolve(&style).size, 16.0);
        assert_eq!(egui::TextStyle::Heading.resolve(&style).size, 22.0);
        assert_eq!(style.spacing.button_padding, egui::vec2(14.0, 9.0));
        assert_eq!(style.spacing.interact_size.y, 40.0);
        assert_eq!(style.spacing.item_spacing.y, 12.0);
        assert_eq!(style.spacing.extra_text_line_spacing, 4.0);
        assert_eq!(style.spacing.window_margin, egui::Margin::symmetric(18, 16));
        assert_eq!(style.interaction.resize_grab_radius_side, 8.0);
        assert_ne!(
            style.visuals.panel_fill,
            studio_surface(theme == egui::Theme::Dark)
        );
    }
}

#[test]
fn editor_buttons_keep_full_height_hit_targets() {
    let context = egui::Context::default();
    configure_interface_style(&context);
    let mut output = context.run_ui(egui::RawInput::default(), |ui| {
        let format = studio_label_format_button(ui, "斜体");
        assert!(format.rect.width() >= 64.0);
        assert!(format.rect.height() >= 40.0);
        let symbol = ui.add_sized([40.0, 40.0], egui::Button::new("σ"));
        assert!(symbol.rect.width() >= 40.0);
        assert!(symbol.rect.height() >= 40.0);
        assert!(ui.button("导出").rect.height() >= 40.0);
    });
    output.textures_delta.clear();
}

#[test]
fn data_sidebar_close_button_is_exactly_centered() {
    let context = egui::Context::default();
    configure_interface_style(&context);
    let mut output = context.run_ui(egui::RawInput::default(), |ui| {
        let response = studio_close_button(ui, "删除");
        assert_eq!(response.rect.size(), egui::vec2(34.0, 34.0));
        let file_response =
            studio_close_button_sized(ui, "删除文件", SIDEBAR_FILE_CLOSE_BUTTON_SIZE);
        assert_eq!(file_response.rect.size(), egui::vec2(28.0, 28.0));
    });
    output.textures_delta.clear();
}

#[test]
fn data_sidebar_resize_drag_persists_after_release() {
    fn frame(context: &egui::Context, events: Vec<egui::Event>) -> (f32, egui::Rect) {
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1_000.0, 700.0),
            )),
            events,
            ..Default::default()
        };
        input.focused = true;
        let mut output = context.run_ui(input, |ui| {
            egui::Panel::left("series_tree")
                .resizable(true)
                .show_separator_line(true)
                .default_size(DATA_SIDEBAR_DEFAULT_WIDTH)
                .min_size(DATA_SIDEBAR_MIN_WIDTH)
                .max_size(DATA_SIDEBAR_MAX_WIDTH)
                .show(ui, |ui| {
                    egui::ScrollArea::both()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.set_width(DATA_SIDEBAR_CONTENT_MIN_WIDTH);
                            ui.label("data");
                        });
                });
            egui::CentralPanel::default().show(ui, |_| {});
        });
        output.textures_delta.clear();
        let width =
            egui::containers::panel::PanelState::load(context, egui::Id::new("series_tree"))
                .expect("sidebar panel state")
                .size()
                .x;
        let handle = context
            .read_response(egui::Id::new("series_tree").with("__resize"))
            .expect("sidebar resize handle")
            .rect;
        (width, handle)
    }

    let context = egui::Context::default();
    let (initial_width, handle) = frame(&context, Vec::new());
    let start = handle.center();
    let end = start + egui::vec2(90.0, 0.0);
    frame(
        &context,
        vec![
            egui::Event::PointerMoved(start),
            egui::Event::PointerButton {
                pos: start,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    frame(&context, vec![egui::Event::PointerMoved(end)]);
    let (released_width, _) = frame(
        &context,
        vec![egui::Event::PointerButton {
            pos: end,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    let (next_frame_width, _) = frame(&context, Vec::new());

    assert!(released_width > initial_width + 60.0);
    assert!((next_frame_width - released_width).abs() < 0.5);
}

#[test]
fn label_preview_uses_the_same_bundled_symbol_face_as_export() {
    let job = label_preview_job(&[
        LabelNode::GreekVariable('ϵ'),
        LabelNode::Operator("≤".into()),
        LabelNode::Number("300".into()),
    ]);
    let faces = job
        .sections
        .iter()
        .map(|section| &section.format.font_id.family)
        .collect::<Vec<_>>();
    assert_eq!(
        faces,
        vec![
            &egui::FontFamily::Name(std::sync::Arc::from("TeXGyreHeros-Italic")),
            &egui::FontFamily::Name(std::sync::Arc::from("STIXTwoMath-Regular")),
            &egui::FontFamily::Name(std::sync::Arc::from("TeXGyreHeros-Regular")),
        ]
    );
}

#[test]
fn label_toolbar_wraps_selection_and_places_cursor_inside_empty_scripts() {
    let (wrapped, cursor) = apply_label_snippet("μH", 1..2, "_{}", true);
    assert_eq!(wrapped, "μ_{H}");
    assert_eq!(cursor, 4);
    let (empty, cursor) = apply_label_snippet("H", 1..1, "^{}", true);
    assert_eq!(empty, "H^{}");
    assert_eq!(cursor, 3);
    let (symbol, cursor) = apply_label_snippet("H", 1..1, "σ", false);
    assert_eq!(symbol, "Hσ");
    assert_eq!(cursor, 2);
}

#[test]
fn fixed_tick_text_accepts_finite_values_and_rejects_bad_input() {
    assert_eq!(
        parse_fixed_ticks("-2; 0, 1.5\t3").unwrap(),
        vec![-2.0, 0.0, 1.5, 3.0]
    );
    for invalid in ["", "   ", "oops", "1, NaN", "-inf 1", "1e999"] {
        assert!(parse_fixed_ticks(invalid).is_err(), "accepted {invalid:?}");
    }
}

#[test]
fn cursor_coordinates_use_formal_axes_and_scale_mapping() {
    let document = FigureDocument::fixed();
    let resolved = resolved_preview(&document).unwrap();
    let axes = resolved.layout.result.axes;
    let origin = egui::pos2(17.0, 23.0);
    let zoom = 1.75;
    let screen = |x: f64, y: f64| origin + egui::vec2(x as f32, y as f32) * zoom;
    let lower_left = data_coordinates_at(
        &resolved,
        &document,
        screen(axes.x, axes.bottom()),
        origin,
        zoom,
    )
    .unwrap();
    let upper_right = data_coordinates_at(
        &resolved,
        &document,
        screen(axes.right(), axes.y),
        origin,
        zoom,
    )
    .unwrap();
    let ranges = document.axis_ranges();
    assert!((lower_left.0 - ranges.x_min).abs() < 1e-5);
    assert!((lower_left.1 - ranges.y_min).abs() < 1e-5);
    assert!((upper_right.0 - ranges.x_max).abs() < 1e-5);
    assert!((upper_right.1 - ranges.y_max).abs() < 1e-5);
    assert!(
        data_coordinates_at(
            &resolved,
            &document,
            screen(axes.x - 1.0, axes.y),
            origin,
            zoom,
        )
        .is_none()
    );
    let mut log_axis = document.axis_record(AxisDimension::X);
    log_axis.scale = AxisScale::Log10;
    log_axis.minimum = 1.0;
    log_axis.maximum = 100.0;
    assert!((axis_value_at_fraction(&log_axis, 0.5).unwrap() - 10.0).abs() < 1e-9);
    assert_eq!(format_data_coordinate(-0.25), "−0.25");
}

#[test]
fn closing_an_empty_annotation_deletes_it_as_one_undoable_edit() {
    let context = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(context);
    let mut app = StudioApp::new(&creation, Instant::now(), None);
    let before = app
        .document
        .series()
        .into_iter()
        .map(|series| series.id)
        .collect::<BTreeSet<_>>();
    app.add_text_annotation();
    let annotation = app
        .document
        .series()
        .into_iter()
        .find(|series| series.kind == SeriesKind::Annotation && !before.contains(&series.id))
        .unwrap();
    let record = app.document.artist_record(&annotation.id).unwrap();
    let ArtistProperties::Annotation { label_id, .. } = record.properties else {
        panic!("new text must be an annotation");
    };
    let nodes = app
        .document
        .semantic_label_nodes(&label_id)
        .unwrap()
        .to_vec();
    app.label_inputs.insert(
        label_id,
        LabelInputState {
            source_nodes: nodes,
            text: String::new(),
        },
    );
    app.delete_empty_annotation_on_close(&annotation.id);
    assert!(app.document.artist_record(&annotation.id).is_none());
    app.undo();
    assert!(app.document.artist_record(&annotation.id).is_some());
}

#[test]
fn preview_uses_formal_layout_and_resolved_rotated_text() {
    let resolved = resolved_preview(&FigureDocument::fixed()).unwrap();
    assert!(!resolved.display.items.iter().any(|item| matches!(
        item,
        ResolvedItem::Graphics(DisplayItem::Path {
            stroke: Some(stroke),
            ..
        }) if stroke.color == Color(218, 221, 224, 255)
    )));
    assert!(resolved.display.items.iter().any(|item| matches!(
        item,
        ResolvedItem::Text(text)
            if text.source == NodeId(4) && text.rotation_degrees == -90.0
    )));
    assert!(resolved.display.items.iter().any(|item| matches!(
        item,
        ResolvedItem::Graphics(DisplayItem::Path { source, .. })
            if *source == NodeId(11)
    )));
    assert!(resolved.display.items.iter().any(|item| matches!(
        item,
        ResolvedItem::Graphics(DisplayItem::Path { source, .. })
            if *source == NodeId(13)
    )));
    assert!(
        !resolved
            .display
            .items
            .iter()
            .any(|item| matches!(item, ResolvedItem::Graphics(DisplayItem::GlyphRun(_))))
    );
}

#[test]
fn p6_hit_testing_and_selection_bounds_use_formal_hit_map() {
    let resolved = resolved_preview(&FigureDocument::fixed()).unwrap();
    let bounds = selected_hit_bounds(&resolved, "node-15").unwrap();
    let local = egui::pos2(
        ((bounds.0 + bounds.2) / 2.0) as f32,
        ((bounds.1 + bounds.3) / 2.0) as f32,
    );
    let origin = egui::pos2(120.0, 80.0);
    let zoom = 1.75;
    let screen = origin + local.to_vec2() * zoom;
    assert_eq!(
        hit_project_id(&resolved, screen, origin, zoom).as_deref(),
        Some("node-15")
    );
}

#[test]
fn annotation_selection_uses_visible_glyph_ink() {
    let resolved = resolved_preview(&FigureDocument::fixed()).unwrap();
    let annotation = resolved
        .display
        .items
        .iter()
        .find_map(|item| match item {
            ResolvedItem::Text(text)
                if resolved
                    .layout
                    .project_ids
                    .get(&text.source)
                    .map(String::as_str)
                    == Some("node-14") =>
            {
                Some(text)
            }
            _ => None,
        })
        .unwrap();
    let ink = resolved_text_ink_bounds(annotation).unwrap();
    let selection =
        selected_hit_bounds_for_role(&resolved, "node-14", Some(SelectableRole::Annotation))
            .unwrap();
    for (actual, expected) in [
        (selection.0, ink.0),
        (selection.1, ink.1),
        (selection.2, ink.2),
        (selection.3, ink.3),
    ] {
        assert!((actual - expected).abs() < 0.01);
    }
}

#[test]
fn legend_numeric_order_moves_an_entry_without_reversing_others() {
    let mut entries = vec!["A", "B", "C", "D"];
    move_legend_entry(&mut entries, 0, 2);
    assert_eq!(entries, ["B", "C", "A", "D"]);
    move_legend_entry(&mut entries, 3, 1);
    assert_eq!(entries, ["B", "D", "C", "A"]);
}

#[test]
fn p6_pointer_centered_zoom_keeps_the_document_anchor_stable() {
    let origin = egui::pos2(40.0, 30.0);
    let pointer = egui::pos2(160.0, 120.0);
    let old_zoom = 1.0;
    let old_scroll = egui::vec2(12.0, 8.0);
    let document_point = (pointer - origin) / old_zoom;
    let (new_scroll, new_zoom) = zoom_about_pointer(old_scroll, pointer, origin, old_zoom, 120.0);
    let old_content = old_scroll + document_point * old_zoom;
    let new_content = new_scroll + document_point * old_zoom;
    assert!((new_content - old_content - document_point * (new_zoom - old_zoom)).length() < 1e-4);
    assert!(new_zoom > old_zoom);
}
