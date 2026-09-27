use core::fmt;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use instplot_core::{DataSet, DataSetKind, NumericColumn};
use instplot_io::save_retained_rows_selected_with_fits;
use instplot_layout::{
    Annotation, AnnotationConnector, AnnotationPosition, AxisAppearance as LayoutAxisAppearance,
    AxisPair as LayoutAxisPair, AxisSpec, Bounds, Chart, DashStyle, DataPoint, ErrorBar,
    ErrorStyle, Formatter, GridSpec, LayoutError, LayoutResult, LegendErrorStyle,
    LegendGrid as LayoutLegendGrid, LegendPosition, LegendSpec, LineStyle, Locator,
    MarkerShape as LayoutMarkerShape, MarkerStyle as LayoutMarkerStyle, Scale, Series,
    TickDirection as LayoutTickDirection, layout,
};
use instplot_render::{Color, CompileError, DisplayList, NodeId, compile, fixed_figure};
use instplot_text::Label;

use crate::project::fingerprint;
use crate::{
    ArtistKind, ArtistProperties, ArtistRecord, ArtistRole, AxisBinding, AxisIdentity, AxisMode,
    AxisRecord, AxisScale, DEFAULT_CURVE_WIDTH_PT, DEFAULT_ERROR_BAR_WIDTH_PT, DataBinding,
    DataSourceKind, DataSourceOrigin, DataSourcePayload, DataSourceRecord, EmbeddedColumn,
    FitIdentity, FormatterSpec, LabelNode, LegendEntry, LegendGrid, LegendPlacement, LocatorSpec,
    ManagedDataFile, ManagedDataFormat, ManualDataRecipe, MarkerShape, MarkerStyle,
    OpenProjectReport, PaletteColor, PaletteRegistry, ProjectDocument, ProjectError,
    ProvenanceRecord, ReferenceOrientation, SemanticLabel, SeriesGroupRecord, StrokeStyle,
    XAxisSlot, YAxisSlot, builtin_palette_registry, open_project, palette_series_color_ids,
    save_project,
};

/// The editable runtime view of the formal, versioned B2 Figure Document.
#[derive(Clone, Debug, PartialEq)]
pub struct FigureDocument {
    project: ProjectDocument,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AxisRanges {
    pub x_min: f64,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,
}

/// The deterministic B3.2 axes result and the project identities used to produce it.
#[derive(Clone, Debug)]
pub struct DocumentLayout {
    pub result: LayoutResult,
    pub data_clip: Bounds,
    pub project_ids: BTreeMap<NodeId, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocumentLayoutError {
    MissingAxes,
    MissingLabel(String),
    MissingArtist(String),
    MissingDataSource(String),
    ExternalDataUnavailable(String),
    MissingColumn {
        source: String,
        column: String,
    },
    ColumnLengthMismatch {
        source: String,
        x: usize,
        y: usize,
    },
    MissingColor(String),
    DuplicateNodeId {
        numeric: u64,
        first: String,
        second: String,
    },
    Layout(LayoutError),
}

impl fmt::Display for DocumentLayoutError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingAxes => formatter.write_str("figure has no axes to lay out"),
            Self::MissingLabel(id) => write!(formatter, "semantic label {id} is missing"),
            Self::MissingArtist(id) => write!(formatter, "axes references missing artist {id}"),
            Self::MissingDataSource(id) => {
                write!(formatter, "artist references missing data source {id}")
            }
            Self::ExternalDataUnavailable(id) => write!(
                formatter,
                "data source {id} is external and has not been loaded into the layout"
            ),
            Self::MissingColumn { source, column } => {
                write!(formatter, "data source {source} is missing column {column}")
            }
            Self::ColumnLengthMismatch { source, x, y } => write!(
                formatter,
                "data source {source} has mismatched X/Y lengths ({x} and {y})"
            ),
            Self::MissingColor(id) => write!(formatter, "palette color {id} is missing"),
            Self::DuplicateNodeId {
                numeric,
                first,
                second,
            } => write!(
                formatter,
                "project IDs {first} and {second} both resolve to node {numeric}"
            ),
            Self::Layout(error) => write!(formatter, "axes layout: {error}"),
        }
    }
}

impl std::error::Error for DocumentLayoutError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeriesKind {
    Line,
    Scatter,
    ErrorBar,
    ReferenceLine,
    Annotation,
    Legend,
}

impl fmt::Display for SeriesKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Line => "Line",
            Self::Scatter => "Scatter",
            Self::ErrorBar => "Error bar",
            Self::ReferenceLine => "Reference line",
            Self::Annotation => "Annotation",
            Self::Legend => "Legend",
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeriesDescriptor {
    pub id: String,
    pub kind: SeriesKind,
    pub role: ArtistRole,
    pub visible: bool,
    pub effective_visible: bool,
    pub label: String,
    pub binding: Option<DataBinding>,
    pub axes: Option<AxisBinding>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeriesCreationStyle {
    Line,
    Scatter,
    LineAndMarker,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveDirection {
    Earlier,
    Later,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AxisDimension {
    X,
    Y,
}

impl FigureDocument {
    pub fn fixed() -> Self {
        Self {
            project: ProjectDocument::fixed_fixture(),
        }
    }

    pub fn showcase() -> Self {
        Self {
            project: ProjectDocument::showcase_fixture(),
        }
    }

    pub fn from_datasets(datasets: &[DataSet]) -> Result<Self, ProjectError> {
        if datasets.is_empty() {
            return Err(ProjectError::Validation(
                "Lite handoff contains no datasets".to_owned(),
            ));
        }
        validate_handoff_datasets(datasets)?;

        let mut project = ProjectDocument::fixed_fixture();
        project.data_sources.clear();
        project.figure.artists.clear();
        project.figure.axes[0].artist_ids.clear();
        project.figure.axes[0].series_groups.clear();
        project.overrides.clear();
        project.provenance.clear();
        add_handoff_palette_colors(&mut project);

        for dataset in datasets
            .iter()
            .filter(|dataset| dataset.kind == DataSetKind::Source)
        {
            project.upsert_embedded_source(
                dataset.plot_id.clone(),
                dataset.display_name(),
                embedded_columns(dataset),
                dataset.alive.clone(),
                DataSourceKind::Source,
                None,
            )?;
            set_dataset_origin(&mut project, dataset);
        }
        for dataset in datasets
            .iter()
            .filter(|dataset| dataset.kind == DataSetKind::Fit)
        {
            let link = dataset.fit_link.as_ref().expect("validated fit link");
            project.upsert_embedded_source(
                dataset.plot_id.clone(),
                dataset.display_name(),
                embedded_columns(dataset),
                dataset.alive.clone(),
                DataSourceKind::Fit,
                Some(FitIdentity {
                    parent_data_source_id: link
                        .parent_dataset_id
                        .clone()
                        .expect("validated parent identity"),
                    source_x_column: link.source_x_column.clone(),
                    source_y_column: link.source_y_column.clone(),
                    equation: link.equation.clone(),
                    display_equation: link.display_equation.clone(),
                }),
            )?;
            set_dataset_origin(&mut project, dataset);
        }

        let source_colors = datasets
            .iter()
            .filter(|dataset| dataset.kind == DataSetKind::Source)
            .enumerate()
            .map(|(index, dataset)| {
                (
                    dataset.plot_id.clone(),
                    HANDOFF_COLORS[index % HANDOFF_COLORS.len()].0.to_owned(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut artists = Vec::new();
        let mut legend_entries = Vec::new();
        let mut plotted_values = Vec::new();
        for (index, dataset) in datasets.iter().enumerate() {
            let (x_column, y_column) = plotted_columns(dataset, datasets)?;
            let color_id = match dataset.kind {
                DataSetKind::Source => source_colors
                    .get(&dataset.plot_id)
                    .expect("every source has a color"),
                DataSetKind::Fit => source_colors
                    .get(
                        dataset
                            .fit_link
                            .as_ref()
                            .and_then(|link| link.parent_dataset_id.as_ref())
                            .expect("validated fit parent"),
                    )
                    .expect("validated fit parent has a color"),
            };
            let artist_id = format!("handoff-artist-{}", dataset.plot_id);
            let label_id = format!("handoff-label-{}", dataset.plot_id);
            project.semantic_registry.push(SemanticLabel {
                id: label_id.clone(),
                nodes: vec![LabelNode::Text(dataset.display_name())],
            });
            let binding = DataBinding {
                data_source_id: dataset.plot_id.clone(),
                x_column: x_column.clone(),
                y_column: y_column.clone(),
            };
            let (kind, role, properties) = match dataset.kind {
                DataSetKind::Source => (
                    ArtistKind::Scatter,
                    ArtistRole::Data,
                    ArtistProperties::Scatter {
                        binding,
                        marker: MarkerStyle {
                            color_id: color_id.clone(),
                            shape: HANDOFF_MARKERS[index % HANDOFF_MARKERS.len()],
                            size_pt: 4.0,
                            filled: true,
                            interval: 1,
                        },
                    },
                ),
                DataSetKind::Fit => (
                    ArtistKind::Line,
                    ArtistRole::Fit,
                    ArtistProperties::Line {
                        binding,
                        stroke: StrokeStyle {
                            color_id: color_id.clone(),
                            width_pt: DEFAULT_CURVE_WIDTH_PT,
                            dash_pt: Vec::new(),
                        },
                    },
                ),
            };
            artists.push(ArtistRecord {
                id: artist_id.clone(),
                kind,
                role,
                visible: true,
                properties,
            });
            legend_entries.push(LegendEntry {
                artist_id: artist_id.clone(),
                label_id,
                visible: true,
            });
            project.figure.axes[0].artist_ids.push(artist_id);
            project.figure.axes[0]
                .series_groups
                .push(SeriesGroupRecord {
                    id: format!("series-group-handoff-{}", dataset.plot_id),
                    artist_ids: vec![format!("handoff-artist-{}", dataset.plot_id)],
                    axes: AxisBinding::PRIMARY,
                });
            collect_plotted_values(dataset, &x_column, &y_column, &mut plotted_values);
        }
        let legend_id = "handoff-legend".to_owned();
        artists.push(ArtistRecord {
            id: legend_id.clone(),
            kind: ArtistKind::Legend,
            role: ArtistRole::Legend,
            visible: true,
            properties: ArtistProperties::Legend {
                entries: legend_entries,
                x_pt: 110.0,
                y_pt: 12.0,
                placement: LegendPlacement::Auto,
                position_custom: false,
                grid: LegendGrid::Auto,
            },
        });
        project.figure.axes[0].artist_ids.push(legend_id);
        project.figure.artists = artists;
        set_handoff_axes(&mut project, datasets, &plotted_values)?;
        project.provenance.push(ProvenanceRecord {
            id: "provenance-lite-handoff".to_owned(),
            operation: "import_lite_datasets_embedded".to_owned(),
            input_ids: datasets
                .iter()
                .map(|dataset| dataset.plot_id.clone())
                .collect(),
            parameters: BTreeMap::from([(
                "dataset_count".to_owned(),
                serde_json::Value::from(datasets.len()),
            )]),
        });
        project.validate()?;
        Ok(Self { project })
    }

    pub fn compile(&self) -> Result<DisplayList, CompileError> {
        let mut figure = fixed_figure();
        let stored = &self.project.figure.axes[0];
        let axes = &mut figure.axes[0];
        axes.x.minimum = stored.x.minimum;
        axes.x.maximum = stored.x.maximum;
        axes.y.minimum = stored.y.minimum;
        axes.y.maximum = stored.y.maximum;
        compile(&figure)
    }

    /// Resolve the first project axes through the formal B3 layout engine.
    ///
    /// This axes-only entry point preserves the focused B3.2 contract.
    pub fn layout_axes(&self) -> Result<DocumentLayout, DocumentLayoutError> {
        self.layout_document(false)
    }

    /// Resolve the formal axes plus the B3.4 linear line/scatter artist slice.
    pub fn layout_figure(&self) -> Result<DocumentLayout, DocumentLayoutError> {
        self.layout_document(true)
    }

    fn layout_document(
        &self,
        include_artists: bool,
    ) -> Result<DocumentLayout, DocumentLayoutError> {
        let stored = self
            .project
            .figure
            .axes
            .first()
            .ok_or(DocumentLayoutError::MissingAxes)?;
        let width_pt = self.project.figure.width_mm * 72.0 / 25.4;
        let height_pt = self.project.figure.height_mm * 72.0 / 25.4;
        let mut project_ids = BTreeMap::new();
        let axes_id = register_node_id(&stored.id, &mut project_ids)?;
        let x = axis_spec(
            &stored.x,
            width_pt,
            &self.project.semantic_registry,
            &mut project_ids,
            true,
        )?;
        let y = axis_spec(
            &stored.y,
            height_pt,
            &self.project.semantic_registry,
            &mut project_ids,
            true,
        )?;
        let x2 = if stored.mode == AxisMode::DualX {
            stored
                .x2
                .as_ref()
                .map(|axis| {
                    axis_spec(
                        axis,
                        width_pt,
                        &self.project.semantic_registry,
                        &mut project_ids,
                        compute_axis_data_bounds(
                            &self.project,
                            AxisIdentity::X2,
                            AutoscalePolicy::default(),
                        )
                        .is_ok(),
                    )
                })
                .transpose()?
        } else {
            None
        };
        let y2 = if stored.mode == AxisMode::DualY {
            stored
                .y2
                .as_ref()
                .map(|axis| {
                    axis_spec(
                        axis,
                        height_pt,
                        &self.project.semantic_registry,
                        &mut project_ids,
                        compute_axis_data_bounds(
                            &self.project,
                            AxisIdentity::Y2,
                            AutoscalePolicy::default(),
                        )
                        .is_ok(),
                    )
                })
                .transpose()?
        } else {
            None
        };
        let (legend, legend_labels) = if include_artists {
            legend_spec(stored, &self.project, &mut project_ids)?
        } else {
            (None, BTreeMap::new())
        };
        let (series, series_axes) = if include_artists {
            formal_series(stored, &self.project, &legend_labels, &mut project_ids)?
        } else {
            (Vec::new(), BTreeMap::new())
        };
        let annotations = if include_artists {
            formal_annotations(stored, &self.project, &mut project_ids)?
        } else {
            Vec::new()
        };
        let result = layout(&Chart {
            id: axes_id,
            width_pt,
            height_pt,
            x,
            y,
            x2,
            y2,
            series_axes,
            series,
            annotations,
            legend,
        })
        .map_err(DocumentLayoutError::Layout)?;
        Ok(DocumentLayout {
            data_clip: result.axes,
            result,
            project_ids,
        })
    }
}

mod autoscale;
mod autoscale_api;
mod axes;
mod datasets;
mod objects;
mod series;

pub use autoscale::{
    AutoscalePolicy, DataBounds, VisualBounds, apply_visual_padding, compute_axis_data_bounds,
    compute_data_bounds,
};
use autoscale::{apply_autoscale, refresh_active_autoscales};

fn reset_empty_axes(project: &mut ProjectDocument) {
    let axes = &mut project.figure.axes[0];
    let mut label_ids = vec![axes.x.label_id.clone(), axes.y.label_id.clone()];
    if let Some(axis) = &axes.x2 {
        label_ids.push(axis.label_id.clone());
    }
    if let Some(axis) = &axes.y2 {
        label_ids.push(axis.label_id.clone());
    }
    for axis in std::iter::once(&mut axes.x)
        .chain(std::iter::once(&mut axes.y))
        .chain(axes.x2.iter_mut())
        .chain(axes.y2.iter_mut())
    {
        axis.minimum = 0.0;
        axis.maximum = 1.0;
        axis.scale = AxisScale::Linear;
        axis.locator = LocatorSpec::Auto { target_count: 6 };
        axis.minor_interval = None;
        axis.formatter = FormatterSpec::Auto;
        axis.autoscale = false;
    }
    for label in &mut project.semantic_registry {
        if label_ids.contains(&label.id) {
            label.nodes = vec![LabelNode::Text(String::new())];
        }
    }
}

fn artist_binding(artist: &ArtistRecord) -> Option<&DataBinding> {
    match &artist.properties {
        ArtistProperties::Line { binding, .. }
        | ArtistProperties::Scatter { binding, .. }
        | ArtistProperties::ErrorBar { binding, .. } => Some(binding),
        _ => None,
    }
}

fn artist_uses_source(artist: &ArtistRecord, data_source_id: &str) -> bool {
    artist_binding(artist).is_some_and(|binding| binding.data_source_id == data_source_id)
}

fn validate_column(
    project: &ProjectDocument,
    data_source_id: &str,
    column_name: &str,
) -> Result<(), String> {
    let source = project
        .data_sources
        .iter()
        .find(|source| source.id == data_source_id)
        .ok_or_else(|| format!("data source {data_source_id} is missing"))?;
    let DataSourcePayload::Embedded { columns, .. } = &source.payload else {
        return Err(format!(
            "data source {data_source_id} is external and unavailable for portable plotting"
        ));
    };
    if columns.iter().any(|column| column.name == column_name) {
        Ok(())
    } else {
        Err(format!(
            "data source {data_source_id} has no column named {column_name}"
        ))
    }
}

fn validate_error_column(
    project: &ProjectDocument,
    data_source_id: &str,
    column_name: &str,
) -> Result<(), String> {
    validate_column(project, data_source_id, column_name)?;
    let source = project
        .data_sources
        .iter()
        .find(|source| source.id == data_source_id)
        .expect("validate_column confirmed the data source");
    let DataSourcePayload::Embedded { columns, .. } = &source.payload else {
        return Err(format!(
            "data source {data_source_id} is external and unavailable for error validation"
        ));
    };
    let column = columns
        .iter()
        .find(|column| column.name == column_name)
        .expect("validate_column confirmed the column");
    for (index, value) in column.values.iter().enumerate() {
        if embedded_value_is_valid(column, index) && (!value.is_finite() || *value < 0.0) {
            return Err(format!(
                "error column {column_name} contains an invalid value at row {}",
                index + 1
            ));
        }
    }
    Ok(())
}

fn validate_binding_columns(
    project: &ProjectDocument,
    data_source_id: &str,
    x_column: &str,
    y_column: &str,
) -> Result<(), String> {
    validate_column(project, data_source_id, x_column)?;
    validate_column(project, data_source_id, y_column)
}

fn next_stable_id(project: &ProjectDocument, prefix: &str) -> String {
    let used = project
        .data_sources
        .iter()
        .map(|item| item.id.as_str())
        .chain(project.figure.artists.iter().map(|item| item.id.as_str()))
        .chain(
            project
                .figure
                .axes
                .iter()
                .flat_map(|axes| axes.series_groups.iter().map(|group| group.id.as_str())),
        )
        .chain(project.figure.axes.iter().flat_map(|axes| {
            std::iter::once(axes.id.as_str())
                .chain(std::iter::once(axes.x.id.as_str()))
                .chain(std::iter::once(axes.y.id.as_str()))
                .chain(axes.x2.iter().map(|axis| axis.id.as_str()))
                .chain(axes.y2.iter().map(|axis| axis.id.as_str()))
        }))
        .chain(
            project
                .semantic_registry
                .iter()
                .map(|item| item.id.as_str()),
        )
        .collect::<BTreeSet<_>>();
    (1_u64..)
        .map(|number| format!("{prefix}-{number}"))
        .find(|candidate| !used.contains(candidate.as_str()))
        .expect("the stable ID sequence is practically unbounded")
}

fn series_color_key(project: &ProjectDocument, binding: &DataBinding) -> String {
    if let Some(fit) = project
        .data_sources
        .iter()
        .find(|source| source.id == binding.data_source_id)
        .and_then(|source| source.fit.as_ref())
    {
        return format!(
            "{}\u{0}{}\u{0}{}",
            fit.parent_data_source_id, fit.source_x_column, fit.source_y_column
        );
    }
    format!(
        "{}\u{0}{}\u{0}{}",
        binding.data_source_id, binding.x_column, binding.y_column
    )
}

fn default_series_color(
    project: &ProjectDocument,
    data_source_id: &str,
    x_column: &str,
    y_column: &str,
) -> String {
    let requested = DataBinding {
        data_source_id: data_source_id.to_owned(),
        x_column: x_column.to_owned(),
        y_column: y_column.to_owned(),
    };
    let requested_key = series_color_key(project, &requested);
    for artist in &project.figure.artists {
        let Some(binding) = artist_binding(artist) else {
            continue;
        };
        if series_color_key(project, binding) != requested_key {
            continue;
        }
        return match &artist.properties {
            ArtistProperties::Line { stroke, .. } | ArtistProperties::ErrorBar { stroke, .. } => {
                stroke.color_id.clone()
            }
            ArtistProperties::Scatter { marker, .. } => marker.color_id.clone(),
            _ => continue,
        };
    }
    let available = palette_series_color_ids(&project.palette.id)
        .iter()
        .copied()
        .filter(|id| project.palette.colors.iter().any(|color| color.id == *id))
        .collect::<Vec<_>>();
    let used = project
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
    available
        .iter()
        .copied()
        .find(|id| !used.contains(id))
        .or_else(|| {
            (!available.is_empty()).then(|| {
                let existing_series = project
                    .figure
                    .artists
                    .iter()
                    .filter_map(artist_binding)
                    .map(|binding| series_color_key(project, binding))
                    .collect::<BTreeSet<_>>()
                    .len();
                available[existing_series % available.len()]
            })
        })
        .map(str::to_owned)
        .or_else(|| project.palette.colors.first().map(|color| color.id.clone()))
        .unwrap_or_else(|| "blue".to_owned())
}

fn default_series_marker(project: &ProjectDocument, data_source_id: &str) -> MarkerShape {
    let family_id = project
        .data_sources
        .iter()
        .find(|source| source.id == data_source_id)
        .and_then(|source| source.fit.as_ref())
        .map_or(data_source_id, |fit| fit.parent_data_source_id.as_str());
    for artist in &project.figure.artists {
        let Some(binding) = artist_binding(artist) else {
            continue;
        };
        let binding_family = project
            .data_sources
            .iter()
            .find(|source| source.id == binding.data_source_id)
            .and_then(|source| source.fit.as_ref())
            .map_or(binding.data_source_id.as_str(), |fit| {
                fit.parent_data_source_id.as_str()
            });
        if binding_family == family_id
            && let ArtistProperties::Scatter { marker, .. } = &artist.properties
        {
            return marker.shape;
        }
    }
    let family_index = project
        .data_sources
        .iter()
        .filter(|source| source.fit.is_none())
        .position(|source| source.id == family_id)
        .unwrap_or(0);
    HANDOFF_MARKERS[family_index % HANDOFF_MARKERS.len()]
}

fn dependent_source_ids(project: &ProjectDocument, data_source_id: &str) -> Vec<String> {
    let mut ids = BTreeSet::from([data_source_id.to_owned()]);
    loop {
        let before = ids.len();
        for source in &project.data_sources {
            if source
                .fit
                .as_ref()
                .is_some_and(|fit| ids.contains(&fit.parent_data_source_id))
            {
                ids.insert(source.id.clone());
            }
        }
        if ids.len() == before {
            break;
        }
    }
    ids.remove(data_source_id);
    ids.into_iter().collect()
}

fn prune_legend_entries(project: &mut ProjectDocument, removed_artist_ids: &[String]) {
    let mut empty_legends = Vec::new();
    let mut removed_label_ids = Vec::new();
    for artist in &mut project.figure.artists {
        if let ArtistProperties::Legend { entries, .. } = &mut artist.properties {
            entries.retain(|entry| {
                let keep = !removed_artist_ids.contains(&entry.artist_id);
                if !keep {
                    removed_label_ids.push(entry.label_id.clone());
                }
                keep
            });
            if entries.is_empty() {
                empty_legends.push(artist.id.clone());
            }
        }
    }
    project
        .figure
        .artists
        .retain(|artist| !empty_legends.contains(&artist.id));
    project.figure.axes[0]
        .artist_ids
        .retain(|id| !empty_legends.contains(id));
    let mut removed_targets = removed_artist_ids.to_vec();
    removed_targets.extend(empty_legends);
    project
        .overrides
        .retain(|record| !removed_targets.contains(&record.target_id));
    let axes = &project.figure.axes[0];
    let mut retained_label_ids = BTreeSet::from([axes.x.label_id.clone(), axes.y.label_id.clone()]);
    for artist in &project.figure.artists {
        match &artist.properties {
            ArtistProperties::Annotation { label_id, .. } => {
                retained_label_ids.insert(label_id.clone());
            }
            ArtistProperties::Legend { entries, .. } => {
                retained_label_ids.extend(entries.iter().map(|entry| entry.label_id.clone()));
            }
            _ => {}
        }
    }
    project.semantic_registry.retain(|label| {
        !removed_label_ids.contains(&label.id) || retained_label_ids.contains(&label.id)
    });
}

const HANDOFF_COLORS: [(&str, [u8; 4]); 7] = [
    ("blue", [68, 119, 170, 255]),
    ("object-red", [238, 102, 119, 255]),
    ("object-green", [34, 136, 51, 255]),
    ("object-yellow", [204, 187, 68, 255]),
    ("object-cyan", [102, 204, 238, 255]),
    ("object-purple", [170, 51, 119, 255]),
    ("object-light-gray", [187, 187, 187, 255]),
];

const HANDOFF_MARKERS: [MarkerShape; 7] = [
    MarkerShape::Circle,
    MarkerShape::Square,
    MarkerShape::Triangle,
    MarkerShape::TriangleDown,
    MarkerShape::Diamond,
    MarkerShape::Pentagon,
    MarkerShape::Star,
];

fn validate_handoff_datasets(datasets: &[DataSet]) -> Result<(), ProjectError> {
    let ids = datasets
        .iter()
        .map(|dataset| dataset.plot_id.as_str())
        .collect::<BTreeSet<_>>();
    if ids.len() != datasets.len() || ids.contains("") {
        return Err(ProjectError::Validation(
            "Lite handoff dataset IDs must be non-empty and unique".to_owned(),
        ));
    }
    let source_ids = datasets
        .iter()
        .filter(|dataset| dataset.kind == DataSetKind::Source)
        .map(|dataset| dataset.plot_id.as_str())
        .collect::<BTreeSet<_>>();
    if source_ids.is_empty() {
        return Err(ProjectError::Validation(
            "Lite handoff must contain at least one source dataset".to_owned(),
        ));
    }
    for dataset in datasets {
        if dataset.columns.len() < 2
            || dataset
                .columns
                .iter()
                .any(|column| column.values.len() != dataset.row_count)
            || dataset.alive.len() != dataset.row_count
        {
            return Err(ProjectError::Validation(format!(
                "Lite handoff dataset {} has inconsistent columns or alive state",
                dataset.plot_id
            )));
        }
        match (dataset.kind, &dataset.fit_link) {
            (DataSetKind::Source, None) => {}
            (DataSetKind::Source, Some(_)) => {
                return Err(ProjectError::Validation(format!(
                    "source dataset {} unexpectedly contains a fit link",
                    dataset.plot_id
                )));
            }
            (DataSetKind::Fit, Some(link)) => {
                let parent_id = link.parent_dataset_id.as_deref().ok_or_else(|| {
                    ProjectError::Validation(format!(
                        "fit dataset {} is missing Parent-ID",
                        dataset.plot_id
                    ))
                })?;
                let parent = datasets
                    .iter()
                    .find(|candidate| candidate.plot_id == parent_id)
                    .filter(|candidate| candidate.kind == DataSetKind::Source)
                    .ok_or_else(|| {
                        ProjectError::Validation(format!(
                            "fit dataset {} references unknown source {}",
                            dataset.plot_id, parent_id
                        ))
                    })?;
                for name in [&link.source_x_column, &link.source_y_column] {
                    if !parent.columns.iter().any(|column| column.name == *name) {
                        return Err(ProjectError::Validation(format!(
                            "fit dataset {} references missing parent column {}",
                            dataset.plot_id, name
                        )));
                    }
                }
            }
            (DataSetKind::Fit, None) => {
                return Err(ProjectError::Validation(format!(
                    "fit dataset {} is missing its explicit fit link",
                    dataset.plot_id
                )));
            }
        }
    }
    Ok(())
}

fn set_dataset_origin(project: &mut ProjectDocument, dataset: &DataSet) {
    if dataset.source.to_string_lossy().starts_with("embedded://") {
        return;
    }
    if let Some(source) = project
        .data_sources
        .iter_mut()
        .find(|source| source.id == dataset.plot_id)
    {
        source.origin_path = dataset.source.to_str().map(ToOwned::to_owned);
    }
}

fn embedded_columns(dataset: &DataSet) -> Vec<EmbeddedColumn> {
    dataset
        .columns
        .iter()
        .map(|column| {
            let valid = column
                .values
                .iter()
                .map(|value| value.is_finite())
                .collect::<Vec<_>>();
            let has_missing = valid.iter().any(|valid| !valid);
            EmbeddedColumn {
                name: column.name.clone(),
                values: column
                    .values
                    .iter()
                    .map(|value| if value.is_finite() { *value } else { 0.0 })
                    .collect(),
                valid: if has_missing { valid } else { Vec::new() },
            }
        })
        .collect()
}

fn add_handoff_palette_colors(project: &mut ProjectDocument) {
    for (id, rgba) in HANDOFF_COLORS {
        if !project.palette.colors.iter().any(|color| color.id == id) {
            project.palette.colors.push(PaletteColor {
                id: id.to_owned(),
                rgba,
            });
        }
    }
}

fn plotted_columns(
    dataset: &DataSet,
    datasets: &[DataSet],
) -> Result<(String, String), ProjectError> {
    if dataset.kind == DataSetKind::Source
        && let Some(link) = datasets.iter().find_map(|candidate| {
            candidate
                .fit_link
                .as_ref()
                .filter(|link| link.parent_dataset_id.as_deref() == Some(dataset.plot_id.as_str()))
        })
    {
        return Ok((link.source_x_column.clone(), link.source_y_column.clone()));
    }
    let mut columns = dataset.columns.iter();
    let x = columns
        .next()
        .ok_or_else(|| ProjectError::Validation("dataset is missing an X column".to_owned()))?;
    let y = columns
        .next()
        .ok_or_else(|| ProjectError::Validation("dataset is missing a Y column".to_owned()))?;
    Ok((x.name.clone(), y.name.clone()))
}

fn collect_plotted_values(
    dataset: &DataSet,
    x_column: &str,
    y_column: &str,
    output: &mut Vec<(f64, f64)>,
) {
    let x = dataset
        .columns
        .iter()
        .find(|column| column.name == x_column)
        .expect("validated X column");
    let y = dataset
        .columns
        .iter()
        .find(|column| column.name == y_column)
        .expect("validated Y column");
    output.extend(
        x.values
            .iter()
            .zip(&y.values)
            .enumerate()
            .filter(|(index, (x, y))| dataset.alive[*index] && x.is_finite() && y.is_finite())
            .map(|(_, (x, y))| (*x, *y)),
    );
}

fn set_handoff_axes(
    project: &mut ProjectDocument,
    datasets: &[DataSet],
    values: &[(f64, f64)],
) -> Result<(), ProjectError> {
    let source = datasets
        .iter()
        .find(|dataset| dataset.kind == DataSetKind::Source)
        .expect("validated source dataset");
    let (x_column, y_column) = plotted_columns(source, datasets)?;
    let Some((x_min, x_max, y_min, y_max)) =
        values
            .iter()
            .copied()
            .fold(None, |bounds, (x, y)| match bounds {
                None => Some((x, x, y, y)),
                Some((x_min, x_max, y_min, y_max)) => {
                    Some((x_min.min(x), x_max.max(x), y_min.min(y), y_max.max(y)))
                }
            })
    else {
        return Err(ProjectError::Validation(
            "Lite handoff contains no alive plotted rows".to_owned(),
        ));
    };
    let padded = |minimum: f64, maximum: f64| {
        let span = maximum - minimum;
        let padding = if span > 0.0 { span * 0.05 } else { 1.0 };
        (minimum - padding, maximum + padding)
    };
    let (x_min, x_max) = padded(x_min, x_max);
    let (y_min, y_max) = padded(y_min, y_max);
    let axes = &mut project.figure.axes[0];
    axes.x.minimum = x_min;
    axes.x.maximum = x_max;
    axes.y.minimum = y_min;
    axes.y.maximum = y_max;
    // These bounds were derived from the visible data. Preserve that semantic state so
    // adding error bars or rebinding columns can extend the range automatically later.
    axes.x.autoscale = true;
    axes.y.autoscale = true;
    let x_label = project
        .semantic_registry
        .iter_mut()
        .find(|label| label.id == axes.x.label_id)
        .expect("fixed document has an X label");
    x_label.nodes = vec![LabelNode::Text(x_column)];
    let y_label = project
        .semantic_registry
        .iter_mut()
        .find(|label| label.id == axes.y.label_id)
        .expect("fixed document has a Y label");
    y_label.nodes = vec![LabelNode::Text(y_column)];
    Ok(())
}

fn axis_label_is_column(nodes: &[LabelNode], column: &str) -> bool {
    matches!(
        nodes,
        [LabelNode::Text(value) | LabelNode::Variable(value) | LabelNode::Upright(value)]
            if value == column
    )
}

fn formal_series(
    axes: &crate::AxesRecord,
    project: &ProjectDocument,
    legend_labels: &BTreeMap<String, instplot_text::Label>,
    project_ids: &mut BTreeMap<NodeId, String>,
) -> Result<(Vec<Series>, BTreeMap<NodeId, LayoutAxisPair>), DocumentLayoutError> {
    let mut output = Vec::new();
    let mut series_axes = BTreeMap::new();
    for artist_id in &axes.artist_ids {
        let artist = project
            .figure
            .artists
            .iter()
            .find(|artist| artist.id == *artist_id)
            .ok_or_else(|| DocumentLayoutError::MissingArtist(artist_id.clone()))?;
        if !project.artist_effectively_visible(artist) {
            continue;
        }
        let (points, line, marker, errors, error_style, color_id) = match &artist.properties {
            ArtistProperties::Line { binding, stroke } => (
                bound_points(binding, project)?,
                Some(LineStyle {
                    width: stroke.width_pt,
                    dash: dash_style(stroke),
                }),
                None,
                Vec::new(),
                None,
                stroke.color_id.as_str(),
            ),
            ArtistProperties::Scatter { binding, marker } => (
                bound_points(binding, project)?,
                None,
                Some(LayoutMarkerStyle {
                    shape: marker_shape(marker.shape),
                    size: marker.size_pt,
                    filled: marker.filled,
                    interval: marker.interval.max(1),
                }),
                Vec::new(),
                None,
                marker.color_id.as_str(),
            ),
            ArtistProperties::ErrorBar {
                binding,
                x_error_column,
                y_error_column,
                cap_width_pt,
                stroke,
            } => {
                let (points, values) =
                    bound_error_data(binding, x_error_column.as_deref(), y_error_column, project)?;
                (
                    points,
                    None,
                    None,
                    values,
                    Some(ErrorStyle {
                        width: stroke.width_pt,
                        cap_width: *cap_width_pt,
                        dash: dash_style(stroke),
                    }),
                    stroke.color_id.as_str(),
                )
            }
            ArtistProperties::ReferenceLine {
                orientation,
                value,
                axes: binding,
                stroke,
            } => {
                let x_axis = match binding.x {
                    XAxisSlot::X1 => Some(&axes.x),
                    XAxisSlot::X2 => axes.x2.as_ref(),
                }
                .ok_or(DocumentLayoutError::MissingAxes)?;
                let y_axis = match binding.y {
                    YAxisSlot::Y1 => Some(&axes.y),
                    YAxisSlot::Y2 => axes.y2.as_ref(),
                }
                .ok_or(DocumentLayoutError::MissingAxes)?;
                let points = match orientation {
                    ReferenceOrientation::Horizontal => vec![
                        DataPoint {
                            x: x_axis.minimum,
                            y: *value,
                        },
                        DataPoint {
                            x: x_axis.maximum,
                            y: *value,
                        },
                    ],
                    ReferenceOrientation::Vertical => vec![
                        DataPoint {
                            x: *value,
                            y: y_axis.minimum,
                        },
                        DataPoint {
                            x: *value,
                            y: y_axis.maximum,
                        },
                    ],
                };
                (
                    points,
                    Some(LineStyle {
                        width: stroke.width_pt,
                        dash: dash_style(stroke),
                    }),
                    None,
                    Vec::new(),
                    None,
                    stroke.color_id.as_str(),
                )
            }
            ArtistProperties::Annotation { .. } | ArtistProperties::Legend { .. } => continue,
        };
        let legend_label = legend_labels.get(&artist.id).cloned();
        let label = legend_label
            .as_ref()
            .map(instplot_text::Label::normalized_text)
            .unwrap_or_default();
        let binding = artist_binding(artist);
        let group_members = project
            .series_group_for_artist(&artist.id)
            .map(|group| group.artist_ids.iter().collect::<BTreeSet<_>>())
            .unwrap_or_default();
        let legend_marker = if !label.is_empty() && marker.is_none() {
            binding.and_then(|_| {
                axes.artist_ids.iter().find_map(|candidate_id| {
                    let candidate = project.figure.artists.iter().find(|candidate| {
                        candidate.id == *candidate_id
                            && group_members.contains(&candidate.id)
                            && project.artist_effectively_visible(candidate)
                    })?;
                    let ArtistProperties::Scatter { binding: _, marker } = &candidate.properties
                    else {
                        return None;
                    };
                    group_members
                        .contains(&candidate.id)
                        .then_some(LayoutMarkerStyle {
                            shape: marker_shape(marker.shape),
                            size: marker.size_pt,
                            filled: marker.filled,
                            interval: marker.interval.max(1),
                        })
                })
            })
        } else {
            None
        };
        let legend_error = if label.is_empty() {
            None
        } else {
            binding
                .and_then(|_| {
                    axes.artist_ids.iter().find_map(|candidate_id| {
                        let candidate = project.figure.artists.iter().find(|candidate| {
                            candidate.id == *candidate_id
                                && group_members.contains(&candidate.id)
                                && project.artist_effectively_visible(candidate)
                        })?;
                        let ArtistProperties::ErrorBar {
                            binding: _,
                            cap_width_pt,
                            stroke,
                            ..
                        } = &candidate.properties
                        else {
                            return None;
                        };
                        group_members
                            .contains(&candidate.id)
                            .then_some((cap_width_pt, stroke))
                    })
                })
                .map(|(cap_width_pt, stroke)| {
                    Ok::<_, DocumentLayoutError>(LegendErrorStyle {
                        style: ErrorStyle {
                            width: stroke.width_pt,
                            cap_width: *cap_width_pt,
                            dash: dash_style(stroke),
                        },
                        color: palette_color(&stroke.color_id, &project.palette)?,
                    })
                })
                .transpose()?
        };
        let node_id = register_node_id(&artist.id, project_ids)?;
        if let Some(binding) = project.artist_axis_binding(artist) {
            series_axes.insert(node_id, layout_axis_pair(binding));
        }
        output.push(Series {
            id: node_id,
            label,
            legend_label,
            points,
            line,
            marker,
            legend_marker,
            legend_error,
            errors,
            error_style,
            color: palette_color(color_id, &project.palette)?,
        });
    }
    Ok((output, series_axes))
}

fn formal_annotations(
    axes: &crate::AxesRecord,
    project: &ProjectDocument,
    project_ids: &mut BTreeMap<NodeId, String>,
) -> Result<Vec<Annotation>, DocumentLayoutError> {
    let mut output = Vec::new();
    for artist_id in &axes.artist_ids {
        let artist = project
            .figure
            .artists
            .iter()
            .find(|artist| artist.id == *artist_id)
            .ok_or_else(|| DocumentLayoutError::MissingArtist(artist_id.clone()))?;
        if !project.artist_effectively_visible(artist) {
            continue;
        }
        let ArtistProperties::Annotation {
            label_id,
            x_pt,
            y_pt,
            connectors,
        } = &artist.properties
        else {
            continue;
        };
        output.push(Annotation {
            id: register_node_id(&artist.id, project_ids)?,
            labels: split_layout_label_lines(semantic_label(label_id, &project.semantic_registry)?),
            position: AnnotationPosition::FigurePoints { x: *x_pt, y: *y_pt },
            offset_pt: (0.0, 0.0),
            connectors: connectors
                .iter()
                .filter(|connector| connector.axes.is_enabled_in(axes.mode))
                .map(|connector| {
                    Ok(AnnotationConnector {
                        target: DataPoint {
                            x: connector.target_x,
                            y: connector.target_y,
                        },
                        stroke: LineStyle {
                            width: connector.stroke.width_pt,
                            dash: dash_style(&connector.stroke),
                        },
                        color: palette_color(&connector.stroke.color_id, &project.palette)?,
                        start_arrow: connector.start_arrow,
                        end_arrow: connector.end_arrow,
                        arrow_size: connector.arrow_size_pt,
                        axes: layout_axis_pair(connector.axes),
                    })
                })
                .collect::<Result<Vec<_>, DocumentLayoutError>>()?,
        });
    }
    Ok(output)
}

fn legend_spec(
    axes: &crate::AxesRecord,
    project: &ProjectDocument,
    project_ids: &mut BTreeMap<NodeId, String>,
) -> Result<(Option<LegendSpec>, BTreeMap<String, instplot_text::Label>), DocumentLayoutError> {
    for artist_id in &axes.artist_ids {
        let artist = project
            .figure
            .artists
            .iter()
            .find(|artist| artist.id == *artist_id)
            .ok_or_else(|| DocumentLayoutError::MissingArtist(artist_id.clone()))?;
        if !project.artist_effectively_visible(artist) {
            continue;
        }
        let ArtistProperties::Legend {
            entries,
            x_pt,
            y_pt,
            placement,
            position_custom,
            grid,
        } = &artist.properties
        else {
            continue;
        };
        let visible_entries: Vec<_> = entries
            .iter()
            .filter(|entry| {
                entry.visible
                    && project.figure.artists.iter().any(|candidate| {
                        candidate.id == entry.artist_id
                            && project.artist_effectively_visible(candidate)
                    })
            })
            .collect();
        let labels = visible_entries
            .iter()
            .map(|entry| {
                Ok((
                    entry.artist_id.clone(),
                    semantic_label(&entry.label_id, &project.semantic_registry)?,
                ))
            })
            .collect::<Result<_, DocumentLayoutError>>()?;
        let entry_order = visible_entries
            .iter()
            .map(|entry| register_node_id(&entry.artist_id, project_ids))
            .collect::<Result<Vec<_>, _>>()?;
        return Ok((
            Some(LegendSpec {
                id: register_node_id(&artist.id, project_ids)?,
                position: match placement {
                    LegendPlacement::Auto => LegendPosition::Auto,
                    LegendPlacement::Inside => LegendPosition::FigurePoints { x: *x_pt, y: *y_pt },
                    LegendPlacement::Above => LegendPosition::Above,
                    LegendPlacement::Right => LegendPosition::Right,
                },
                manual_position: if *position_custom
                    && matches!(placement, LegendPlacement::Above | LegendPlacement::Right)
                {
                    Some((*x_pt, *y_pt))
                } else {
                    None
                },
                grid: match grid {
                    LegendGrid::Auto => LayoutLegendGrid::Auto,
                    LegendGrid::Rows(rows) => LayoutLegendGrid::Rows(usize::from(*rows)),
                    LegendGrid::Columns(columns) => {
                        LayoutLegendGrid::Columns(usize::from(*columns))
                    }
                },
                entry_order,
            }),
            labels,
        ));
    }
    Ok((None, BTreeMap::new()))
}

fn bound_points(
    binding: &DataBinding,
    project: &ProjectDocument,
) -> Result<Vec<DataPoint>, DocumentLayoutError> {
    let source = project
        .data_sources
        .iter()
        .find(|source| source.id == binding.data_source_id)
        .ok_or_else(|| DocumentLayoutError::MissingDataSource(binding.data_source_id.clone()))?;
    let DataSourcePayload::Embedded { columns, alive, .. } = &source.payload else {
        return Err(DocumentLayoutError::ExternalDataUnavailable(
            source.id.clone(),
        ));
    };
    let column = |name: &str| {
        columns
            .iter()
            .find(|column| column.name == name)
            .ok_or_else(|| DocumentLayoutError::MissingColumn {
                source: source.id.clone(),
                column: name.to_owned(),
            })
    };
    let x = column(&binding.x_column)?;
    let y = column(&binding.y_column)?;
    if x.values.len() != y.values.len() {
        return Err(DocumentLayoutError::ColumnLengthMismatch {
            source: source.id.clone(),
            x: x.values.len(),
            y: y.values.len(),
        });
    }
    Ok(x.values
        .iter()
        .zip(&y.values)
        .enumerate()
        .filter(|(index, _)| {
            alive.get(*index).copied().unwrap_or(true)
                && embedded_value_is_valid(x, *index)
                && embedded_value_is_valid(y, *index)
        })
        .map(|(_, (x, y))| DataPoint { x: *x, y: *y })
        .collect())
}

fn bound_error_data(
    binding: &DataBinding,
    x_error_column: Option<&str>,
    y_error_column: &str,
    project: &ProjectDocument,
) -> Result<(Vec<DataPoint>, Vec<ErrorBar>), DocumentLayoutError> {
    let source = project
        .data_sources
        .iter()
        .find(|source| source.id == binding.data_source_id)
        .ok_or_else(|| DocumentLayoutError::MissingDataSource(binding.data_source_id.clone()))?;
    let DataSourcePayload::Embedded { columns, alive, .. } = &source.payload else {
        return Err(DocumentLayoutError::ExternalDataUnavailable(
            source.id.clone(),
        ));
    };
    let find_column = |name: &str| {
        columns
            .iter()
            .find(|column| column.name == name)
            .ok_or_else(|| DocumentLayoutError::MissingColumn {
                source: source.id.clone(),
                column: name.to_owned(),
            })
    };
    let x = find_column(&binding.x_column)?;
    let y = find_column(&binding.y_column)?;
    let y_error = find_column(y_error_column)?;
    let x_error = x_error_column.map(find_column).transpose()?;
    if x.values.len() != y.values.len()
        || x.values.len() != y_error.values.len()
        || x_error.is_some_and(|error| x.values.len() != error.values.len())
    {
        return Err(DocumentLayoutError::ColumnLengthMismatch {
            source: source.id.clone(),
            x: x.values.len(),
            y: y.values.len().min(y_error.values.len()),
        });
    }
    let mut points = Vec::new();
    let mut errors = Vec::new();
    for index in 0..x.values.len() {
        if alive.get(index).copied().unwrap_or(true)
            && embedded_value_is_valid(x, index)
            && embedded_value_is_valid(y, index)
            && embedded_value_is_valid(y_error, index)
            && x_error.is_none_or(|error| embedded_value_is_valid(error, index))
        {
            points.push(DataPoint {
                x: x.values[index],
                y: y.values[index],
            });
            errors.push(ErrorBar {
                x_minus: x_error.map_or(0.0, |error| error.values[index].abs()),
                x_plus: x_error.map_or(0.0, |error| error.values[index].abs()),
                y_minus: y_error.values[index].abs(),
                y_plus: y_error.values[index].abs(),
            });
        }
    }
    Ok((points, errors))
}

fn embedded_value_is_valid(column: &EmbeddedColumn, index: usize) -> bool {
    column.valid.get(index).copied().unwrap_or(true)
}

fn semantic_label(label_id: &str, labels: &[SemanticLabel]) -> Result<Label, DocumentLayoutError> {
    labels
        .iter()
        .find(|label| label.id == label_id)
        .map(|label| Label::Group(label.nodes.iter().map(label_node).collect()))
        .ok_or_else(|| DocumentLayoutError::MissingLabel(label_id.to_owned()))
}

fn split_layout_label_lines(label: Label) -> Vec<Label> {
    fn split_text(value: String, wrap: impl Fn(String) -> Label) -> Vec<Label> {
        value
            .split('\n')
            .map(|part| wrap(part.to_owned()))
            .collect()
    }
    fn split(label: Label) -> Vec<Label> {
        match label {
            Label::Text(value) => split_text(value, Label::Text),
            Label::Variable(value) => split_text(value, Label::Variable),
            Label::Upright(value) => split_text(value, Label::Upright),
            Label::Number(value) => split_text(value, Label::Number),
            Label::Unit(value) => split_text(value, Label::Unit),
            Label::Operator(value) => split_text(value, Label::Operator),
            Label::Emphasis(value) => split_text(value, Label::Emphasis),
            Label::BoldVariable(value) => split_text(value, Label::BoldVariable),
            Label::DescriptiveSubscript(inner) => split(*inner)
                .into_iter()
                .map(|line| Label::DescriptiveSubscript(Box::new(line)))
                .collect(),
            Label::VariableSubscript(inner) => split(*inner)
                .into_iter()
                .map(|line| Label::VariableSubscript(Box::new(line)))
                .collect(),
            Label::Superscript(inner) => split(*inner)
                .into_iter()
                .map(|line| Label::Superscript(Box::new(line)))
                .collect(),
            Label::Group(children) => {
                let mut lines = vec![Vec::new()];
                for child in children {
                    let parts = split(child);
                    for (index, part) in parts.into_iter().enumerate() {
                        if index > 0 {
                            lines.push(Vec::new());
                        }
                        lines.last_mut().expect("one label line").push(part);
                    }
                }
                lines.into_iter().map(Label::Group).collect()
            }
            other => vec![other],
        }
    }
    let lines = split(label);
    if lines.is_empty() {
        vec![Label::Text(String::new())]
    } else {
        lines
    }
}

fn palette_color(color_id: &str, palette: &PaletteRegistry) -> Result<Color, DocumentLayoutError> {
    palette
        .colors
        .iter()
        .find(|color| color.id == color_id)
        .map(|color| Color(color.rgba[0], color.rgba[1], color.rgba[2], color.rgba[3]))
        .ok_or_else(|| DocumentLayoutError::MissingColor(color_id.to_owned()))
}

fn dash_style(stroke: &StrokeStyle) -> DashStyle {
    match stroke.dash_pt.as_slice() {
        [] => DashStyle::Solid,
        [8.0, 3.0] => DashStyle::LongDash,
        [8.0, 2.0, 2.0, 2.0] => DashStyle::LongShortDash,
        [6.0, 2.0, 0.8, 2.0, 0.8, 2.0] => DashStyle::DashDotDot,
        [dash, gap] if *dash <= stroke.width_pt * 2.0 && *gap > 0.0 => DashStyle::Dotted,
        [_, _] => DashStyle::Dashed,
        _ => DashStyle::DashDot,
    }
}

fn marker_shape(shape: MarkerShape) -> LayoutMarkerShape {
    match shape {
        MarkerShape::Circle => LayoutMarkerShape::Circle,
        MarkerShape::Square => LayoutMarkerShape::Square,
        MarkerShape::Triangle => LayoutMarkerShape::TriangleUp,
        MarkerShape::TriangleDown => LayoutMarkerShape::TriangleDown,
        MarkerShape::Diamond => LayoutMarkerShape::Diamond,
        MarkerShape::Pentagon => LayoutMarkerShape::Pentagon,
        MarkerShape::Star => LayoutMarkerShape::Star,
        MarkerShape::Plus => LayoutMarkerShape::Plus,
        MarkerShape::Cross => LayoutMarkerShape::Cross,
    }
}

fn axis_spec(
    axis: &AxisRecord,
    available_pt: f64,
    labels: &[SemanticLabel],
    project_ids: &mut BTreeMap<NodeId, String>,
    has_data: bool,
) -> Result<AxisSpec, DocumentLayoutError> {
    let semantic = labels
        .iter()
        .find(|label| label.id == axis.label_id)
        .ok_or_else(|| DocumentLayoutError::MissingLabel(axis.label_id.clone()))?;
    let scale = match axis.scale {
        AxisScale::Linear => Scale::Linear,
        AxisScale::Log10 => Scale::Log10,
    };
    let locator = match &axis.locator {
        LocatorSpec::Auto { target_count } => Locator::Auto {
            target_spacing_pt: available_pt / f64::from(*target_count),
        },
        LocatorSpec::Interval { step } => Locator::Interval { step: *step },
        LocatorSpec::Fixed { values } => Locator::Fixed(values.clone()),
    };
    let formatter = match axis.formatter {
        FormatterSpec::Auto => Formatter::Auto,
        FormatterSpec::Decimal { precision } => Formatter::Decimal {
            precision: usize::from(precision),
        },
        FormatterSpec::Scientific { precision } => Formatter::Scientific {
            precision: usize::from(precision),
        },
    };
    // Preserve legacy project data but render the product's inward-only rule.
    let tick_direction = LayoutTickDirection::In;
    Ok(AxisSpec {
        id: register_node_id(&axis.id, project_ids)?,
        label: layout_label_from_nodes(&semantic.nodes),
        minimum: axis.minimum,
        maximum: axis.maximum,
        scale,
        locator,
        minor_interval: axis.minor_interval,
        formatter,
        // Legacy projects may still carry grid flags. Keep them readable, but
        // the canvas-first product does not draw a grid.
        grid: GridSpec {
            major: false,
            minor: false,
        },
        appearance: LayoutAxisAppearance {
            near_spine: axis.appearance.near_spine,
            far_spine: axis.appearance.far_spine,
            near_ticks: axis.appearance.near_ticks,
            far_ticks: axis.appearance.far_ticks,
            near_tick_labels: axis.appearance.near_tick_labels,
            far_tick_labels: axis.appearance.far_tick_labels,
            major_ticks: axis.appearance.major_ticks,
            minor_ticks: axis.appearance.minor_ticks,
            tick_direction,
            tick_label_pad_pt: axis.appearance.tick_label_pad_pt,
            label_edge_pad_pt: axis.appearance.label_edge_pad_pt,
            label_tick_pad_pt: axis.appearance.label_tick_pad_pt,
        },
        has_data,
    })
}

fn layout_axis_pair(binding: AxisBinding) -> LayoutAxisPair {
    match (binding.x, binding.y) {
        (XAxisSlot::X1, YAxisSlot::Y1) => LayoutAxisPair::X1Y1,
        (XAxisSlot::X2, YAxisSlot::Y1) => LayoutAxisPair::X2Y1,
        (XAxisSlot::X1, YAxisSlot::Y2) => LayoutAxisPair::X1Y2,
        (XAxisSlot::X2, YAxisSlot::Y2) => {
            unreachable!("project validation rejects X2/Y2 bindings")
        }
    }
}

/// Convert persisted label semantics using the same shaping rules as preview and export.
pub fn layout_label_from_nodes(nodes: &[LabelNode]) -> Label {
    Label::Group(nodes.iter().map(label_node).collect())
}

fn label_node(node: &LabelNode) -> Label {
    let nested = |nodes: &[LabelNode]| Label::Group(nodes.iter().map(label_node).collect());
    match node {
        LabelNode::Text(value) => Label::Text(value.clone()),
        LabelNode::Variable(value) => Label::Variable(value.clone()),
        LabelNode::Upright(value) => Label::Upright(value.clone()),
        LabelNode::GreekVariable(value) => Label::GreekVariable(*value),
        LabelNode::Number(value) => Label::Number(value.clone()),
        LabelNode::DescriptiveSubscript(nodes) => {
            Label::DescriptiveSubscript(Box::new(nested(nodes)))
        }
        LabelNode::VariableSubscript(nodes) => Label::VariableSubscript(Box::new(nested(nodes))),
        LabelNode::Superscript(nodes) => Label::Superscript(Box::new(nested(nodes))),
        LabelNode::Unit(value) => Label::Unit(value.clone()),
        LabelNode::UnitSeparator => Label::UnitSeparator,
        LabelNode::Operator(value) => Label::Operator(value.clone()),
        LabelNode::Emphasis(value) => Label::Emphasis(value.clone()),
        LabelNode::BoldVariable(value) => Label::BoldVariable(value.clone()),
    }
}

fn register_node_id(
    project_id: &str,
    project_ids: &mut BTreeMap<NodeId, String>,
) -> Result<NodeId, DocumentLayoutError> {
    let numeric = project_id
        .strip_prefix("node-")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or_else(|| fnv1a(project_id.as_bytes()));
    let node = NodeId(numeric);
    if let Some(first) = project_ids.get(&node)
        && first != project_id
    {
        return Err(DocumentLayoutError::DuplicateNodeId {
            numeric,
            first: first.clone(),
            second: project_id.to_owned(),
        });
    }
    project_ids.insert(node, project_id.to_owned());
    Ok(node)
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod document_tests;
