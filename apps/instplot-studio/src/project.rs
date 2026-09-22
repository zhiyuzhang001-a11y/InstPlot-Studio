use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use atomicwrites::{AllowOverwrite, AtomicFile};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const PROJECT_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectDocument {
    pub schema_version: u32,
    pub producer_version: String,
    pub figure: FigureRecord,
    pub data_sources: Vec<DataSourceRecord>,
    pub semantic_registry: Vec<SemanticLabel>,
    pub palette: PaletteRegistry,
    pub typography: TypographyProfile,
    pub overrides: Vec<OverrideRecord>,
    pub export_preferences: ExportPreferences,
    pub provenance: Vec<ProvenanceRecord>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FigureRecord {
    pub id: String,
    pub width_mm: f64,
    pub height_mm: f64,
    pub axes: Vec<AxesRecord>,
    pub artists: Vec<ArtistRecord>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AxesRecord {
    pub id: String,
    pub x: AxisRecord,
    pub y: AxisRecord,
    pub artist_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AxisRecord {
    pub id: String,
    pub label_id: String,
    pub minimum: f64,
    pub maximum: f64,
    pub scale: AxisScale,
    pub locator: LocatorSpec,
    pub formatter: FormatterSpec,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AxisScale {
    Linear,
    Log10,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LocatorSpec {
    Auto { target_count: u8 },
    Fixed { values: Vec<f64> },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum FormatterSpec {
    Auto,
    Decimal { precision: u8 },
    Scientific { precision: u8 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtistRecord {
    pub id: String,
    pub kind: ArtistKind,
    pub role: ArtistRole,
    pub properties: ArtistProperties,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtistKind {
    Line,
    Scatter,
    ErrorBar,
    ReferenceLine,
    Annotation,
    Legend,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtistRole {
    Data,
    Fit,
    Theory,
    Reference,
    Baseline,
    Annotation,
    Legend,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ArtistProperties {
    Line {
        binding: DataBinding,
        stroke: StrokeStyle,
    },
    Scatter {
        binding: DataBinding,
        marker: MarkerStyle,
    },
    ErrorBar {
        binding: DataBinding,
        y_error_column: String,
        cap_width_pt: f64,
        stroke: StrokeStyle,
    },
    ReferenceLine {
        orientation: ReferenceOrientation,
        value: f64,
        stroke: StrokeStyle,
    },
    Annotation {
        label_id: String,
        x_pt: f64,
        y_pt: f64,
    },
    Legend {
        entries: Vec<LegendEntry>,
        x_pt: f64,
        y_pt: f64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataBinding {
    pub data_source_id: String,
    pub x_column: String,
    pub y_column: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StrokeStyle {
    pub color_id: String,
    pub width_pt: f64,
    pub dash_pt: Vec<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarkerStyle {
    pub color_id: String,
    pub shape: MarkerShape,
    pub size_pt: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkerShape {
    Circle,
    Square,
    Triangle,
    Diamond,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceOrientation {
    Horizontal,
    Vertical,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegendEntry {
    pub artist_id: String,
    pub label_id: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataSourceRecord {
    pub id: String,
    pub label: String,
    pub kind: DataSourceKind,
    pub payload: DataSourcePayload,
    pub fit: Option<FitIdentity>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataSourceKind {
    Source,
    Fit,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "storage", rename_all = "snake_case", deny_unknown_fields)]
pub enum DataSourcePayload {
    External {
        path: String,
        fingerprint: SourceFingerprint,
    },
    Embedded {
        columns: Vec<EmbeddedColumn>,
        row_count: usize,
        #[serde(default)]
        alive: Vec<bool>,
        sha256: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceFingerprint {
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmbeddedColumn {
    pub name: String,
    pub values: Vec<f64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FitIdentity {
    pub parent_data_source_id: String,
    pub source_x_column: String,
    pub source_y_column: String,
    pub equation: Option<String>,
    #[serde(default)]
    pub display_equation: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticLabel {
    pub id: String,
    pub nodes: Vec<LabelNode>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum LabelNode {
    Text(String),
    Variable(String),
    Upright(String),
    GreekVariable(char),
    Number(String),
    DescriptiveSubscript(Vec<LabelNode>),
    VariableSubscript(Vec<LabelNode>),
    Superscript(Vec<LabelNode>),
    Unit(String),
    UnitSeparator,
    Operator(String),
    Emphasis(String),
    BoldVariable(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaletteRegistry {
    pub id: String,
    pub colors: Vec<PaletteColor>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaletteColor {
    pub id: String,
    pub rgba: [u8; 4],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypographyProfile {
    pub id: String,
    pub family: String,
    pub font_version: String,
    pub faces: Vec<FontFaceRecord>,
    pub subscript_scale: f64,
    pub superscript_scale: f64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FontFaceRecord {
    pub style: FontStyle,
    pub postscript_name: String,
    pub sha256: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FontStyle {
    Regular,
    Italic,
    Bold,
    BoldItalic,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OverrideRecord {
    pub target_id: String,
    pub property: String,
    pub value: Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportPreferences {
    pub vector_format: String,
    pub raster_dpi: Vec<u32>,
    pub transparent_background: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvenanceRecord {
    pub id: String,
    pub operation: String,
    pub input_ids: Vec<String>,
    pub parameters: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceState {
    Unchanged,
    Missing,
    Unavailable {
        error: String,
    },
    Changed {
        expected_sha256: String,
        actual_sha256: String,
    },
    Embedded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenProjectSource {
    Primary,
    Backup,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OpenProjectReport {
    pub document: ProjectDocument,
    pub source: OpenProjectSource,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum ProjectError {
    Io(std::io::Error),
    Encode(serde_json::Error),
    Decode(String),
    Validation(String),
    UnsupportedSchema(u32),
    AtomicWrite(String),
    InvalidExistingProject(String),
    RecoveryFailed { primary: String, backup: String },
    NonUtf8Path(PathBuf),
}

impl fmt::Display for ProjectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "project I/O: {error}"),
            Self::Encode(error) => write!(formatter, "encode project: {error}"),
            Self::Decode(error) => write!(formatter, "decode project: {error}"),
            Self::Validation(error) => write!(formatter, "invalid project: {error}"),
            Self::UnsupportedSchema(version) => {
                write!(formatter, "unsupported future project schema {version}")
            }
            Self::AtomicWrite(error) => write!(formatter, "atomic project write: {error}"),
            Self::InvalidExistingProject(error) => {
                write!(formatter, "refusing to overwrite invalid project: {error}")
            }
            Self::RecoveryFailed { primary, backup } => {
                write!(
                    formatter,
                    "primary project failed ({primary}); backup failed ({backup})"
                )
            }
            Self::NonUtf8Path(path) => {
                write!(
                    formatter,
                    "project paths must be Unicode: {}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for ProjectError {}

impl From<std::io::Error> for ProjectError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for ProjectError {
    fn from(value: serde_json::Error) -> Self {
        Self::Encode(value)
    }
}

impl ProjectDocument {
    pub fn fixed_fixture() -> Self {
        let artist_ids = [10_u64, 11, 12, 13, 14, 15].map(|id| format!("node-{id}"));
        let embedded_columns = vec![
            EmbeddedColumn {
                name: "line_x".to_owned(),
                values: vec![-3.2, 0.0, 3.2],
            },
            EmbeddedColumn {
                name: "line_y".to_owned(),
                values: vec![-2.34, 0.0, 2.34],
            },
            EmbeddedColumn {
                name: "scatter_x".to_owned(),
                values: vec![-3.2, 0.0, 3.2],
            },
            EmbeddedColumn {
                name: "scatter_y".to_owned(),
                values: vec![-2.38, 0.01, 2.34],
            },
            EmbeddedColumn {
                name: "error".to_owned(),
                values: vec![0.0, 0.08, 0.0],
            },
        ];
        let embedded_alive = vec![true; 3];
        let embedded_sha256 = embedded_digest(&embedded_columns, 3, &embedded_alive)
            .expect("the fixed project fixture has serializable embedded data");
        let blue_stroke = || StrokeStyle {
            color_id: "blue".to_owned(),
            width_pt: 0.9,
            dash_pt: Vec::new(),
        };
        Self {
            schema_version: PROJECT_SCHEMA_VERSION,
            producer_version: env!("CARGO_PKG_VERSION").to_owned(),
            figure: FigureRecord {
                id: "node-1".to_owned(),
                width_mm: 89.0,
                height_mm: 65.0,
                axes: vec![AxesRecord {
                    id: "node-2".to_owned(),
                    x: AxisRecord {
                        id: "node-3".to_owned(),
                        label_id: "label-x".to_owned(),
                        minimum: -3.0,
                        maximum: 3.0,
                        scale: AxisScale::Linear,
                        locator: LocatorSpec::Auto { target_count: 6 },
                        formatter: FormatterSpec::Auto,
                    },
                    y: AxisRecord {
                        id: "node-4".to_owned(),
                        label_id: "label-y".to_owned(),
                        minimum: -2.5,
                        maximum: 2.5,
                        scale: AxisScale::Linear,
                        locator: LocatorSpec::Auto { target_count: 6 },
                        formatter: FormatterSpec::Auto,
                    },
                    artist_ids: artist_ids.to_vec(),
                }],
                artists: vec![
                    ArtistRecord {
                        id: artist_ids[0].clone(),
                        kind: ArtistKind::ReferenceLine,
                        role: ArtistRole::Baseline,
                        properties: ArtistProperties::ReferenceLine {
                            orientation: ReferenceOrientation::Horizontal,
                            value: 0.0,
                            stroke: StrokeStyle {
                                color_id: "gray".to_owned(),
                                width_pt: 0.7,
                                dash_pt: vec![1.4, 1.4],
                            },
                        },
                    },
                    ArtistRecord {
                        id: artist_ids[1].clone(),
                        kind: ArtistKind::Line,
                        role: ArtistRole::Fit,
                        properties: ArtistProperties::Line {
                            binding: binding("fixture-data", "line_x", "line_y"),
                            stroke: blue_stroke(),
                        },
                    },
                    ArtistRecord {
                        id: artist_ids[2].clone(),
                        kind: ArtistKind::ErrorBar,
                        role: ArtistRole::Data,
                        properties: ArtistProperties::ErrorBar {
                            binding: binding("fixture-data", "scatter_x", "scatter_y"),
                            y_error_column: "error".to_owned(),
                            cap_width_pt: 4.0,
                            stroke: StrokeStyle {
                                width_pt: 0.7,
                                ..blue_stroke()
                            },
                        },
                    },
                    ArtistRecord {
                        id: artist_ids[3].clone(),
                        kind: ArtistKind::Scatter,
                        role: ArtistRole::Data,
                        properties: ArtistProperties::Scatter {
                            binding: binding("fixture-data", "scatter_x", "scatter_y"),
                            marker: MarkerStyle {
                                color_id: "blue".to_owned(),
                                shape: MarkerShape::Circle,
                                size_pt: 2.0,
                            },
                        },
                    },
                    ArtistRecord {
                        id: artist_ids[4].clone(),
                        kind: ArtistKind::Annotation,
                        role: ArtistRole::Annotation,
                        properties: ArtistProperties::Annotation {
                            label_id: "label-temperature".to_owned(),
                            x_pt: 48.0,
                            y_pt: 28.0,
                        },
                    },
                    ArtistRecord {
                        id: artist_ids[5].clone(),
                        kind: ArtistKind::Legend,
                        role: ArtistRole::Legend,
                        properties: ArtistProperties::Legend {
                            entries: vec![
                                LegendEntry {
                                    artist_id: artist_ids[3].clone(),
                                    label_id: "label-experiment".to_owned(),
                                },
                                LegendEntry {
                                    artist_id: artist_ids[1].clone(),
                                    label_id: "label-fit".to_owned(),
                                },
                            ],
                            x_pt: 164.0,
                            y_pt: 30.0,
                        },
                    },
                ],
            },
            data_sources: vec![DataSourceRecord {
                id: "fixture-data".to_owned(),
                label: "B2 fixed publication fixture".to_owned(),
                kind: DataSourceKind::Source,
                payload: DataSourcePayload::Embedded {
                    columns: embedded_columns,
                    row_count: 3,
                    alive: embedded_alive,
                    sha256: embedded_sha256,
                },
                fit: None,
            }],
            semantic_registry: vec![
                SemanticLabel {
                    id: "label-x".to_owned(),
                    nodes: vec![
                        LabelNode::GreekVariable('μ'),
                        LabelNode::VariableSubscript(vec![LabelNode::Number("0".to_owned())]),
                        LabelNode::Variable("H".to_owned()),
                        LabelNode::DescriptiveSubscript(vec![LabelNode::Text("DL".to_owned())]),
                        LabelNode::Text(" (".to_owned()),
                        LabelNode::Unit("mT".to_owned()),
                        LabelNode::Text(")".to_owned()),
                    ],
                },
                SemanticLabel {
                    id: "label-temperature".to_owned(),
                    nodes: vec![
                        LabelNode::Variable("T".to_owned()),
                        LabelNode::Text(" ".to_owned()),
                        LabelNode::Operator("≤".to_owned()),
                        LabelNode::Text(" ".to_owned()),
                        LabelNode::Number("300".to_owned()),
                        LabelNode::Text(" ".to_owned()),
                        LabelNode::Unit("K".to_owned()),
                    ],
                },
                SemanticLabel {
                    id: "label-experiment".to_owned(),
                    nodes: vec![LabelNode::Text("Experiment".to_owned())],
                },
                SemanticLabel {
                    id: "label-fit".to_owned(),
                    nodes: vec![LabelNode::Text("Fit".to_owned())],
                },
                SemanticLabel {
                    id: "label-y".to_owned(),
                    nodes: vec![
                        LabelNode::Text("Current density ".to_owned()),
                        LabelNode::Variable("J".to_owned()),
                        LabelNode::VariableSubscript(vec![LabelNode::Variable("e".to_owned())]),
                        LabelNode::Text(" (".to_owned()),
                        LabelNode::Unit("A".to_owned()),
                        LabelNode::UnitSeparator,
                        LabelNode::Unit("m".to_owned()),
                        LabelNode::Superscript(vec![LabelNode::Number("−2".to_owned())]),
                        LabelNode::Text(")".to_owned()),
                    ],
                },
            ],
            palette: PaletteRegistry {
                id: "publication-default-v1".to_owned(),
                colors: vec![
                    PaletteColor {
                        id: "blue".to_owned(),
                        rgba: [68, 119, 170, 255],
                    },
                    PaletteColor {
                        id: "gray".to_owned(),
                        rgba: [102, 102, 102, 255],
                    },
                ],
            },
            typography: default_typography(),
            overrides: Vec::new(),
            export_preferences: ExportPreferences {
                vector_format: "pdf".to_owned(),
                raster_dpi: vec![300, 600, 1200],
                transparent_background: false,
            },
            provenance: vec![ProvenanceRecord {
                id: "provenance-create".to_owned(),
                operation: "create_fixed_fixture".to_owned(),
                input_ids: Vec::new(),
                parameters: BTreeMap::new(),
            }],
        }
    }

    pub fn validate(&self) -> Result<(), ProjectError> {
        if self.schema_version != PROJECT_SCHEMA_VERSION {
            return Err(ProjectError::UnsupportedSchema(self.schema_version));
        }
        if self.producer_version.trim().is_empty() {
            return Err(ProjectError::Validation(
                "producer_version is empty".to_owned(),
            ));
        }
        validate_finite_positive("figure width", self.figure.width_mm)?;
        validate_finite_positive("figure height", self.figure.height_mm)?;
        if self.figure.axes.len() != 1 {
            return Err(ProjectError::Validation(
                "V1 projects must contain exactly one axes".to_owned(),
            ));
        }

        let mut ids = BTreeSet::new();
        insert_id(&mut ids, &self.figure.id)?;
        let labels = collect_ids(
            self.semantic_registry.iter().map(|label| label.id.as_str()),
            "semantic label",
        )?;
        if self
            .semantic_registry
            .iter()
            .any(|label| label.nodes.is_empty())
        {
            return Err(ProjectError::Validation(
                "semantic labels must contain at least one node".to_owned(),
            ));
        }
        let sources = collect_ids(
            self.data_sources.iter().map(|source| source.id.as_str()),
            "data source",
        )?;
        let artists = collect_ids(
            self.figure.artists.iter().map(|artist| artist.id.as_str()),
            "artist",
        )?;
        let palette_colors = collect_ids(
            self.palette.colors.iter().map(|color| color.id.as_str()),
            "palette color",
        )?;
        for artist_id in &artists {
            insert_id(&mut ids, artist_id)?;
        }
        for axes in &self.figure.axes {
            insert_id(&mut ids, &axes.id)?;
            insert_id(&mut ids, &axes.x.id)?;
            insert_id(&mut ids, &axes.y.id)?;
            validate_axis(&axes.x, &labels)?;
            validate_axis(&axes.y, &labels)?;
            let mut axes_artists = BTreeSet::new();
            for artist_id in &axes.artist_ids {
                if !artists.contains(artist_id) {
                    return Err(ProjectError::Validation(format!(
                        "axes {} references unknown artist {artist_id}",
                        axes.id
                    )));
                }
                if !axes_artists.insert(artist_id) {
                    return Err(ProjectError::Validation(format!(
                        "axes {} references artist {artist_id} more than once",
                        axes.id
                    )));
                }
            }
            if axes_artists.len() != artists.len() {
                return Err(ProjectError::Validation(format!(
                    "axes {} must reference every V1 artist exactly once",
                    axes.id
                )));
            }
        }
        for artist in &self.figure.artists {
            validate_artist(artist, &sources, &labels, &artists, &palette_colors)?;
        }
        for source in &self.data_sources {
            if source.label.trim().is_empty() {
                return Err(ProjectError::Validation(format!(
                    "data source {} has an empty label",
                    source.id
                )));
            }
            match (&source.kind, &source.fit) {
                (DataSourceKind::Source, Some(_)) => {
                    return Err(ProjectError::Validation(format!(
                        "source {} unexpectedly contains fit identity",
                        source.id
                    )));
                }
                (DataSourceKind::Fit, None) => {
                    return Err(ProjectError::Validation(format!(
                        "fit {} is missing fit identity",
                        source.id
                    )));
                }
                (DataSourceKind::Fit, Some(fit))
                    if !sources.contains(&fit.parent_data_source_id) =>
                {
                    return Err(ProjectError::Validation(format!(
                        "fit {} references unknown parent {}",
                        source.id, fit.parent_data_source_id
                    )));
                }
                (DataSourceKind::Fit, Some(fit))
                    if fit.source_x_column.trim().is_empty()
                        || fit.source_y_column.trim().is_empty() =>
                {
                    return Err(ProjectError::Validation(format!(
                        "fit {} has empty source-column identity",
                        source.id
                    )));
                }
                _ => {}
            }
            validate_payload(source)?;
        }
        collect_ids(
            self.provenance.iter().map(|record| record.id.as_str()),
            "provenance",
        )?;
        if self
            .provenance
            .iter()
            .any(|record| record.operation.trim().is_empty())
        {
            return Err(ProjectError::Validation(
                "provenance operation is empty".to_owned(),
            ));
        }
        let mut override_targets = ids.clone();
        override_targets.extend(labels.iter().cloned());
        override_targets.extend(sources.iter().cloned());
        for override_record in &self.overrides {
            if !override_targets.contains(&override_record.target_id) {
                return Err(ProjectError::Validation(format!(
                    "override references unknown target {}",
                    override_record.target_id
                )));
            }
            if override_record.property.trim().is_empty() {
                return Err(ProjectError::Validation(
                    "override property is empty".to_owned(),
                ));
            }
            if override_record.property.starts_with("publication_check:")
                && override_record
                    .value
                    .get("reason")
                    .and_then(Value::as_str)
                    .is_none_or(|reason| reason.trim().is_empty())
            {
                return Err(ProjectError::Validation(
                    "publication-check overrides must record a non-empty reason".to_owned(),
                ));
            }
        }
        if self.typography.id.trim().is_empty()
            || self.typography.font_version.trim().is_empty()
            || self.typography.family != "TeX Gyre Heros"
            || self.typography.faces.len() != 4
        {
            return Err(ProjectError::Validation(
                "typography must record all four TeX Gyre Heros faces".to_owned(),
            ));
        }
        validate_scale("subscript scale", self.typography.subscript_scale)?;
        validate_scale("superscript scale", self.typography.superscript_scale)?;
        let styles = self
            .typography
            .faces
            .iter()
            .map(|face| face.style)
            .collect::<BTreeSet<_>>();
        if styles.len() != 4
            || self
                .typography
                .faces
                .iter()
                .any(|face| face.sha256.len() != 64 || face.postscript_name.is_empty())
        {
            return Err(ProjectError::Validation(
                "typography face identities are incomplete".to_owned(),
            ));
        }
        if self.palette.id.trim().is_empty()
            || self.export_preferences.raster_dpi.is_empty()
            || self.export_preferences.raster_dpi.contains(&0)
        {
            return Err(ProjectError::Validation(
                "palette identity and raster export DPI must be defined".to_owned(),
            ));
        }
        if self.export_preferences.vector_format != "pdf" {
            return Err(ProjectError::Validation(
                "V1 vector export preference must be pdf".to_owned(),
            ));
        }
        Ok(())
    }

    pub fn upsert_external_source(
        &mut self,
        id: impl Into<String>,
        label: impl Into<String>,
        path: &Path,
        kind: DataSourceKind,
        fit: Option<FitIdentity>,
    ) -> Result<(), ProjectError> {
        let id = id.into();
        let path_text = path
            .to_str()
            .ok_or_else(|| ProjectError::NonUtf8Path(path.to_path_buf()))?;
        let record = DataSourceRecord {
            id: id.clone(),
            label: label.into(),
            kind,
            payload: DataSourcePayload::External {
                path: path_text.to_owned(),
                fingerprint: fingerprint(path)?,
            },
            fit,
        };
        let mut candidate = self.clone();
        if let Some(existing) = candidate.data_sources.iter_mut().find(|item| item.id == id) {
            *existing = record;
        } else {
            candidate.data_sources.push(record);
        }
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    pub fn upsert_embedded_source(
        &mut self,
        id: impl Into<String>,
        label: impl Into<String>,
        columns: Vec<EmbeddedColumn>,
        alive: Vec<bool>,
        kind: DataSourceKind,
        fit: Option<FitIdentity>,
    ) -> Result<(), ProjectError> {
        let id = id.into();
        let row_count = columns.first().map_or(0, |column| column.values.len());
        let sha256 = embedded_digest(&columns, row_count, &alive)?;
        let record = DataSourceRecord {
            id: id.clone(),
            label: label.into(),
            kind,
            payload: DataSourcePayload::Embedded {
                columns,
                row_count,
                alive,
                sha256,
            },
            fit,
        };
        let mut candidate = self.clone();
        if let Some(existing) = candidate.data_sources.iter_mut().find(|item| item.id == id) {
            *existing = record;
        } else {
            candidate.data_sources.push(record);
        }
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    pub fn source_states(&self) -> Vec<(&str, SourceState)> {
        self.data_sources
            .iter()
            .map(|source| (source.id.as_str(), source_state(source)))
            .collect()
    }
}

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

fn source_state(source: &DataSourceRecord) -> SourceState {
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
    let warnings = document
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
        .collect();
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

fn validate_axis(axis: &AxisRecord, labels: &BTreeSet<String>) -> Result<(), ProjectError> {
    if !axis.minimum.is_finite() || !axis.maximum.is_finite() || axis.minimum >= axis.maximum {
        return Err(ProjectError::Validation(format!(
            "axis {} requires finite minimum < maximum",
            axis.id
        )));
    }
    if !labels.contains(&axis.label_id) {
        return Err(ProjectError::Validation(format!(
            "axis {} references unknown label {}",
            axis.id, axis.label_id
        )));
    }
    if matches!(axis.scale, AxisScale::Log10) && axis.minimum <= 0.0 {
        return Err(ProjectError::Validation(format!(
            "log axis {} requires a positive minimum",
            axis.id
        )));
    }
    match &axis.locator {
        LocatorSpec::Auto { target_count } if !(2..=20).contains(target_count) => {
            return Err(ProjectError::Validation(format!(
                "axis {} auto locator target must be within 2..=20",
                axis.id
            )));
        }
        LocatorSpec::Fixed { values }
            if values.is_empty() || values.iter().any(|value| !value.is_finite()) =>
        {
            return Err(ProjectError::Validation(format!(
                "axis {} fixed locator requires finite values",
                axis.id
            )));
        }
        _ => {}
    }
    match axis.formatter {
        FormatterSpec::Decimal { precision } | FormatterSpec::Scientific { precision }
            if precision > 15 =>
        {
            return Err(ProjectError::Validation(format!(
                "axis {} formatter precision exceeds 15",
                axis.id
            )));
        }
        _ => {}
    }
    Ok(())
}

fn validate_artist(
    artist: &ArtistRecord,
    sources: &BTreeSet<String>,
    labels: &BTreeSet<String>,
    artists: &BTreeSet<String>,
    palette_colors: &BTreeSet<String>,
) -> Result<(), ProjectError> {
    let expected_kind = match &artist.properties {
        ArtistProperties::Line { binding, stroke } => {
            validate_binding(artist, binding, sources)?;
            validate_stroke(artist, stroke, palette_colors)?;
            ArtistKind::Line
        }
        ArtistProperties::Scatter { binding, marker } => {
            validate_binding(artist, binding, sources)?;
            if !marker.size_pt.is_finite()
                || marker.size_pt <= 0.0
                || !palette_colors.contains(&marker.color_id)
            {
                return Err(ProjectError::Validation(format!(
                    "artist {} has an invalid marker style",
                    artist.id
                )));
            }
            ArtistKind::Scatter
        }
        ArtistProperties::ErrorBar {
            binding,
            y_error_column,
            cap_width_pt,
            stroke,
        } => {
            validate_binding(artist, binding, sources)?;
            validate_stroke(artist, stroke, palette_colors)?;
            if y_error_column.trim().is_empty() || !cap_width_pt.is_finite() || *cap_width_pt <= 0.0
            {
                return Err(ProjectError::Validation(format!(
                    "artist {} has an invalid error-bar binding",
                    artist.id
                )));
            }
            ArtistKind::ErrorBar
        }
        ArtistProperties::ReferenceLine { value, stroke, .. } => {
            validate_stroke(artist, stroke, palette_colors)?;
            if !value.is_finite() {
                return Err(ProjectError::Validation(format!(
                    "artist {} has a non-finite reference value",
                    artist.id
                )));
            }
            ArtistKind::ReferenceLine
        }
        ArtistProperties::Annotation {
            label_id,
            x_pt,
            y_pt,
        } => {
            if !labels.contains(label_id) || !x_pt.is_finite() || !y_pt.is_finite() {
                return Err(ProjectError::Validation(format!(
                    "artist {} has an invalid annotation",
                    artist.id
                )));
            }
            ArtistKind::Annotation
        }
        ArtistProperties::Legend {
            entries,
            x_pt,
            y_pt,
        } => {
            if entries.is_empty()
                || !x_pt.is_finite()
                || !y_pt.is_finite()
                || entries.iter().any(|entry| {
                    !artists.contains(&entry.artist_id) || !labels.contains(&entry.label_id)
                })
            {
                return Err(ProjectError::Validation(format!(
                    "artist {} has invalid legend entries or placement",
                    artist.id
                )));
            }
            ArtistKind::Legend
        }
    };
    if artist.kind != expected_kind {
        return Err(ProjectError::Validation(format!(
            "artist {} kind does not match its properties",
            artist.id
        )));
    }
    Ok(())
}

fn validate_binding(
    artist: &ArtistRecord,
    binding: &DataBinding,
    sources: &BTreeSet<String>,
) -> Result<(), ProjectError> {
    if !sources.contains(&binding.data_source_id)
        || binding.x_column.trim().is_empty()
        || binding.y_column.trim().is_empty()
    {
        return Err(ProjectError::Validation(format!(
            "artist {} has an invalid data binding",
            artist.id
        )));
    }
    Ok(())
}

fn validate_stroke(
    artist: &ArtistRecord,
    stroke: &StrokeStyle,
    palette_colors: &BTreeSet<String>,
) -> Result<(), ProjectError> {
    if !palette_colors.contains(&stroke.color_id)
        || !stroke.width_pt.is_finite()
        || stroke.width_pt <= 0.0
        || stroke
            .dash_pt
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
    {
        return Err(ProjectError::Validation(format!(
            "artist {} has an invalid stroke style",
            artist.id
        )));
    }
    Ok(())
}

fn validate_payload(source: &DataSourceRecord) -> Result<(), ProjectError> {
    match &source.payload {
        DataSourcePayload::External { path, fingerprint } => {
            if path.is_empty() || fingerprint.sha256.len() != 64 {
                return Err(ProjectError::Validation(format!(
                    "external data source {} has invalid path or fingerprint",
                    source.id
                )));
            }
        }
        DataSourcePayload::Embedded {
            columns,
            row_count,
            alive,
            sha256,
        } => {
            if sha256.len() != 64
                || (!alive.is_empty() && alive.len() != *row_count)
                || columns
                    .iter()
                    .any(|column| column.values.len() != *row_count)
                || columns
                    .iter()
                    .flat_map(|column| &column.values)
                    .any(|value| !value.is_finite())
                || sha256 != &embedded_digest(columns, *row_count, alive)?
            {
                return Err(ProjectError::Validation(format!(
                    "embedded data source {} has inconsistent data",
                    source.id
                )));
            }
        }
    }
    Ok(())
}

fn embedded_digest(
    columns: &[EmbeddedColumn],
    row_count: usize,
    alive: &[bool],
) -> Result<String, ProjectError> {
    let bytes = if alive.is_empty() {
        serde_json::to_vec(&(columns, row_count))?
    } else {
        serde_json::to_vec(&(columns, row_count, alive))?
    };
    Ok(hex_digest(&bytes))
}

fn validate_finite_positive(name: &str, value: f64) -> Result<(), ProjectError> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(ProjectError::Validation(format!(
            "{name} must be finite and positive"
        )))
    }
}

fn validate_scale(name: &str, value: f64) -> Result<(), ProjectError> {
    if value.is_finite() && (0.1..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(ProjectError::Validation(format!(
            "{name} must be within 0.1..=1.0"
        )))
    }
}

fn collect_ids<'a>(
    values: impl Iterator<Item = &'a str>,
    kind: &str,
) -> Result<BTreeSet<String>, ProjectError> {
    let mut ids = BTreeSet::new();
    for id in values {
        if id.trim().is_empty() || !ids.insert(id.to_owned()) {
            return Err(ProjectError::Validation(format!(
                "{kind} ID is empty or duplicated: {id:?}"
            )));
        }
    }
    Ok(ids)
}

fn insert_id(ids: &mut BTreeSet<String>, id: &str) -> Result<(), ProjectError> {
    if id.trim().is_empty() || !ids.insert(id.to_owned()) {
        return Err(ProjectError::Validation(format!(
            "figure node ID is empty or duplicated: {id:?}"
        )));
    }
    Ok(())
}

fn hex_digest(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn default_typography() -> TypographyProfile {
    TypographyProfile {
        id: "instplot-publication-v1".to_owned(),
        family: "TeX Gyre Heros".to_owned(),
        font_version: "2.501".to_owned(),
        faces: vec![
            font_face(
                FontStyle::Regular,
                "TeXGyreHeros-Regular",
                "6ae1a09d5a940367b7aaaa91ee8bd8a2c333bfe193e7096e23f931357d62081f",
            ),
            font_face(
                FontStyle::Italic,
                "TeXGyreHeros-Italic",
                "6473df7fa107b3fb4be38973710afe22b0640c2ac076d5337cf126bed9aa108c",
            ),
            font_face(
                FontStyle::Bold,
                "TeXGyreHeros-Bold",
                "b170162835f4efc288886dd4231406dc47e19b614cf4416836635599d44a7d60",
            ),
            font_face(
                FontStyle::BoldItalic,
                "TeXGyreHeros-BoldItalic",
                "166fc6d068d9c9974281555cb3d730365537a9b676ab269bb5163f5a75496505",
            ),
        ],
        subscript_scale: 0.72,
        superscript_scale: 0.72,
    }
}

fn font_face(style: FontStyle, postscript_name: &str, sha256: &str) -> FontFaceRecord {
    FontFaceRecord {
        style,
        postscript_name: postscript_name.to_owned(),
        sha256: sha256.to_owned(),
    }
}

fn binding(data_source_id: &str, x_column: &str, y_column: &str) -> DataBinding {
    DataBinding {
        data_source_id: data_source_id.to_owned(),
        x_column: x_column.to_owned(),
        y_column: y_column.to_owned(),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyProjectV0 {
    schema_version: u32,
    producer_version: String,
    figure_id: String,
    x_min: f64,
    x_max: f64,
    y_min: f64,
    y_max: f64,
}

fn migrate_v0(value: Value) -> Result<ProjectDocument, ProjectError> {
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
        operation: "migrate_schema_0_to_1".to_owned(),
        input_ids: vec![legacy.producer_version],
        parameters: BTreeMap::new(),
    });
    Ok(document)
}

#[cfg(test)]
mod tests {
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
        let columns: Vec<EmbeddedColumn> =
            serde_json::from_value(payload["columns"].clone()).unwrap();
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
    fn legacy_schema_zero_migrates_with_ranges_and_audit_record() {
        let legacy = include_bytes!("../tests/fixtures/project-v0.instplot");
        let migrated = decode_project(legacy).unwrap();
        assert_eq!(migrated.schema_version, PROJECT_SCHEMA_VERSION);
        assert_eq!(migrated.figure.id, "legacy-figure");
        assert_eq!(migrated.figure.axes[0].x.minimum, -4.0);
        assert_eq!(
            migrated.provenance.last().unwrap().operation,
            "migrate_schema_0_to_1"
        );
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
            Err(ProjectError::UnsupportedSchema(2))
        ));
    }
}
