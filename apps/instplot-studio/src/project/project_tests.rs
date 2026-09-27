use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

static NEXT_TEMP: AtomicU64 = AtomicU64::new(1);

struct TempDirectory(PathBuf);

impl TempDirectory {
    fn new() -> Self {
        let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("instplot-b2-{}-{sequence}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn current_project_round_trips_without_losing_identity_or_provenance() {
    let project = ProjectDocument::fixed_fixture();
    let bytes = serde_json::to_vec(&project).unwrap();
    let decoded = decode_project(&bytes).unwrap();
    assert_eq!(decoded, project);
    assert_eq!(decoded.figure.id, "node-1");
    assert_eq!(decoded.typography.family, "TeX Gyre Heros");
    assert_eq!(decoded.palette.id, "publication-default-v1");
}

#[test]
fn older_schema_one_embedded_payload_without_alive_still_opens() {
    let mut value = serde_json::to_value(ProjectDocument::fixed_fixture()).unwrap();
    let payload = value["data_sources"][0]["payload"].as_object_mut().unwrap();
    let columns: Vec<EmbeddedColumn> = serde_json::from_value(payload["columns"].clone()).unwrap();
    let row_count = payload["row_count"].as_u64().unwrap() as usize;
    payload.remove("alive");
    payload.insert(
        "sha256".to_owned(),
        Value::String(embedded_digest(&columns, row_count, &[]).unwrap()),
    );
    let decoded = decode_project(&serde_json::to_vec(&value).unwrap()).unwrap();
    let DataSourcePayload::Embedded { alive, .. } = &decoded.data_sources[0].payload else {
        panic!("fixed payload must remain embedded");
    };
    assert!(alive.is_empty());
}

#[test]
fn source_and_fit_identity_survive_round_trip() {
    let embedded = || {
        let columns = vec![EmbeddedColumn {
            name: "x".to_owned(),
            values: vec![1.0, 2.0],
            valid: Vec::new(),
        }];
        let alive = vec![true, false];
        DataSourcePayload::Embedded {
            sha256: embedded_digest(&columns, 2, &alive).unwrap(),
            columns,
            row_count: 2,
            alive,
        }
    };
    let mut project = ProjectDocument::fixed_fixture();
    project.data_sources.extend([
        DataSourceRecord {
            id: "source-1".to_owned(),
            label: "measurement".to_owned(),
            kind: DataSourceKind::Source,
            payload: embedded(),
            fit: None,
            origin_path: None,
            origin: DataSourceOrigin::Imported,
            manual_recipe: None,
            managed_file: None,
        },
        DataSourceRecord {
            id: "fit-1".to_owned(),
            label: "linear fit".to_owned(),
            kind: DataSourceKind::Fit,
            payload: embedded(),
            fit: Some(FitIdentity {
                parent_data_source_id: "source-1".to_owned(),
                source_x_column: "field".to_owned(),
                source_y_column: "response".to_owned(),
                equation: Some("a*x+b".to_owned()),
                display_equation: Some("y = a × x + b".to_owned()),
            }),
            origin_path: None,
            origin: DataSourceOrigin::Imported,
            manual_recipe: None,
            managed_file: None,
        },
    ]);
    project.validate().unwrap();
    let decoded = decode_project(&serde_json::to_vec(&project).unwrap()).unwrap();
    assert_eq!(decoded.data_sources, project.data_sources);
    assert_eq!(decoded.typography, project.typography);
    assert_eq!(decoded.palette, project.palette);

    let mut legacy_value = serde_json::to_value(&project).unwrap();
    legacy_value["data_sources"][2]["fit"]
        .as_object_mut()
        .unwrap()
        .remove("display_equation");
    let legacy = decode_project(&serde_json::to_vec(&legacy_value).unwrap()).unwrap();
    assert_eq!(
        legacy.data_sources[2]
            .fit
            .as_ref()
            .unwrap()
            .display_equation,
        None
    );
}

#[test]
fn unknown_fields_are_rejected_instead_of_silently_discarded() {
    let mut value = serde_json::to_value(ProjectDocument::fixed_fixture()).unwrap();
    value["unexpected"] = Value::Bool(true);
    let error = decode_project(&serde_json::to_vec(&value).unwrap()).unwrap_err();
    assert!(error.to_string().contains("unknown field `unexpected`"));

    let mut nested = serde_json::to_value(ProjectDocument::fixed_fixture()).unwrap();
    nested["figure"]["axes"][0]["unexpected"] = Value::Bool(true);
    let error = decode_project(&serde_json::to_vec(&nested).unwrap()).unwrap_err();
    assert!(error.to_string().contains("unknown field `unexpected`"));
}

#[test]
fn interval_locator_round_trips_and_rejects_unsafe_steps() {
    let mut project = ProjectDocument::fixed_fixture();
    project.figure.axes[0].x.locator = LocatorSpec::Interval { step: 0.5 };
    project.validate().unwrap();
    let reopened = decode_project(&serde_json::to_vec(&project).unwrap()).unwrap();
    assert_eq!(
        reopened.figure.axes[0].x.locator,
        project.figure.axes[0].x.locator
    );

    for step in [0.0, -1.0, f64::NAN, 0.000_001] {
        project.figure.axes[0].x.locator = LocatorSpec::Interval { step };
        assert!(project.validate().is_err(), "accepted {step:?}");
    }
    project.figure.axes[0].x.locator = LocatorSpec::Interval { step: 0.5 };
    project.figure.axes[0].x.scale = AxisScale::Log10;
    assert!(project.validate().is_err());
}

#[test]
fn minor_interval_round_trips_and_legacy_projects_keep_auto_minor_ticks() {
    let mut project = ProjectDocument::fixed_fixture();
    project.figure.axes[0].x.minor_interval = Some(0.5);
    project.validate().unwrap();
    let encoded = serde_json::to_vec(&project).unwrap();
    let reopened = decode_project(&encoded).unwrap();
    assert_eq!(reopened.figure.axes[0].x.minor_interval, Some(0.5));

    let mut legacy = serde_json::to_value(&project).unwrap();
    legacy["figure"]["axes"][0]["x"]
        .as_object_mut()
        .unwrap()
        .remove("minor_interval");
    let reopened = decode_project(&serde_json::to_vec(&legacy).unwrap()).unwrap();
    assert_eq!(reopened.figure.axes[0].x.minor_interval, None);

    for step in [0.0, -1.0, f64::NAN, 0.001] {
        project.figure.axes[0].x.minor_interval = Some(step);
        assert!(
            project.validate().is_err(),
            "accepted minor interval {step:?}"
        );
    }
    project.figure.axes[0].x.minor_interval = Some(0.5);
    project.figure.axes[0].x.scale = AxisScale::Log10;
    assert!(project.validate().is_err());
}

#[test]
fn legacy_schema_zero_migrates_with_ranges_and_audit_record() {
    let legacy = include_bytes!("../../tests/fixtures/project-v0.instplot");
    let migrated = decode_project(legacy).unwrap();
    assert_eq!(migrated.schema_version, PROJECT_SCHEMA_VERSION);
    assert_eq!(migrated.figure.id, "legacy-figure");
    assert_eq!(migrated.figure.axes[0].x.minimum, -4.0);
    assert_eq!(
        migrated.provenance.last().unwrap().operation,
        "migrate_schema_0_to_8"
    );
}

#[test]
fn schema_one_migrates_artist_visibility_to_visible() {
    let mut value = serde_json::to_value(ProjectDocument::fixed_fixture()).unwrap();
    value["schema_version"] = Value::from(1);
    for artist in value["figure"]["artists"].as_array_mut().unwrap() {
        artist.as_object_mut().unwrap().remove("visible");
    }
    let migrated = decode_project(&serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(migrated.schema_version, PROJECT_SCHEMA_VERSION);
    assert!(migrated.figure.artists.iter().all(|artist| artist.visible));
    assert_eq!(
        migrated.provenance.last().unwrap().operation,
        "migrate_schema_1_to_8"
    );
}

#[test]
fn schema_two_migrates_axis_appearance_defaults() {
    let mut value = serde_json::to_value(ProjectDocument::fixed_fixture()).unwrap();
    value["schema_version"] = Value::from(2);
    for axes in value["figure"]["axes"].as_array_mut().unwrap() {
        for name in ["x", "y"] {
            let axis = axes[name].as_object_mut().unwrap();
            axis.remove("autoscale");
            axis.remove("appearance");
        }
    }
    let migrated = decode_project(&serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(migrated.schema_version, PROJECT_SCHEMA_VERSION);
    assert!(migrated.figure.axes[0].x.appearance.far_ticks);
    assert_eq!(
        migrated.figure.axes[0].y.appearance.tick_direction,
        TickDirection::In
    );
    assert_eq!(
        migrated.provenance.last().unwrap().operation,
        "migrate_schema_2_to_8"
    );
}

#[test]
fn old_legend_without_placement_keeps_its_manual_position() {
    let mut value = serde_json::to_value(ProjectDocument::fixed_fixture()).unwrap();
    value["schema_version"] = Value::from(3);
    let legend = value["figure"]["artists"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|artist| artist["kind"] == "legend")
        .unwrap();
    legend["properties"]
        .as_object_mut()
        .unwrap()
        .remove("placement");
    let decoded = decode_project(&serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(decoded.schema_version, PROJECT_SCHEMA_VERSION);
    assert_eq!(
        decoded.provenance.last().unwrap().operation,
        "migrate_schema_3_to_8"
    );
    let record = decoded
        .figure
        .artists
        .iter()
        .find(|artist| artist.kind == ArtistKind::Legend)
        .unwrap();
    assert!(matches!(
        record.properties,
        ArtistProperties::Legend {
            placement: LegendPlacement::Inside,
            grid: LegendGrid::Auto,
            x_pt: 110.0,
            y_pt: 12.0,
            ..
        }
    ));
}

#[test]
fn schema_four_without_source_origin_migrates_without_losing_data() {
    let mut value = serde_json::to_value(ProjectDocument::fixed_fixture()).unwrap();
    value["schema_version"] = Value::from(4);
    for source in value["data_sources"].as_array_mut().unwrap() {
        source.as_object_mut().unwrap().remove("origin_path");
    }
    let migrated = decode_project(&serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(migrated.schema_version, PROJECT_SCHEMA_VERSION);
    assert!(
        migrated
            .data_sources
            .iter()
            .all(|source| source.origin_path.is_none())
    );
    assert_eq!(
        migrated.provenance.last().unwrap().operation,
        "migrate_schema_4_to_8"
    );
}

#[test]
fn schema_five_annotations_migrate_with_no_connectors() {
    let mut value = serde_json::to_value(ProjectDocument::fixed_fixture()).unwrap();
    value["schema_version"] = Value::from(5);
    for artist in value["figure"]["artists"].as_array_mut().unwrap() {
        if artist["kind"] == "annotation" {
            artist["properties"]
                .as_object_mut()
                .unwrap()
                .remove("connectors");
        }
    }
    let migrated = decode_project(&serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(migrated.schema_version, PROJECT_SCHEMA_VERSION);
    assert!(migrated.figure.artists.iter().any(|artist| matches!(
        &artist.properties,
        ArtistProperties::Annotation { connectors, .. } if connectors.is_empty()
    )));
    assert_eq!(
        migrated.provenance.last().unwrap().operation,
        "migrate_schema_5_to_8"
    );
}

#[test]
fn schema_six_distinguishes_legacy_manual_sources_without_inventing_a_recipe() {
    let mut value = serde_json::to_value(ProjectDocument::fixed_fixture()).unwrap();
    value["schema_version"] = Value::from(6);
    let source = value["data_sources"][0].as_object_mut().unwrap();
    source.insert(
        "origin_path".to_owned(),
        Value::String("manual-data/手动数据 1.txt".to_owned()),
    );
    source.remove("origin");
    source.remove("manual_recipe");
    source.remove("managed_file");

    let migrated = decode_project(&serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(migrated.schema_version, PROJECT_SCHEMA_VERSION);
    assert_eq!(
        migrated.data_sources[0].origin,
        DataSourceOrigin::LegacyManual
    );
    assert!(migrated.data_sources[0].manual_recipe.is_none());
    assert_eq!(
        migrated.provenance.last().unwrap().operation,
        "migrate_schema_6_to_8"
    );
}

#[test]
fn old_marker_without_fill_field_remains_filled() {
    let mut value = serde_json::to_value(ProjectDocument::fixed_fixture()).unwrap();
    let scatter = value["figure"]["artists"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|artist| artist["kind"] == "scatter")
        .unwrap();
    scatter["properties"]["marker"]
        .as_object_mut()
        .unwrap()
        .remove("filled");
    let decoded = decode_project(&serde_json::to_vec(&value).unwrap()).unwrap();
    let scatter = decoded
        .figure
        .artists
        .iter()
        .find(|artist| artist.kind == ArtistKind::Scatter)
        .unwrap();
    let ArtistProperties::Scatter { marker, .. } = &scatter.properties else {
        unreachable!()
    };
    assert!(marker.filled);
}

#[test]
fn atomic_save_keeps_a_valid_backup_and_recovers_from_corruption() {
    let directory = TempDirectory::new();
    let path = directory.0.join("figure.instplot");
    let first = ProjectDocument::fixed_fixture();
    save_project(&path, &first).unwrap();
    let mut second = first.clone();
    second.figure.axes[0].x.maximum = 8.0;
    save_project(&path, &second).unwrap();
    fs::write(&path, b"not JSON").unwrap();

    let recovered = open_project(&path).unwrap();
    assert_eq!(recovered.source, OpenProjectSource::Backup);
    assert_eq!(recovered.document, first);
    assert!(!recovered.warnings.is_empty());
}

#[test]
fn project_round_trip_supports_unicode_and_spaces_in_the_path() {
    let directory = TempDirectory::new();
    let nested = directory.0.join("实验 数据");
    fs::create_dir_all(&nested).unwrap();
    let path = nested.join("磁化 曲线.instplot");
    let expected = ProjectDocument::fixed_fixture();
    save_project(&path, &expected).unwrap();
    let opened = open_project(&path).unwrap();
    assert_eq!(opened.source, OpenProjectSource::Primary);
    assert_eq!(opened.document, expected);
}

#[test]
fn invalid_existing_project_is_never_overwritten() {
    let directory = TempDirectory::new();
    let path = directory.0.join("invalid.instplot");
    let original = b"not a project\n";
    fs::write(&path, original).unwrap();
    let error = save_project(&path, &ProjectDocument::fixed_fixture()).unwrap_err();
    assert!(matches!(error, ProjectError::InvalidExistingProject(_)));
    assert_eq!(fs::read(path).unwrap(), original);
}

#[test]
fn changed_and_missing_sources_warn_without_replacing_stored_identity() {
    let directory = TempDirectory::new();
    let source = directory.0.join("data.csv");
    fs::write(&source, b"x,y\n1,2\n").unwrap();
    let mut project = ProjectDocument::fixed_fixture();
    project
        .upsert_external_source(
            "source-1",
            "data.csv",
            &source,
            DataSourceKind::Source,
            None,
        )
        .unwrap();
    let state = || {
        project
            .source_states()
            .into_iter()
            .find(|(id, _)| *id == "source-1")
            .unwrap()
            .1
    };
    assert_eq!(state(), SourceState::Unchanged);
    let project_path = directory.0.join("source-state.instplot");
    save_project(&project_path, &project).unwrap();

    fs::write(&source, b"x,y\n1,3\n").unwrap();
    assert!(matches!(state(), SourceState::Changed { .. }));
    assert!(
        project
            .data_sources
            .iter()
            .any(|record| record.id == "source-1")
    );
    let changed = open_project(&project_path).unwrap();
    assert_eq!(changed.warnings.len(), 1);
    assert_eq!(changed.document, project);

    fs::remove_file(source).unwrap();
    assert_eq!(state(), SourceState::Missing);
    let missing = open_project(&project_path).unwrap();
    assert_eq!(missing.warnings.len(), 1);
    assert_eq!(missing.document, project);
}

#[test]
fn future_schema_is_rejected_explicitly() {
    let mut value = serde_json::to_value(ProjectDocument::fixed_fixture()).unwrap();
    value["schema_version"] = Value::from(PROJECT_SCHEMA_VERSION + 1);
    assert!(matches!(
        decode_project(&serde_json::to_vec(&value).unwrap()),
        Err(ProjectError::UnsupportedSchema(version)) if version == PROJECT_SCHEMA_VERSION + 1
    ));
}

#[test]
fn high_precision_embedded_values_keep_their_digest_after_json_round_trip() {
    let directory = TempDirectory::new();
    let path = directory.0.join("high-precision.instplot");
    let mut project = ProjectDocument::fixed_fixture();
    project
        .upsert_embedded_source(
            "high-precision",
            "high-precision.csv",
            vec![
                EmbeddedColumn {
                    name: "x".to_owned(),
                    values: vec![0.1, 2.1019019019019023, 4.103803803803804],
                    valid: Vec::new(),
                },
                EmbeddedColumn {
                    name: "y".to_owned(),
                    values: vec![0.10680447707255405, 1.8911068235672623, 3.279660934229967],
                    valid: Vec::new(),
                },
            ],
            vec![true; 3],
            DataSourceKind::Source,
            None,
        )
        .unwrap();

    save_project(&path, &project).unwrap();
    let opened = open_project(&path).unwrap();
    assert_eq!(opened.document, project);
}

#[test]
fn schema_seven_migrates_deterministic_logical_series_without_visual_loss() {
    let mut value = serde_json::to_value(ProjectDocument::fixed_fixture()).unwrap();
    value["schema_version"] = Value::from(7);
    let axes = value["figure"]["axes"][0].as_object_mut().unwrap();
    for field in ["mode", "x2", "y2", "series_groups"] {
        axes.remove(field);
    }
    for artist in value["figure"]["artists"].as_array_mut().unwrap() {
        let properties = artist["properties"].as_object_mut().unwrap();
        properties.remove("axes");
        if let Some(connectors) = properties
            .get_mut("connectors")
            .and_then(Value::as_array_mut)
        {
            for connector in connectors {
                connector.as_object_mut().unwrap().remove("axes");
            }
        }
    }
    let bytes = serde_json::to_vec(&value).unwrap();
    let first = decode_project(&bytes).unwrap();
    let second = decode_project(&bytes).unwrap();
    assert_eq!(first.figure.artists, second.figure.artists);
    assert_eq!(
        first.figure.axes[0].series_groups,
        second.figure.axes[0].series_groups
    );
    assert_eq!(first.figure.axes[0].series_groups.len(), 2);
    assert!(first.figure.axes[0].series_groups.iter().any(|group| {
        group.artist_ids.len() == 2
            && group.artist_ids.iter().any(|id| id == "node-12")
            && group.artist_ids.iter().any(|id| id == "node-13")
    }));
    assert_eq!(
        first.provenance.last().unwrap().operation,
        "migrate_schema_7_to_8"
    );
}

#[test]
fn project_rejects_dual_secondary_binding_and_missing_active_axis() {
    let mut project = ProjectDocument::fixed_fixture();
    project.figure.axes[0].series_groups[0].axes = AxisBinding {
        x: XAxisSlot::X2,
        y: YAxisSlot::Y2,
    };
    assert!(matches!(
        project.validate(),
        Err(ProjectError::Validation(_))
    ));

    let mut project = ProjectDocument::fixed_fixture();
    project.figure.axes[0].mode = AxisMode::DualY;
    assert!(matches!(
        project.validate(),
        Err(ProjectError::Validation(_))
    ));
}

#[test]
fn dual_axis_state_round_trips_and_each_edge_has_one_owner() {
    let mut project = ProjectDocument::fixed_fixture();
    let mut x2 = project.figure.axes[0].x.clone();
    x2.id = "node-x2".to_owned();
    let mut y2 = project.figure.axes[0].y.clone();
    y2.id = "node-y2".to_owned();
    project.figure.axes[0].mode = AxisMode::DualX;
    project.figure.axes[0].x2 = Some(x2);
    project.figure.axes[0].y2 = Some(y2);
    project.figure.axes[0].series_groups[0].axes = AxisBinding {
        x: XAxisSlot::X2,
        y: YAxisSlot::Y1,
    };
    project.validate().unwrap();
    let bytes = serde_json::to_vec(&project).unwrap();
    assert_eq!(decode_project(&bytes).unwrap(), project);

    assert_eq!(AxisMode::Single.edge_owner(AxisEdge::Top), AxisIdentity::X1);
    assert_eq!(
        AxisMode::Single.edge_owner(AxisEdge::Right),
        AxisIdentity::Y1
    );
    assert_eq!(AxisMode::DualX.edge_owner(AxisEdge::Top), AxisIdentity::X2);
    assert_eq!(
        AxisMode::DualY.edge_owner(AxisEdge::Right),
        AxisIdentity::Y2
    );
    for mode in [AxisMode::Single, AxisMode::DualX, AxisMode::DualY] {
        assert_eq!(mode.edge_owner(AxisEdge::Bottom), AxisIdentity::X1);
        assert_eq!(mode.edge_owner(AxisEdge::Left), AxisIdentity::Y1);
    }
}

#[test]
fn ambiguous_schema_seven_series_migration_is_non_destructive_and_warns() {
    let directory = TempDirectory::new();
    let path = directory.0.join("ambiguous-schema-seven.instplot");
    let mut value = serde_json::to_value(ProjectDocument::fixed_fixture()).unwrap();
    let artists = value["figure"]["artists"].as_array_mut().unwrap();
    let mut duplicate = artists
        .iter()
        .find(|artist| artist["id"] == "node-11")
        .unwrap()
        .clone();
    duplicate["id"] = Value::String("node-16".to_owned());
    artists.push(duplicate);
    value["figure"]["axes"][0]["artist_ids"]
        .as_array_mut()
        .unwrap()
        .push(Value::String("node-16".to_owned()));
    value["schema_version"] = Value::from(7);
    let axes = value["figure"]["axes"][0].as_object_mut().unwrap();
    for field in ["mode", "x2", "y2", "series_groups"] {
        axes.remove(field);
    }
    for artist in value["figure"]["artists"].as_array_mut().unwrap() {
        let properties = artist["properties"].as_object_mut().unwrap();
        properties.remove("axes");
        if let Some(connectors) = properties
            .get_mut("connectors")
            .and_then(Value::as_array_mut)
        {
            for connector in connectors {
                connector.as_object_mut().unwrap().remove("axes");
            }
        }
    }
    let old_artist_count = value["figure"]["artists"].as_array().unwrap().len();
    let old_legend_entry_count = value["figure"]["artists"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|artist| artist["properties"]["entries"].as_array())
        .map(Vec::len)
        .sum::<usize>();
    fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();

    let report = open_project(&path).unwrap();
    assert_eq!(report.document.figure.artists.len(), old_artist_count);
    let new_legend_entry_count = report
        .document
        .figure
        .artists
        .iter()
        .filter_map(|artist| match &artist.properties {
            ArtistProperties::Legend { entries, .. } => Some(entries.len()),
            _ => None,
        })
        .sum::<usize>();
    assert_eq!(new_legend_entry_count, old_legend_entry_count);
    assert!(
        report
            .document
            .provenance
            .iter()
            .any(|record| { record.operation == "migrate_legacy_series_groups_with_ambiguity" })
    );
    assert!(
        report
            .warnings
            .iter()
            .any(|warning| warning.contains("could not be grouped"))
    );
    let groups = &report.document.figure.axes[0].series_groups;
    assert!(groups.iter().any(|group| group.artist_ids == ["node-11"]));
    assert!(groups.iter().any(|group| group.artist_ids == ["node-16"]));
}
