use core::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use atomicwrites::{AllowOverwrite, AtomicFile};
use instplot_core::{DataSet, DataSetKind, FitLink, NumericColumn};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{DocumentLayoutError, FigureDocument, ProjectError};

pub const HANDOFF_SCHEMA_VERSION: u32 = 1;
pub const HANDOFF_EXTENSION: &str = "instplot-handoff";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandoffCleanup {
    Keep,
    DeleteAfterImport,
}

#[derive(Debug)]
pub struct HandoffImport {
    pub document: FigureDocument,
    pub datasets: Vec<DataSet>,
    pub producer_name: String,
    pub producer_version: String,
    pub source_path: PathBuf,
    pub source_removed: bool,
}

#[derive(Debug)]
pub enum HandoffError {
    Io(std::io::Error),
    Encode(serde_json::Error),
    Decode(String),
    UnsupportedSchema(u32),
    ChecksumMismatch,
    InvalidExtension(PathBuf),
    UnsafeCleanup(PathBuf),
    Project(ProjectError),
    Layout(DocumentLayoutError),
}

impl fmt::Display for HandoffError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "handoff I/O: {error}"),
            Self::Encode(error) => write!(formatter, "encode handoff: {error}"),
            Self::Decode(error) => write!(formatter, "decode handoff: {error}"),
            Self::UnsupportedSchema(version) => {
                write!(formatter, "unsupported handoff schema {version}")
            }
            Self::ChecksumMismatch => formatter.write_str("handoff payload checksum mismatch"),
            Self::InvalidExtension(path) => write!(
                formatter,
                "handoff path must end in .{HANDOFF_EXTENSION}: {}",
                path.display()
            ),
            Self::UnsafeCleanup(path) => write!(
                formatter,
                "refusing to delete unsafe handoff path: {}",
                path.display()
            ),
            Self::Project(error) => write!(formatter, "create Figure Document: {error}"),
            Self::Layout(error) => write!(formatter, "layout handoff Figure Document: {error}"),
        }
    }
}

impl std::error::Error for HandoffError {}

impl From<std::io::Error> for HandoffError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for HandoffError {
    fn from(value: serde_json::Error) -> Self {
        Self::Encode(value)
    }
}

impl From<ProjectError> for HandoffError {
    fn from(value: ProjectError) -> Self {
        Self::Project(value)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HandoffEnvelope {
    schema_version: u32,
    payload_sha256: String,
    payload: HandoffPayload,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HandoffPayload {
    producer_name: String,
    producer_version: String,
    datasets: Vec<HandoffDataset>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HandoffDataset {
    id: String,
    label: Option<String>,
    source_identity: String,
    kind: HandoffDatasetKind,
    columns: Vec<HandoffColumn>,
    row_count: usize,
    alive: Vec<bool>,
    fit_link: Option<HandoffFitLink>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum HandoffDatasetKind {
    Source,
    Fit,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HandoffColumn {
    id: String,
    values: Vec<f64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HandoffFitLink {
    parent_id: String,
    source_x: String,
    source_y: String,
    equation: Option<String>,
    display_equation: Option<String>,
}

pub fn encode_handoff(
    datasets: &[DataSet],
    producer_name: &str,
    producer_version: &str,
) -> Result<Vec<u8>, HandoffError> {
    FigureDocument::from_datasets(datasets)?;
    if producer_name.trim().is_empty() || producer_version.trim().is_empty() {
        return Err(HandoffError::Decode(
            "handoff producer name and version must be present".to_owned(),
        ));
    }
    let payload = HandoffPayload {
        producer_name: producer_name.to_owned(),
        producer_version: producer_version.to_owned(),
        datasets: datasets.iter().map(HandoffDataset::from).collect(),
    };
    let payload_sha256 = digest(&serde_json::to_vec(&payload)?);
    let envelope = HandoffEnvelope {
        schema_version: HANDOFF_SCHEMA_VERSION,
        payload_sha256,
        payload,
    };
    let mut bytes = serde_json::to_vec_pretty(&envelope)?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub fn write_handoff(
    path: &Path,
    datasets: &[DataSet],
    producer_name: &str,
    producer_version: &str,
) -> Result<usize, HandoffError> {
    validate_extension(path)?;
    let bytes = encode_handoff(datasets, producer_name, producer_version)?;
    AtomicFile::new(path, AllowOverwrite)
        .write(|file| file.write_all(&bytes).and_then(|_| file.sync_all()))
        .map_err(|error| HandoffError::Io(error.into()))?;
    Ok(bytes.len())
}

pub fn import_handoff(path: &Path, cleanup: HandoffCleanup) -> Result<HandoffImport, HandoffError> {
    validate_extension(path)?;
    if cleanup == HandoffCleanup::DeleteAfterImport {
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
            return Err(HandoffError::UnsafeCleanup(path.to_path_buf()));
        }
    }
    let bytes = fs::read(path)?;
    let envelope: HandoffEnvelope =
        serde_json::from_slice(&bytes).map_err(|error| HandoffError::Decode(error.to_string()))?;
    if envelope.schema_version != HANDOFF_SCHEMA_VERSION {
        return Err(HandoffError::UnsupportedSchema(envelope.schema_version));
    }
    let actual = digest(&compact_payload_bytes(&bytes)?);
    if actual != envelope.payload_sha256 {
        return Err(HandoffError::ChecksumMismatch);
    }
    let datasets = envelope
        .payload
        .datasets
        .iter()
        .map(DataSet::from)
        .collect::<Vec<_>>();
    let document = FigureDocument::from_datasets(&datasets)?;
    document.layout_figure().map_err(HandoffError::Layout)?;
    let source_removed = cleanup == HandoffCleanup::DeleteAfterImport;
    if source_removed {
        fs::remove_file(path)?;
    }
    Ok(HandoffImport {
        document,
        datasets,
        producer_name: envelope.payload.producer_name,
        producer_version: envelope.payload.producer_version,
        source_path: path.to_path_buf(),
        source_removed,
    })
}

fn compact_payload_bytes(envelope: &[u8]) -> Result<Vec<u8>, HandoffError> {
    let key = b"\"payload\"";
    let key_at = envelope
        .windows(key.len())
        .position(|window| window == key)
        .ok_or_else(|| HandoffError::Decode("handoff payload is missing".to_owned()))?;
    let mut at = key_at + key.len();
    while envelope.get(at).is_some_and(u8::is_ascii_whitespace) {
        at += 1;
    }
    if envelope.get(at) != Some(&b':') {
        return Err(HandoffError::Decode(
            "handoff payload key has no value".to_owned(),
        ));
    }
    at += 1;
    while envelope.get(at).is_some_and(u8::is_ascii_whitespace) {
        at += 1;
    }
    if !matches!(envelope.get(at), Some(b'{') | Some(b'[')) {
        return Err(HandoffError::Decode(
            "handoff payload must be a JSON object or array".to_owned(),
        ));
    }

    let mut compact = Vec::new();
    let mut depth = 0_usize;
    let mut in_string = false;
    let mut escaped = false;
    for &byte in &envelope[at..] {
        if in_string {
            compact.push(byte);
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => {
                in_string = true;
                compact.push(byte);
            }
            b'{' | b'[' => {
                depth += 1;
                compact.push(byte);
            }
            b'}' | b']' => {
                if depth == 0 {
                    return Err(HandoffError::Decode(
                        "handoff payload delimiters are unbalanced".to_owned(),
                    ));
                }
                depth -= 1;
                compact.push(byte);
                if depth == 0 {
                    return Ok(compact);
                }
            }
            byte if byte.is_ascii_whitespace() => {}
            _ => compact.push(byte),
        }
    }
    Err(HandoffError::Decode(
        "handoff payload is incomplete".to_owned(),
    ))
}

impl From<&DataSet> for HandoffDataset {
    fn from(dataset: &DataSet) -> Self {
        Self {
            id: dataset.plot_id.clone(),
            label: dataset.label.clone(),
            source_identity: dataset.source.to_string_lossy().into_owned(),
            kind: match dataset.kind {
                DataSetKind::Source => HandoffDatasetKind::Source,
                DataSetKind::Fit => HandoffDatasetKind::Fit,
            },
            columns: dataset
                .columns
                .iter()
                .map(|column| HandoffColumn {
                    id: column.name.clone(),
                    values: column.values.clone(),
                })
                .collect(),
            row_count: dataset.row_count,
            alive: dataset.alive.clone(),
            fit_link: dataset.fit_link.as_ref().map(|link| HandoffFitLink {
                parent_id: link.parent_dataset_id.clone().unwrap_or_default(),
                source_x: link.source_x_column.clone(),
                source_y: link.source_y_column.clone(),
                equation: link.equation.clone(),
                display_equation: link.display_equation.clone(),
            }),
        }
    }
}

impl From<&HandoffDataset> for DataSet {
    fn from(dataset: &HandoffDataset) -> Self {
        Self {
            source: PathBuf::from(&dataset.source_identity),
            label: dataset.label.clone(),
            kind: match dataset.kind {
                HandoffDatasetKind::Source => DataSetKind::Source,
                HandoffDatasetKind::Fit => DataSetKind::Fit,
            },
            plot_id: dataset.id.clone(),
            fit_link: dataset.fit_link.as_ref().map(|link| FitLink {
                parent_dataset_id: Some(link.parent_id.clone()),
                source_x_column: link.source_x.clone(),
                source_y_column: link.source_y.clone(),
                equation: link.equation.clone(),
                display_equation: link.display_equation.clone(),
            }),
            encoding: "handoff-json-v1".to_owned(),
            separator: "embedded".to_owned(),
            columns: dataset
                .columns
                .iter()
                .map(|column| NumericColumn {
                    name: column.id.clone(),
                    values: column.values.clone(),
                })
                .collect(),
            row_count: dataset.row_count,
            alive: dataset.alive.clone(),
        }
    }
}

fn validate_extension(path: &Path) -> Result<(), HandoffError> {
    if path.extension().and_then(|value| value.to_str()) == Some(HANDOFF_EXTENSION) {
        Ok(())
    } else {
        Err(HandoffError::InvalidExtension(path.to_path_buf()))
    }
}

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(1);

    fn datasets() -> Vec<DataSet> {
        let source = DataSet {
            source: PathBuf::from("measurement.txt"),
            label: Some("Measurement".to_owned()),
            kind: DataSetKind::Source,
            plot_id: "source-a".to_owned(),
            fit_link: None,
            encoding: "UTF-8".to_owned(),
            separator: "tab".to_owned(),
            columns: vec![
                NumericColumn {
                    name: "field".to_owned(),
                    values: vec![0.0, 1.0, 2.0],
                },
                NumericColumn {
                    name: "signal".to_owned(),
                    values: vec![1.0, 2.0, 3.0],
                },
            ],
            row_count: 3,
            alive: vec![true, false, true],
        };
        let fit = DataSet {
            source: PathBuf::from("measurement.txt"),
            label: Some("Linear fit".to_owned()),
            kind: DataSetKind::Fit,
            plot_id: "fit-a".to_owned(),
            fit_link: Some(FitLink {
                parent_dataset_id: Some("source-a".to_owned()),
                source_x_column: "field".to_owned(),
                source_y_column: "signal".to_owned(),
                equation: Some("y=1+x".to_owned()),
                display_equation: Some("y = 1 + x".to_owned()),
            }),
            encoding: "UTF-8".to_owned(),
            separator: "tab".to_owned(),
            columns: vec![
                NumericColumn {
                    name: "fit_x".to_owned(),
                    values: vec![0.0, 1.0, 2.0],
                },
                NumericColumn {
                    name: "fit_y".to_owned(),
                    values: vec![1.0, 2.0, 3.0],
                },
            ],
            row_count: 3,
            alive: vec![true, true, true],
        };
        vec![source, fit]
    }

    fn temp_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "instplot-handoff-{}-{}.{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed),
            HANDOFF_EXTENSION
        ))
    }

    #[test]
    fn handoff_round_trip_is_lossless_and_creates_embedded_document() {
        let path = temp_path();
        let expected = datasets();
        write_handoff(&path, &expected, "InstPlot Lite", "0.1.0").unwrap();
        let imported = import_handoff(&path, HandoffCleanup::Keep).unwrap();
        assert_eq!(imported.producer_name, "InstPlot Lite");
        assert_eq!(imported.datasets.len(), expected.len());
        assert_eq!(imported.datasets[0].alive, expected[0].alive);
        assert_eq!(imported.datasets[0].columns[1].values, [1.0, 2.0, 3.0]);
        assert_eq!(
            imported.datasets[1]
                .fit_link
                .as_ref()
                .unwrap()
                .display_equation
                .as_deref(),
            Some("y = 1 + x")
        );
        assert!(imported.document.layout_figure().is_ok());
        assert!(
            imported
                .document
                .project()
                .data_sources
                .iter()
                .all(|source| {
                    matches!(source.payload, crate::DataSourcePayload::Embedded { .. })
                })
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn checksum_round_trip_preserves_high_precision_measurements() {
        let path = temp_path();
        let mut expected = datasets();
        expected[0].columns[0].values = vec![0.1, 2.1019019019019023, 4.103803803803804];
        write_handoff(&path, &expected, "InstPlot Lite", "0.1.0").unwrap();
        let imported = import_handoff(&path, HandoffCleanup::Keep).unwrap();
        assert_eq!(
            imported.datasets[0].columns[0].values,
            expected[0].columns[0].values
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn consume_removes_only_the_validated_handoff_file_after_success() {
        let path = temp_path();
        write_handoff(&path, &datasets(), "InstPlot Lite", "0.1.0").unwrap();
        let imported = import_handoff(&path, HandoffCleanup::DeleteAfterImport).unwrap();
        assert!(imported.source_removed);
        assert!(!path.exists());
        let project_path = path.with_extension("instplot");
        imported.document.save(&project_path).unwrap();
        let (reopened, _) = FigureDocument::open(&project_path).unwrap();
        assert!(reopened.layout_figure().is_ok());
        fs::remove_file(project_path).unwrap();
    }

    #[test]
    fn checksum_and_explicit_parent_identity_are_enforced() {
        let mut bytes = encode_handoff(&datasets(), "InstPlot Lite", "0.1.0").unwrap();
        let needle = b"Measurement";
        let position = bytes
            .windows(needle.len())
            .position(|window| window == needle)
            .unwrap();
        bytes[position] = b'X';
        let path = temp_path();
        fs::write(&path, bytes).unwrap();
        assert!(matches!(
            import_handoff(&path, HandoffCleanup::Keep),
            Err(HandoffError::ChecksumMismatch)
        ));
        fs::remove_file(&path).unwrap();

        let mut invalid = datasets();
        invalid[1].fit_link.as_mut().unwrap().parent_dataset_id =
            Some("guessed-from-filename".to_owned());
        assert!(encode_handoff(&invalid, "InstPlot Lite", "0.1.0").is_err());
    }
}
