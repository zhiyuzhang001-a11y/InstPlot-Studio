use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use atomicwrites::{AllowOverwrite, AtomicFile};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const PROJECT_SCHEMA_VERSION: u32 = 6;

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minor_interval: Option<f64>,
    pub formatter: FormatterSpec,
    #[serde(default)]
    pub autoscale: bool,
    #[serde(default)]
    pub appearance: AxisAppearanceRecord,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AxisAppearanceRecord {
    pub near_spine: bool,
    pub far_spine: bool,
    pub near_ticks: bool,
    pub far_ticks: bool,
    pub near_tick_labels: bool,
    pub far_tick_labels: bool,
    pub major_ticks: bool,
    pub minor_ticks: bool,
    pub tick_direction: TickDirection,
    pub grid_major: bool,
    pub grid_minor: bool,
    pub tick_label_pad_pt: f64,
    pub label_edge_pad_pt: f64,
    pub label_tick_pad_pt: f64,
}

impl Default for AxisAppearanceRecord {
    fn default() -> Self {
        Self {
            near_spine: true,
            far_spine: true,
            near_ticks: true,
            far_ticks: true,
            near_tick_labels: true,
            far_tick_labels: false,
            major_ticks: true,
            minor_ticks: true,
            tick_direction: TickDirection::In,
            grid_major: false,
            grid_minor: false,
            tick_label_pad_pt: 4.0,
            label_edge_pad_pt: 6.0,
            label_tick_pad_pt: 4.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TickDirection {
    In,
    Out,
    InOut,
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
    Interval { step: f64 },
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
    #[serde(default = "default_true")]
    pub visible: bool,
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
#[serde(deny_unknown_fields)]
pub struct AnnotationConnectorRecord {
    pub target_x: f64,
    pub target_y: f64,
    pub stroke: StrokeStyle,
    #[serde(default)]
    pub start_arrow: bool,
    #[serde(default)]
    pub end_arrow: bool,
    pub arrow_size_pt: f64,
}

fn default_true() -> bool {
    true
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        x_error_column: Option<String>,
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
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        connectors: Vec<AnnotationConnectorRecord>,
    },
    Legend {
        entries: Vec<LegendEntry>,
        x_pt: f64,
        y_pt: f64,
        #[serde(default)]
        placement: LegendPlacement,
        #[serde(default)]
        grid: LegendGrid,
        #[serde(default)]
        position_custom: bool,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegendPlacement {
    Auto,
    #[default]
    Inside,
    Above,
    Right,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegendGrid {
    #[default]
    Auto,
    Rows(u8),
    Columns(u8),
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
    #[serde(default = "default_true")]
    pub filled: bool,
    #[serde(default = "default_marker_interval")]
    pub interval: usize,
}

fn default_marker_interval() -> usize {
    1
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkerShape {
    Circle,
    Square,
    Triangle,
    TriangleDown,
    Diamond,
    Pentagon,
    Star,
    Plus,
    Cross,
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
    #[serde(default = "default_true")]
    pub visible: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataSourceRecord {
    pub id: String,
    pub label: String,
    pub kind: DataSourceKind,
    pub payload: DataSourcePayload,
    pub fit: Option<FitIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_path: Option<String>,
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
    /// Per-cell validity for imports containing blank or non-finite values.
    /// Empty means every value is valid, preserving the compact legacy format.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub valid: Vec<bool>,
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
    #[serde(default = "default_selected_raster_dpi")]
    pub selected_raster_dpi: u32,
    pub transparent_background: bool,
}

fn default_selected_raster_dpi() -> u32 {
    300
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

fn showcase_curve_value(index: usize, x: f64) -> f64 {
    let baseline = -1.95 + index as f64 * 0.58;
    let center = -1.8 + index as f64 * 0.6;
    let radius = 1.15 + (index % 3) as f64 * 0.12;
    let distance = (x - center) / radius;
    let rounded_peak = (1.0 - distance * distance).max(0.0).powi(2);
    let y = baseline + 0.45 * rounded_peak + (index as f64 - 3.0) * 0.02 * x;
    (y * 1_000_000.0).round() / 1_000_000.0
}

mod fixture;

impl ProjectDocument {
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
            || !self
                .export_preferences
                .raster_dpi
                .contains(&self.export_preferences.selected_raster_dpi)
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
            origin_path: Some(path_text.to_owned()),
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
            origin_path: self
                .data_sources
                .iter()
                .find(|source| source.id == id)
                .and_then(|source| source.origin_path.clone()),
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

mod storage;

use storage::source_state;
pub use storage::{decode_project, fingerprint, open_project, save_project};

mod validation;

use validation::*;

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

mod migration;

use migration::{migrate_v0, migrate_v1, migrate_v2, migrate_v3, migrate_v4, migrate_v5};

#[cfg(test)]
mod project_tests;
