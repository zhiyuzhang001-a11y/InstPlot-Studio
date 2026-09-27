use super::*;

pub fn save_project(path: &Path, document: &ProjectDocument) -> Result<(), ProjectError> {
    document.validate()?;
    let mut bytes = serde_json::to_vec_pretty(document)?;
    bytes.push(b'\n');
    if path.exists() {
        let old = fs::read(path)?;
        decode_project(&old)
            .map_err(|error| ProjectError::InvalidExistingProject(error.to_string()))?;
        atomic_write(&backup_path(path), &old)?;
    }
    atomic_write(path, &bytes)
}

pub fn open_project(path: &Path) -> Result<OpenProjectReport, ProjectError> {
    let primary = fs::read(path).and_then(|bytes| {
        decode_project(&bytes).map_err(|error| std::io::Error::other(error.to_string()))
    });
    match primary {
        Ok(document) => Ok(report_for(document, OpenProjectSource::Primary)),
        Err(primary_error) => {
            let backup_path = backup_path(path);
            let backup = fs::read(&backup_path).and_then(|bytes| {
                decode_project(&bytes).map_err(|error| std::io::Error::other(error.to_string()))
            });
            match backup {
                Ok(document) => {
                    let mut report = report_for(document, OpenProjectSource::Backup);
                    report.warnings.insert(
                        0,
                        format!(
                            "Primary project could not be opened; recovered read-only state from {}: {primary_error}",
                            backup_path.display()
                        ),
                    );
                    Ok(report)
                }
                Err(backup_error) => Err(ProjectError::RecoveryFailed {
                    primary: primary_error.to_string(),
                    backup: backup_error.to_string(),
                }),
            }
        }
    }
}

pub fn decode_project(bytes: &[u8]) -> Result<ProjectDocument, ProjectError> {
    let value: Value =
        serde_json::from_slice(bytes).map_err(|error| ProjectError::Decode(error.to_string()))?;
    let version_u64 = value
        .get("schema_version")
        .and_then(Value::as_u64)
        .ok_or_else(|| ProjectError::Decode("schema_version is missing or invalid".to_owned()))?;
    let version = u32::try_from(version_u64)
        .map_err(|_| ProjectError::Decode("schema_version exceeds u32".to_owned()))?;
    let document = match version {
        PROJECT_SCHEMA_VERSION => serde_json::from_value(value)
            .map_err(|error| ProjectError::Decode(error.to_string()))?,
        7 => migrate_v7(value)?,
        6 => migrate_v6(value)?,
        5 => migrate_v5(value)?,
        4 => migrate_v4(value)?,
        3 => migrate_v3(value)?,
        2 => migrate_v2(value)?,
        1 => migrate_v1(value)?,
        0 => migrate_v0(value)?,
        future => return Err(ProjectError::UnsupportedSchema(future)),
    };
    document.validate()?;
    Ok(document)
}

pub fn fingerprint(path: &Path) -> Result<SourceFingerprint, ProjectError> {
    let bytes = fs::read(path)?;
    Ok(SourceFingerprint {
        size_bytes: u64::try_from(bytes.len())
            .map_err(|_| ProjectError::Validation("source is too large".to_owned()))?,
        sha256: hex_digest(&bytes),
    })
}

pub(super) fn source_state(source: &DataSourceRecord) -> SourceState {
    let DataSourcePayload::External {
        path,
        fingerprint: expected,
    } = &source.payload
    else {
        return SourceState::Embedded;
    };
    match fingerprint(Path::new(path)) {
        Ok(actual) if &actual == expected => SourceState::Unchanged,
        Ok(actual) => SourceState::Changed {
            expected_sha256: expected.sha256.clone(),
            actual_sha256: actual.sha256,
        },
        Err(ProjectError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            SourceState::Missing
        }
        Err(error) => SourceState::Unavailable {
            error: error.to_string(),
        },
    }
}

fn report_for(document: ProjectDocument, source: OpenProjectSource) -> OpenProjectReport {
    let mut warnings = document
        .source_states()
        .into_iter()
        .filter_map(|(id, state)| match state {
            SourceState::Missing => Some(format!("External data source {id} is missing")),
            SourceState::Changed { .. } => Some(format!(
                "External data source {id} changed; stored data was not replaced"
            )),
            SourceState::Unavailable { error } => Some(format!(
                "External data source {id} could not be checked: {error}"
            )),
            SourceState::Unchanged | SourceState::Embedded => None,
        })
        .collect::<Vec<_>>();
    if document
        .provenance
        .iter()
        .any(|record| record.operation == "migrate_legacy_series_groups_with_ambiguity")
    {
        warnings.push(
            "Some legacy plot elements could not be grouped into logical series unambiguously; they were kept as separate series"
                .to_owned(),
        );
    }
    OpenProjectReport {
        document,
        source,
        warnings,
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), ProjectError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    if !parent.exists() {
        return Err(ProjectError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("parent directory does not exist: {}", parent.display()),
        )));
    }
    AtomicFile::new(path, AllowOverwrite)
        .write(|file| {
            file.write_all(bytes)?;
            file.sync_all()
        })
        .map_err(|error| ProjectError::AtomicWrite(error.to_string()))
}

fn backup_path(path: &Path) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(".bak");
    PathBuf::from(value)
}
