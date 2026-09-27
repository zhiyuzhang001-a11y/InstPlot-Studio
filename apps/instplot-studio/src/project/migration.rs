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
        operation: "migrate_schema_0_to_7".to_owned(),
        input_ids: vec![legacy.producer_version],
        parameters: BTreeMap::new(),
    });
    Ok(document)
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
        operation: "migrate_schema_1_to_7".to_owned(),
        input_ids: Vec::new(),
        parameters: BTreeMap::new(),
    });
    Ok(document)
}

pub(super) fn migrate_v2(mut value: Value) -> Result<ProjectDocument, ProjectError> {
    value["schema_version"] = Value::from(PROJECT_SCHEMA_VERSION);
    let mut document: ProjectDocument = serde_json::from_value(value)
        .map_err(|error| ProjectError::Decode(format!("schema 2 migration: {error}")))?;
    document.provenance.push(ProvenanceRecord {
        id: "provenance-migrate-v2".to_owned(),
        operation: "migrate_schema_2_to_7".to_owned(),
        input_ids: Vec::new(),
        parameters: BTreeMap::new(),
    });
    Ok(document)
}

pub(super) fn migrate_v3(mut value: Value) -> Result<ProjectDocument, ProjectError> {
    value["schema_version"] = Value::from(PROJECT_SCHEMA_VERSION);
    let mut document: ProjectDocument = serde_json::from_value(value)
        .map_err(|error| ProjectError::Decode(format!("schema 3 migration: {error}")))?;
    document.provenance.push(ProvenanceRecord {
        id: "provenance-migrate-v3".to_owned(),
        operation: "migrate_schema_3_to_7".to_owned(),
        input_ids: Vec::new(),
        parameters: BTreeMap::new(),
    });
    Ok(document)
}

pub(super) fn migrate_v4(mut value: Value) -> Result<ProjectDocument, ProjectError> {
    value["schema_version"] = Value::from(PROJECT_SCHEMA_VERSION);
    let mut document: ProjectDocument = serde_json::from_value(value)
        .map_err(|error| ProjectError::Decode(format!("schema 4 migration: {error}")))?;
    document.provenance.push(ProvenanceRecord {
        id: "provenance-migrate-v4".to_owned(),
        operation: "migrate_schema_4_to_7".to_owned(),
        input_ids: Vec::new(),
        parameters: BTreeMap::new(),
    });
    Ok(document)
}

pub(super) fn migrate_v5(mut value: Value) -> Result<ProjectDocument, ProjectError> {
    value["schema_version"] = Value::from(PROJECT_SCHEMA_VERSION);
    let mut document: ProjectDocument = serde_json::from_value(value)
        .map_err(|error| ProjectError::Decode(format!("schema 5 migration: {error}")))?;
    document.provenance.push(ProvenanceRecord {
        id: "provenance-migrate-v5".to_owned(),
        operation: "migrate_schema_5_to_7".to_owned(),
        input_ids: Vec::new(),
        parameters: BTreeMap::new(),
    });
    Ok(document)
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
        operation: "migrate_schema_6_to_7".to_owned(),
        input_ids: Vec::new(),
        parameters: BTreeMap::new(),
    });
    Ok(document)
}
