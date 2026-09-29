use super::*;

fn remove_app_data_sources(app: &mut StudioApp, ids: Vec<String>) {
    let state = AppTransactionState {
        document: &app.document,
        session: &app.session,
        workspace: &app.workspace,
        edit_history: &app.edit_history,
    };
    let outcome = ApplicationController::execute(state, AppAction::RemoveDataSources(ids)).unwrap();
    let effect = app.commit_app_outcome(outcome);
    assert!(matches!(effect, AppEffect::RemovedData { .. }));
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
    let preview = resolved_preview(&document).unwrap();
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
    assert_eq!(outcome.resolved.display.width, preview.display.width);
    assert_eq!(outcome.resolved.display.height, preview.display.height);
    assert_eq!(
        outcome.resolved.display.geometry.export_translation,
        (0.0, 0.0)
    );
    assert!(std::fs::read_to_string(&path).unwrap().starts_with("<svg "));
    std::fs::remove_file(path).unwrap();
}

#[test]
fn hidden_ticks_and_publication_errors_do_not_block_export() {
    let mut document = FigureDocument::fixed();
    for identity in [AxisIdentity::X1, AxisIdentity::Y1] {
        let mut visibility = document.axis_visibility(identity).unwrap();
        visibility.ticks = false;
        visibility.tick_labels = false;
        document.set_axis_visibility(identity, visibility).unwrap();
    }
    let mut preferences = document.export_preferences().clone();
    preferences.raster_dpi.push(72);
    preferences.selected_raster_dpi = 72;
    document.set_export_preferences(preferences).unwrap();
    let preview = resolved_preview(&document).unwrap();
    let report = check_publication(&document, &preview, 72);
    assert!(report.error_count() > 0);
    assert_eq!(
        export_confirmation_text(UiLanguage::Chinese, report.error_count()),
        "仍然导出"
    );

    let (session, _) = StudioSession::from_project(document.project());
    let workspace = WorkspaceState::new("Untitled");
    let history = EditHistory::new(&document, true);
    let path = std::env::temp_dir().join(format!(
        "instplot-hidden-ticks-export-{}.svg",
        std::process::id()
    ));
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
fn reimporting_changed_file_invalidates_stale_manual_ranges_and_ticks() {
    let creation = eframe::CreationContext::_new_kittest(egui::Context::default());
    let mut app = StudioApp::new(&creation, Instant::now(), None);
    let path = std::env::temp_dir().join(format!(
        "instplot-reimport-autoscale-{}.csv",
        std::process::id()
    ));
    std::fs::write(&path, "x,y\n0,0.001\n1,0.002\n").unwrap();
    app.load_data_paths(vec![path.clone()]);
    for dimension in [AxisDimension::X, AxisDimension::Y] {
        let mut axis = app.document.axis_record(dimension);
        axis.autoscale = false;
        axis.minimum = -0.01;
        axis.maximum = 0.01;
        axis.locator = LocatorSpec::Fixed {
            values: vec![-0.01, 0.0, 0.01],
        };
        axis.minor_interval = Some(0.001);
        axis.formatter = FormatterSpec::Decimal { precision: 4 };
        app.document.set_axis_record(dimension, axis).unwrap();
    }

    std::fs::write(
        &path,
        "x,y\n0,1000\n1,10000\n2,10001\n3,10005\n4,120000\n5,1100\n",
    )
    .unwrap();
    app.load_data_paths(vec![path.clone()]);
    std::fs::remove_file(path).unwrap();

    let x = app.document.axis_record(AxisDimension::X);
    let y = app.document.axis_record(AxisDimension::Y);
    for axis in [&x, &y] {
        assert!(
            axis.autoscale,
            "reimport must reactivate autoscale: {axis:?}"
        );
        assert!(matches!(axis.locator, LocatorSpec::Auto { .. }));
        assert_eq!(axis.minor_interval, None);
        assert_eq!(axis.formatter, FormatterSpec::Auto);
    }
    assert!(x.minimum < 0.0 && x.maximum > 5.0, "{x:?}");
    assert!(y.minimum < 1000.0 && y.maximum > 120_000.0, "{y:?}");
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
fn manual_multiple_series_share_axis_exponents_and_dual_axes_scale_independently() {
    let creation = eframe::CreationContext::_new_kittest(egui::Context::default());
    let mut app = StudioApp::new(&creation, Instant::now(), None);
    app.manual_data.input = ManualDataInput {
        groups: [
            ("small-a", "0.001 0.002", "0.001 0.002"),
            ("small-b", "0.003 0.004", "0.003 0.004"),
            ("large-a", "10000 20000", "10000 20000"),
            ("large-b", "30000 40000", "30000 40000"),
        ]
        .into_iter()
        .map(|(id, x, y)| ManualDataGroupInput {
            group_id: id.to_owned(),
            source_name: id.to_owned(),
            x: ManualAxisInput {
                name: format!("x-{id}"),
                measurements: vec![x.to_owned()],
            },
            y: ManualAxisInput {
                name: format!("y-{id}"),
                measurements: vec![y.to_owned()],
            },
            error_statistic: ErrorStatistic::StandardDeviation,
            plot_style: ManualPlotStyle::LineAndMarker,
        })
        .collect(),
    };

    app.insert_manual_data().unwrap();

    let layout = app.document.layout_figure().unwrap();
    assert_eq!(layout.result.x_axis.shared_exponent, Some(4));
    assert_eq!(layout.result.y_axis.shared_exponent, Some(4));

    let series_by_source = app
        .document
        .logical_series()
        .into_iter()
        .map(|series| {
            let source = series.binding.as_ref().unwrap().data_source_id.clone();
            (source, series.id)
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(series_by_source.len(), 4);

    app.document.set_axis_mode(AxisMode::DualY).unwrap();
    for source in ["large-a", "large-b"] {
        app.document
            .set_series_axis_binding(
                &series_by_source[source],
                AxisBinding {
                    x: XAxisSlot::X1,
                    y: YAxisSlot::Y2,
                },
            )
            .unwrap();
    }
    let layout = app.document.layout_figure().unwrap();
    assert_eq!(layout.result.x_axis.shared_exponent, Some(4));
    assert_eq!(layout.result.y_axis.shared_exponent, Some(-3));
    assert_eq!(
        layout.result.y2_axis.as_ref().unwrap().shared_exponent,
        Some(4)
    );

    app.document.set_axis_mode(AxisMode::DualX).unwrap();
    for source in ["small-a", "small-b"] {
        app.document
            .set_series_axis_binding(&series_by_source[source], AxisBinding::PRIMARY)
            .unwrap();
    }
    for source in ["large-a", "large-b"] {
        app.document
            .set_series_axis_binding(
                &series_by_source[source],
                AxisBinding {
                    x: XAxisSlot::X2,
                    y: YAxisSlot::Y1,
                },
            )
            .unwrap();
    }
    let layout = app.document.layout_figure().unwrap();
    let snapshot = layout.result.snapshot();
    assert_eq!(layout.result.x_axis.shared_exponent, Some(-3), "{snapshot}");
    assert_eq!(
        layout.result.x2_axis.as_ref().unwrap().shared_exponent,
        Some(4),
        "{snapshot}"
    );
    assert_eq!(layout.result.y_axis.shared_exponent, Some(4), "{snapshot}");
}

#[test]
fn imported_and_manual_series_share_the_same_dual_axis_binding_path() {
    let creation = eframe::CreationContext::_new_kittest(egui::Context::default());
    let mut app = StudioApp::new(&creation, Instant::now(), None);
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("smoke.csv");
    app.load_data_paths(vec![fixture]);
    app.manual_data.input = ManualDataInput {
        groups: vec![ManualDataGroupInput {
            group_id: "manual-dual-axis".to_owned(),
            source_name: "Manual dual axis".to_owned(),
            x: ManualAxisInput {
                name: "manual x".to_owned(),
                measurements: vec!["0 1 2".to_owned()],
            },
            y: ManualAxisInput {
                name: "manual y".to_owned(),
                measurements: vec!["10 20 30".to_owned()],
            },
            error_statistic: ErrorStatistic::StandardDeviation,
            plot_style: ManualPlotStyle::LineAndMarker,
        }],
    };
    app.insert_manual_data().unwrap();

    let logical = app.document.logical_series();
    assert_eq!(logical.len(), 2);
    let imported = logical
        .iter()
        .find(|series| {
            series
                .binding
                .as_ref()
                .is_some_and(|binding| binding.data_source_id != "manual-dual-axis")
        })
        .unwrap();
    let manual = logical
        .iter()
        .find(|series| {
            series
                .binding
                .as_ref()
                .is_some_and(|binding| binding.data_source_id == "manual-dual-axis")
        })
        .unwrap();
    app.document.set_axis_mode(AxisMode::DualY).unwrap();
    app.document
        .set_series_axis_binding(
            &manual.id,
            AxisBinding {
                x: XAxisSlot::X1,
                y: YAxisSlot::Y2,
            },
        )
        .unwrap();

    assert_eq!(
        app.document.series_axis_binding(&imported.id),
        Some(AxisBinding::PRIMARY)
    );
    assert_eq!(
        app.document.series_axis_binding(&manual.id),
        Some(AxisBinding {
            x: XAxisSlot::X1,
            y: YAxisSlot::Y2,
        })
    );
    assert_eq!(
        app.document.project().data_sources.len(),
        app.session.dataset_count()
    );
}

#[test]
fn logical_series_title_reports_the_combined_plot_style() {
    let creation = eframe::CreationContext::_new_kittest(egui::Context::default());
    let mut app = StudioApp::new(&creation, Instant::now(), None);
    app.manual_data.input = ManualDataInput {
        groups: vec![ManualDataGroupInput {
            group_id: "combined-title".to_owned(),
            source_name: "Combined title".to_owned(),
            x: ManualAxisInput {
                name: "x".to_owned(),
                measurements: vec!["0 1".to_owned()],
            },
            y: ManualAxisInput {
                name: "y".to_owned(),
                measurements: vec!["1 2".to_owned()],
            },
            error_statistic: ErrorStatistic::StandardDeviation,
            plot_style: ManualPlotStyle::LineAndMarker,
        }],
    };
    app.insert_manual_data().unwrap();
    let series = app.document.logical_series().remove(0);
    assert_eq!(
        canvas_logical_series_title(UiLanguage::Chinese, &app.document, &series),
        "曲线＋点 · Combined title"
    );
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
fn editing_manual_values_invalidates_stale_manual_axes_and_preserves_custom_labels() {
    let creation = eframe::CreationContext::_new_kittest(egui::Context::default());
    let mut app = StudioApp::new(&creation, Instant::now(), None);
    app.manual_data.input = ManualDataInput {
        groups: vec![ManualDataGroupInput {
            group_id: "manual-autoscale".to_owned(),
            source_name: "Autoscale".to_owned(),
            x: ManualAxisInput {
                name: "Field".to_owned(),
                measurements: vec!["0 1".to_owned()],
            },
            y: ManualAxisInput {
                name: "Signal".to_owned(),
                measurements: vec!["0.001 0.002".to_owned()],
            },
            error_statistic: ErrorStatistic::StandardDeviation,
            plot_style: ManualPlotStyle::LineAndMarker,
        }],
    };
    app.insert_manual_data().unwrap();
    app.document
        .set_axis_label(
            AxisDimension::Y,
            vec![LabelNode::Text("Custom response".to_owned())],
        )
        .unwrap();
    for dimension in [AxisDimension::X, AxisDimension::Y] {
        let mut axis = app.document.axis_record(dimension);
        axis.autoscale = false;
        axis.minimum = -0.01;
        axis.maximum = 0.01;
        axis.locator = LocatorSpec::Interval { step: 0.001 };
        axis.minor_interval = Some(0.0002);
        axis.formatter = FormatterSpec::Decimal { precision: 4 };
        app.document.set_axis_record(dimension, axis).unwrap();
    }

    app.prepare_manual_data_window();
    app.manual_data.editing_group_id = Some("manual-autoscale".to_owned());
    app.manual_data.input.groups[0].x.measurements[0] = "0 1 2 3 4 5".to_owned();
    app.manual_data.input.groups[0].y.measurements[0] =
        "1000 10000 10001 10005 120000 1100".to_owned();
    app.insert_manual_data().unwrap();

    let x = app.document.axis_record(AxisDimension::X);
    let y = app.document.axis_record(AxisDimension::Y);
    for axis in [&x, &y] {
        assert!(
            axis.autoscale,
            "data edits must reactivate autoscale: {axis:?}"
        );
        assert!(matches!(axis.locator, LocatorSpec::Auto { .. }));
        assert_eq!(axis.minor_interval, None);
        assert_eq!(axis.formatter, FormatterSpec::Auto);
    }
    assert!(x.minimum < 0.0 && x.maximum > 5.0, "{x:?}");
    assert!(y.minimum < 1000.0 && y.maximum > 120_000.0, "{y:?}");
    assert_eq!(
        app.document.axis_label(AxisDimension::Y),
        &[LabelNode::Text("Custom response".to_owned())]
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
