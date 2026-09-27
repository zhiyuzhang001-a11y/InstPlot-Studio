use super::*;

pub(super) fn migrate_v0(value: Value) -> Result<ProjectDocument, ProjectError> {
    let legacy: LegacyProjectV0 = serde_json::from_value(value)
        .map_err(|error| ProjectError::Decode(format!("legacy schema 0: {error}")))?;
    if legacy.schema_version != 0 {
        return Err(ProjectError::Decode(
            "legacy migration received a non-zero schema".to_owned(),
        ));
    }
    let mut document = ProjectDocument::fixed_fixture();
    document.figure.id = legacy.figure_id;
    document.producer_version = env!("CARGO_PKG_VERSION").to_owned();
    let axes = document
        .figure
        .axes
        .first_mut()
        .expect("fixed fixture has one axes");
    axes.x.minimum = legacy.x_min;
    axes.x.maximum = legacy.x_max;
    axes.y.minimum = legacy.y_min;
    axes.y.maximum = legacy.y_max;
    document.provenance.push(ProvenanceRecord {
        id: "provenance-migrate-v0".to_owned(),
        operation: "migrate_schema_0_to_8".to_owned(),
        input_ids: vec![legacy.producer_version],
        parameters: BTreeMap::new(),
    });
    Ok(document)
}

pub(super) fn migrate_v7(mut value: Value) -> Result<ProjectDocument, ProjectError> {
    value["schema_version"] = Value::from(PROJECT_SCHEMA_VERSION);
    let mut document: ProjectDocument = serde_json::from_value(value)
        .map_err(|error| ProjectError::Decode(format!("schema 7 migration: {error}")))?;
    document.provenance.push(ProvenanceRecord {
        id: "provenance-migrate-v7".to_owned(),
        operation: "migrate_schema_7_to_8".to_owned(),
        input_ids: Vec::new(),
        parameters: BTreeMap::new(),
    });
    finish_legacy_migration(document, 7)
}

pub(super) fn migrate_v1(mut value: Value) -> Result<ProjectDocument, ProjectError> {
    value["schema_version"] = Value::from(PROJECT_SCHEMA_VERSION);
    let artists = value["figure"]["artists"]
        .as_array_mut()
        .ok_or_else(|| ProjectError::Decode("schema 1 artists are missing".to_owned()))?;
    for artist in artists {
        artist
            .as_object_mut()
            .ok_or_else(|| ProjectError::Decode("schema 1 artist is invalid".to_owned()))?
            .insert("visible".to_owned(), Value::Bool(true));
    }
    let mut document: ProjectDocument = serde_json::from_value(value)
        .map_err(|error| ProjectError::Decode(format!("schema 1 migration: {error}")))?;
    document.provenance.push(ProvenanceRecord {
        id: "provenance-migrate-v1".to_owned(),
        operation: "migrate_schema_1_to_8".to_owned(),
        input_ids: Vec::new(),
        parameters: BTreeMap::new(),
    });
    finish_legacy_migration(document, 1)
}

pub(super) fn migrate_v2(mut value: Value) -> Result<ProjectDocument, ProjectError> {
    value["schema_version"] = Value::from(PROJECT_SCHEMA_VERSION);
    let mut document: ProjectDocument = serde_json::from_value(value)
        .map_err(|error| ProjectError::Decode(format!("schema 2 migration: {error}")))?;
    document.provenance.push(ProvenanceRecord {
        id: "provenance-migrate-v2".to_owned(),
        operation: "migrate_schema_2_to_8".to_owned(),
        input_ids: Vec::new(),
        parameters: BTreeMap::new(),
    });
    finish_legacy_migration(document, 2)
}

pub(super) fn migrate_v3(mut value: Value) -> Result<ProjectDocument, ProjectError> {
    value["schema_version"] = Value::from(PROJECT_SCHEMA_VERSION);
    let mut document: ProjectDocument = serde_json::from_value(value)
        .map_err(|error| ProjectError::Decode(format!("schema 3 migration: {error}")))?;
    document.provenance.push(ProvenanceRecord {
        id: "provenance-migrate-v3".to_owned(),
        operation: "migrate_schema_3_to_8".to_owned(),
        input_ids: Vec::new(),
        parameters: BTreeMap::new(),
    });
    finish_legacy_migration(document, 3)
}

pub(super) fn migrate_v4(mut value: Value) -> Result<ProjectDocument, ProjectError> {
    value["schema_version"] = Value::from(PROJECT_SCHEMA_VERSION);
    let mut document: ProjectDocument = serde_json::from_value(value)
        .map_err(|error| ProjectError::Decode(format!("schema 4 migration: {error}")))?;
    document.provenance.push(ProvenanceRecord {
        id: "provenance-migrate-v4".to_owned(),
        operation: "migrate_schema_4_to_8".to_owned(),
        input_ids: Vec::new(),
        parameters: BTreeMap::new(),
    });
    finish_legacy_migration(document, 4)
}

pub(super) fn migrate_v5(mut value: Value) -> Result<ProjectDocument, ProjectError> {
    value["schema_version"] = Value::from(PROJECT_SCHEMA_VERSION);
    let mut document: ProjectDocument = serde_json::from_value(value)
        .map_err(|error| ProjectError::Decode(format!("schema 5 migration: {error}")))?;
    document.provenance.push(ProvenanceRecord {
        id: "provenance-migrate-v5".to_owned(),
        operation: "migrate_schema_5_to_8".to_owned(),
        input_ids: Vec::new(),
        parameters: BTreeMap::new(),
    });
    finish_legacy_migration(document, 5)
}

pub(super) fn migrate_v6(mut value: Value) -> Result<ProjectDocument, ProjectError> {
    value["schema_version"] = Value::from(PROJECT_SCHEMA_VERSION);
    let sources = value["data_sources"]
        .as_array_mut()
        .ok_or_else(|| ProjectError::Decode("schema 6 data sources are missing".to_owned()))?;
    for source in sources {
        let object = source
            .as_object_mut()
            .ok_or_else(|| ProjectError::Decode("schema 6 data source is invalid".to_owned()))?;
        let legacy_manual = object
            .get("origin_path")
            .and_then(Value::as_str)
            .is_some_and(|path| path.starts_with("manual-data/"));
        object.insert(
            "origin".to_owned(),
            Value::String(if legacy_manual {
                "legacy_manual".to_owned()
            } else {
                "imported".to_owned()
            }),
        );
    }
    let mut document: ProjectDocument = serde_json::from_value(value)
        .map_err(|error| ProjectError::Decode(format!("schema 6 migration: {error}")))?;
    document.provenance.push(ProvenanceRecord {
        id: "provenance-migrate-v6".to_owned(),
        operation: "migrate_schema_6_to_8".to_owned(),
        input_ids: Vec::new(),
        parameters: BTreeMap::new(),
    });
    finish_legacy_migration(document, 6)
}

fn finish_legacy_migration(
    mut document: ProjectDocument,
    source_schema: u32,
) -> Result<ProjectDocument, ProjectError> {
    let ambiguous = rebuild_legacy_series_groups(&mut document)?;
    if ambiguous {
        let record = ProvenanceRecord {
            id: format!("provenance-series-groups-v{source_schema}"),
            operation: "migrate_legacy_series_groups_with_ambiguity".to_owned(),
            input_ids: Vec::new(),
            parameters: BTreeMap::from([("source_schema".to_owned(), Value::from(source_schema))]),
        };
        let index = document.provenance.len().saturating_sub(1);
        document.provenance.insert(index, record);
    }
    Ok(document)
}

fn rebuild_legacy_series_groups(document: &mut ProjectDocument) -> Result<bool, ProjectError> {
    let Some(axes) = document.figure.axes.first_mut() else {
        return Err(ProjectError::Decode(
            "legacy project has no axes for series migration".to_owned(),
        ));
    };
    if !axes.series_groups.is_empty() {
        return Ok(false);
    }
    let data_artists = document
        .figure
        .artists
        .iter()
        .filter_map(|artist| {
            legacy_artist_binding(artist)
                .cloned()
                .map(|binding| (artist.id.clone(), artist.kind, binding))
        })
        .collect::<Vec<_>>();
    let legend_anchors = document
        .figure
        .artists
        .iter()
        .filter_map(|artist| match &artist.properties {
            ArtistProperties::Legend { entries, .. } => Some(entries.as_slice()),
            _ => None,
        })
        .flatten()
        .map(|entry| entry.artist_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut assigned = BTreeSet::new();
    let mut groups = Vec::<SeriesGroupRecord>::new();
    let mut used_group_ids = BTreeSet::new();
    let mut ambiguous = false;

    for (artist_id, _, _) in &data_artists {
        if legend_anchors.contains(artist_id.as_str()) {
            assigned.insert(artist_id.clone());
            groups.push(SeriesGroupRecord {
                id: legacy_group_id(artist_id, &mut used_group_ids),
                artist_ids: vec![artist_id.clone()],
                axes: AxisBinding::PRIMARY,
            });
        }
    }

    for (artist_id, kind, binding) in &data_artists {
        if assigned.contains(artist_id) {
            continue;
        }
        let candidates = groups
            .iter()
            .enumerate()
            .filter_map(|(index, group)| {
                let members = group
                    .artist_ids
                    .iter()
                    .filter_map(|member_id| {
                        data_artists
                            .iter()
                            .find(|(candidate_id, _, _)| candidate_id == member_id)
                    })
                    .collect::<Vec<_>>();
                let same_binding = members
                    .iter()
                    .any(|(_, _, candidate_binding)| candidate_binding == binding);
                let has_same_kind = members
                    .iter()
                    .any(|(_, candidate_kind, _)| candidate_kind == kind);
                (same_binding && !has_same_kind).then_some(index)
            })
            .collect::<Vec<_>>();
        if let [index] = candidates.as_slice() {
            groups[*index].artist_ids.push(artist_id.clone());
            assigned.insert(artist_id.clone());
        } else if candidates.len() > 1
            || groups.iter().any(|group| {
                group.artist_ids.iter().any(|member_id| {
                    data_artists
                        .iter()
                        .any(|(candidate_id, _, candidate_binding)| {
                            candidate_id == member_id && candidate_binding == binding
                        })
                })
            })
        {
            ambiguous = true;
        }
    }

    for (artist_id, _, binding) in &data_artists {
        if assigned.contains(artist_id) {
            continue;
        }
        let same_binding = data_artists
            .iter()
            .filter(|(candidate_id, _, candidate_binding)| {
                !assigned.contains(candidate_id) && candidate_binding == binding
            })
            .collect::<Vec<_>>();
        let mut unique_kinds = Vec::new();
        for (_, kind, _) in &same_binding {
            if !unique_kinds.contains(kind) {
                unique_kinds.push(*kind);
            }
        }
        if unique_kinds.len() == same_binding.len() {
            let member_ids = same_binding
                .iter()
                .map(|(candidate_id, _, _)| candidate_id.clone())
                .collect::<Vec<_>>();
            for member_id in &member_ids {
                assigned.insert(member_id.clone());
            }
            groups.push(SeriesGroupRecord {
                id: legacy_group_id(&member_ids[0], &mut used_group_ids),
                artist_ids: member_ids,
                axes: AxisBinding::PRIMARY,
            });
        } else {
            ambiguous = true;
            assigned.insert(artist_id.clone());
            groups.push(SeriesGroupRecord {
                id: legacy_group_id(artist_id, &mut used_group_ids),
                artist_ids: vec![artist_id.clone()],
                axes: AxisBinding::PRIMARY,
            });
        }
    }
    axes.series_groups = groups;
    Ok(ambiguous)
}

fn legacy_artist_binding(artist: &ArtistRecord) -> Option<&DataBinding> {
    match &artist.properties {
        ArtistProperties::Line { binding, .. }
        | ArtistProperties::Scatter { binding, .. }
        | ArtistProperties::ErrorBar { binding, .. } => Some(binding),
        _ => None,
    }
}

fn legacy_group_id(artist_id: &str, used: &mut BTreeSet<String>) -> String {
    let base = format!("legacy-series-group-{artist_id}");
    if used.insert(base.clone()) {
        return base;
    }
    (2_u64..)
        .map(|suffix| format!("{base}-{suffix}"))
        .find(|candidate| used.insert(candidate.clone()))
        .expect("legacy series group ID space is practically unbounded")
}
