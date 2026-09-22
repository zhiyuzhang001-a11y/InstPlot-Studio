use core::fmt;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use instplot_core::{DataSet, DataSetKind};
use instplot_layout::{
    Annotation, AnnotationPosition, AxisSpec, Bounds, Chart, DashStyle, DataPoint, ErrorBar,
    ErrorStyle, Formatter, GridSpec, LayoutError, LayoutResult, LegendPosition, LegendSpec,
    LineStyle, Locator, MarkerShape as LayoutMarkerShape, MarkerStyle as LayoutMarkerStyle, Scale,
    Series, layout,
};
use studio_render_spike::{Color, CompileError, DisplayList, NodeId, compile, fixed_figure};
use text_shaping_spike::Label;

use crate::{
    ArtistKind, ArtistProperties, ArtistRecord, ArtistRole, AxisRecord, AxisScale, DataBinding,
    DataSourceKind, DataSourcePayload, EmbeddedColumn, FitIdentity, FormatterSpec, LabelNode,
    LegendEntry, LocatorSpec, MarkerShape, MarkerStyle, OpenProjectReport, PaletteColor,
    PaletteRegistry, ProjectDocument, ProjectError, ProvenanceRecord, ReferenceOrientation,
    SemanticLabel, StrokeStyle, open_project, save_project,
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
    pub label: String,
    pub binding: Option<DataBinding>,
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

impl FigureDocument {
    pub fn fixed() -> Self {
        Self {
            project: ProjectDocument::fixed_fixture(),
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
                            width_pt: 0.9,
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
            });
            project.figure.axes[0].artist_ids.push(artist_id);
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
                x_pt: 164.0,
                y_pt: 30.0,
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
        )?;
        let y = axis_spec(
            &stored.y,
            height_pt,
            &self.project.semantic_registry,
            &mut project_ids,
        )?;
        let (legend, legend_labels) = if include_artists {
            legend_spec(stored, &self.project, &mut project_ids)?
        } else {
            (None, BTreeMap::new())
        };
        let series = if include_artists {
            formal_series(stored, &self.project, &legend_labels, &mut project_ids)?
        } else {
            Vec::new()
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

    pub fn axis_ranges(&self) -> AxisRanges {
        let axes = &self.project.figure.axes[0];
        AxisRanges {
            x_min: axes.x.minimum,
            x_max: axes.x.maximum,
            y_min: axes.y.minimum,
            y_max: axes.y.maximum,
        }
    }

    pub fn set_axis_ranges(&mut self, ranges: AxisRanges) -> Result<(), &'static str> {
        if !ranges.x_min.is_finite()
            || !ranges.x_max.is_finite()
            || !ranges.y_min.is_finite()
            || !ranges.y_max.is_finite()
        {
            return Err("axis ranges must be finite");
        }
        if ranges.x_min >= ranges.x_max || ranges.y_min >= ranges.y_max {
            return Err("each axis minimum must be smaller than its maximum");
        }
        let axes = &mut self.project.figure.axes[0];
        axes.x.minimum = ranges.x_min;
        axes.x.maximum = ranges.x_max;
        axes.y.minimum = ranges.y_min;
        axes.y.maximum = ranges.y_max;
        Ok(())
    }

    pub fn create_series(
        &mut self,
        data_source_id: &str,
        x_column: &str,
        y_column: &str,
        style: SeriesCreationStyle,
    ) -> Result<Vec<String>, String> {
        validate_binding_columns(&self.project, data_source_id, x_column, y_column)?;
        let source = self
            .project
            .data_sources
            .iter()
            .find(|source| source.id == data_source_id)
            .ok_or_else(|| format!("data source {data_source_id} is missing"))?;
        let role = match source.kind {
            DataSourceKind::Source => ArtistRole::Data,
            DataSourceKind::Fit => ArtistRole::Fit,
        };
        let source_label = source.label.clone();
        let color_id = default_series_color(&self.project, data_source_id);
        let kinds = match style {
            SeriesCreationStyle::Line => vec![ArtistKind::Line],
            SeriesCreationStyle::Scatter => vec![ArtistKind::Scatter],
            SeriesCreationStyle::LineAndMarker => vec![ArtistKind::Line, ArtistKind::Scatter],
        };
        let mut created = Vec::new();
        let label_id = next_stable_id(&self.project, "series-label");
        self.project.semantic_registry.push(SemanticLabel {
            id: label_id.clone(),
            nodes: vec![LabelNode::Text(source_label)],
        });
        for kind in kinds {
            let id = next_stable_id(&self.project, "series");
            let binding = DataBinding {
                data_source_id: data_source_id.to_owned(),
                x_column: x_column.to_owned(),
                y_column: y_column.to_owned(),
            };
            let properties = match kind {
                ArtistKind::Line => ArtistProperties::Line {
                    binding,
                    stroke: StrokeStyle {
                        color_id: color_id.clone(),
                        width_pt: 0.9,
                        dash_pt: Vec::new(),
                    },
                },
                ArtistKind::Scatter => ArtistProperties::Scatter {
                    binding,
                    marker: MarkerStyle {
                        color_id: color_id.clone(),
                        shape: MarkerShape::Circle,
                        size_pt: 4.0,
                    },
                },
                _ => unreachable!("series creation only constructs line/scatter artists"),
            };
            self.project.figure.artists.push(ArtistRecord {
                id: id.clone(),
                kind,
                role,
                visible: true,
                properties,
            });
            self.project.figure.axes[0].artist_ids.push(id.clone());
            created.push(id);
        }
        let legend_entry = LegendEntry {
            artist_id: created[0].clone(),
            label_id,
        };
        if let Some(legend) = self
            .project
            .figure
            .artists
            .iter_mut()
            .find(|artist| artist.kind == ArtistKind::Legend)
        {
            let ArtistProperties::Legend { entries, .. } = &mut legend.properties else {
                unreachable!("legend kind and properties are validated together")
            };
            entries.push(legend_entry);
        } else {
            let legend_id = next_stable_id(&self.project, "legend");
            self.project.figure.artists.push(ArtistRecord {
                id: legend_id.clone(),
                kind: ArtistKind::Legend,
                role: ArtistRole::Legend,
                visible: true,
                properties: ArtistProperties::Legend {
                    entries: vec![legend_entry],
                    x_pt: 164.0,
                    y_pt: 30.0,
                },
            });
            self.project.figure.axes[0].artist_ids.push(legend_id);
        }
        Ok(created)
    }

    pub fn duplicate_series(&mut self, artist_id: &str) -> Result<String, String> {
        let artist = self
            .project
            .figure
            .artists
            .iter()
            .find(|artist| artist.id == artist_id)
            .cloned()
            .ok_or_else(|| format!("artist {artist_id} is missing"))?;
        if !matches!(
            artist.kind,
            ArtistKind::Line | ArtistKind::Scatter | ArtistKind::ErrorBar
        ) {
            return Err("only data series can be duplicated".to_owned());
        }
        let new_id = next_stable_id(&self.project, "series");
        let mut duplicate = artist;
        duplicate.id = new_id.clone();
        self.project.figure.artists.push(duplicate);
        let axes = &mut self.project.figure.axes[0];
        let position = axes
            .artist_ids
            .iter()
            .position(|id| id == artist_id)
            .ok_or_else(|| format!("artist {artist_id} is not attached to the axes"))?;
        axes.artist_ids.insert(position + 1, new_id.clone());
        for candidate in &mut self.project.figure.artists {
            if let ArtistProperties::Legend { entries, .. } = &mut candidate.properties
                && let Some(entry) = entries
                    .iter()
                    .find(|entry| entry.artist_id == artist_id)
                    .cloned()
            {
                entries.push(LegendEntry {
                    artist_id: new_id.clone(),
                    label_id: entry.label_id,
                });
                break;
            }
        }
        Ok(new_id)
    }

    pub fn delete_series(&mut self, artist_id: &str) -> Result<(), String> {
        let position = self
            .project
            .figure
            .artists
            .iter()
            .position(|artist| artist.id == artist_id)
            .ok_or_else(|| format!("artist {artist_id} is missing"))?;
        if self.project.figure.artists[position].kind == ArtistKind::Legend {
            return Err("the legend is managed separately from data series".to_owned());
        }
        self.project.figure.artists.remove(position);
        self.project.figure.axes[0]
            .artist_ids
            .retain(|id| id != artist_id);
        prune_legend_entries(&mut self.project, &[artist_id.to_owned()]);
        Ok(())
    }

    pub fn set_series_visible(&mut self, artist_id: &str, visible: bool) -> Result<(), String> {
        let artist = self
            .project
            .figure
            .artists
            .iter_mut()
            .find(|artist| artist.id == artist_id)
            .ok_or_else(|| format!("artist {artist_id} is missing"))?;
        artist.visible = visible;
        Ok(())
    }

    pub fn move_series(&mut self, artist_id: &str, direction: MoveDirection) -> Result<(), String> {
        let artist_ids = &mut self.project.figure.axes[0].artist_ids;
        let position = artist_ids
            .iter()
            .position(|id| id == artist_id)
            .ok_or_else(|| format!("artist {artist_id} is not attached to the axes"))?;
        let destination = match direction {
            MoveDirection::Earlier if position > 0 => position - 1,
            MoveDirection::Later if position + 1 < artist_ids.len() => position + 1,
            _ => return Ok(()),
        };
        artist_ids.swap(position, destination);
        let order = artist_ids
            .iter()
            .enumerate()
            .map(|(index, id)| (id.clone(), index))
            .collect::<BTreeMap<_, _>>();
        for artist in &mut self.project.figure.artists {
            if let ArtistProperties::Legend { entries, .. } = &mut artist.properties {
                entries.sort_by_key(|entry| {
                    order.get(&entry.artist_id).copied().unwrap_or(usize::MAX)
                });
            }
        }
        Ok(())
    }

    pub fn rebind_series(
        &mut self,
        artist_id: &str,
        data_source_id: &str,
        x_column: &str,
        y_column: &str,
        y_error_column: Option<&str>,
    ) -> Result<(), String> {
        validate_binding_columns(&self.project, data_source_id, x_column, y_column)?;
        if let Some(error_column) = y_error_column {
            validate_column(&self.project, data_source_id, error_column)?;
        }
        let artist = self
            .project
            .figure
            .artists
            .iter_mut()
            .find(|artist| artist.id == artist_id)
            .ok_or_else(|| format!("artist {artist_id} is missing"))?;
        let replacement = DataBinding {
            data_source_id: data_source_id.to_owned(),
            x_column: x_column.to_owned(),
            y_column: y_column.to_owned(),
        };
        match &mut artist.properties {
            ArtistProperties::Line { binding, .. } | ArtistProperties::Scatter { binding, .. } => {
                *binding = replacement
            }
            ArtistProperties::ErrorBar {
                binding,
                y_error_column: current,
                ..
            } => {
                *binding = replacement;
                *current = y_error_column
                    .ok_or_else(|| "error bars require an error column".to_owned())?
                    .to_owned();
            }
            _ => return Err("the selected object has no data binding".to_owned()),
        }
        Ok(())
    }

    pub fn data_source_dependency_count(&self, data_source_id: &str) -> usize {
        let dependent_sources = dependent_source_ids(&self.project, data_source_id);
        let mut source_ids = dependent_sources.clone();
        source_ids.push(data_source_id.to_owned());
        dependent_sources.len()
            + self
                .project
                .figure
                .artists
                .iter()
                .filter(|artist| source_ids.iter().any(|id| artist_uses_source(artist, id)))
                .count()
    }

    pub fn delete_data_source(
        &mut self,
        data_source_id: &str,
        cascade: bool,
    ) -> Result<(), String> {
        if !self
            .project
            .data_sources
            .iter()
            .any(|source| source.id == data_source_id)
        {
            return Err(format!("data source {data_source_id} is missing"));
        }
        let mut source_ids = dependent_source_ids(&self.project, data_source_id);
        source_ids.push(data_source_id.to_owned());
        let artist_ids = self
            .project
            .figure
            .artists
            .iter()
            .filter(|artist| source_ids.iter().any(|id| artist_uses_source(artist, id)))
            .map(|artist| artist.id.clone())
            .collect::<Vec<_>>();
        if !cascade && (source_ids.len() > 1 || !artist_ids.is_empty()) {
            return Err(format!(
                "data source has {} dependent source(s) and {} bound artist(s)",
                source_ids.len() - 1,
                artist_ids.len()
            ));
        }
        self.project
            .data_sources
            .retain(|source| !source_ids.contains(&source.id));
        self.project
            .figure
            .artists
            .retain(|artist| !artist_ids.contains(&artist.id));
        self.project.figure.axes[0]
            .artist_ids
            .retain(|id| !artist_ids.contains(id));
        prune_legend_entries(&mut self.project, &artist_ids);
        Ok(())
    }

    pub fn series(&self) -> Vec<SeriesDescriptor> {
        self.project
            .figure
            .artists
            .iter()
            .map(|artist| {
                let kind = match artist.kind {
                    crate::ArtistKind::Line => SeriesKind::Line,
                    crate::ArtistKind::Scatter => SeriesKind::Scatter,
                    crate::ArtistKind::ErrorBar => SeriesKind::ErrorBar,
                    crate::ArtistKind::ReferenceLine => SeriesKind::ReferenceLine,
                    crate::ArtistKind::Annotation => SeriesKind::Annotation,
                    crate::ArtistKind::Legend => SeriesKind::Legend,
                };
                SeriesDescriptor {
                    id: artist.id.clone(),
                    kind,
                    role: artist.role,
                    visible: artist.visible,
                    label: artist_binding(artist)
                        .and_then(|binding| {
                            self.project
                                .data_sources
                                .iter()
                                .find(|source| source.id == binding.data_source_id)
                        })
                        .map_or_else(|| artist.id.clone(), |source| source.label.clone()),
                    binding: artist_binding(artist).cloned(),
                }
            })
            .collect()
    }

    pub fn project(&self) -> &ProjectDocument {
        &self.project
    }

    pub fn from_project(project: ProjectDocument) -> Result<Self, ProjectError> {
        project.validate()?;
        Ok(Self { project })
    }

    pub fn open(path: &Path) -> Result<(Self, OpenProjectReport), ProjectError> {
        let report = open_project(path)?;
        let document = Self::from_project(report.document.clone())?;
        Ok((document, report))
    }

    pub fn save(&self, path: &Path) -> Result<(), ProjectError> {
        save_project(path, &self.project)
    }

    pub fn sync_datasets(&mut self, datasets: &[DataSet]) -> Result<(), ProjectError> {
        let mut candidate = self.project.clone();
        for dataset in datasets
            .iter()
            .filter(|dataset| dataset.kind == DataSetKind::Source)
        {
            candidate.upsert_embedded_source(
                dataset.plot_id.clone(),
                dataset.display_name(),
                embedded_columns(dataset),
                dataset.alive.clone(),
                DataSourceKind::Source,
                None,
            )?;
        }
        for dataset in datasets
            .iter()
            .filter(|dataset| dataset.kind == DataSetKind::Fit)
        {
            let fit = dataset.fit_link.as_ref().ok_or_else(|| {
                ProjectError::Validation(format!(
                    "fit data source {} is missing its fit link",
                    dataset.plot_id
                ))
            })?;
            candidate.upsert_embedded_source(
                dataset.plot_id.clone(),
                dataset.display_name(),
                embedded_columns(dataset),
                dataset.alive.clone(),
                DataSourceKind::Fit,
                Some(FitIdentity {
                    parent_data_source_id: fit.parent_dataset_id.clone().ok_or_else(|| {
                        ProjectError::Validation(format!(
                            "fit data source {} is missing its parent identity",
                            dataset.plot_id
                        ))
                    })?,
                    source_x_column: fit.source_x_column.clone(),
                    source_y_column: fit.source_y_column.clone(),
                    equation: fit.equation.clone(),
                    display_equation: fit.display_equation.clone(),
                }),
            )?;
        }
        self.project = candidate;
        Ok(())
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

fn default_series_color(project: &ProjectDocument, data_source_id: &str) -> String {
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
        if binding_family != family_id {
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
    project
        .palette
        .colors
        .iter()
        .find(|color| color.id == "blue")
        .or_else(|| project.palette.colors.first())
        .map(|color| color.id.clone())
        .unwrap_or_else(|| "blue".to_owned())
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
    for artist in &mut project.figure.artists {
        if let ArtistProperties::Legend { entries, .. } = &mut artist.properties {
            entries.retain(|entry| !removed_artist_ids.contains(&entry.artist_id));
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

const HANDOFF_MARKERS: [MarkerShape; 4] = [
    MarkerShape::Circle,
    MarkerShape::Square,
    MarkerShape::Triangle,
    MarkerShape::Diamond,
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
            || dataset
                .columns
                .iter()
                .flat_map(|column| &column.values)
                .any(|value| !value.is_finite())
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

fn embedded_columns(dataset: &DataSet) -> Vec<EmbeddedColumn> {
    dataset
        .columns
        .iter()
        .map(|column| EmbeddedColumn {
            name: column.name.clone(),
            values: column.values.clone(),
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
            .filter(|(index, _)| dataset.alive[*index])
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
    let x_label = project
        .semantic_registry
        .iter_mut()
        .find(|label| label.id == axes.x.label_id)
        .expect("fixed document has an X label");
    x_label.nodes = vec![LabelNode::Variable(x_column)];
    let y_label = project
        .semantic_registry
        .iter_mut()
        .find(|label| label.id == axes.y.label_id)
        .expect("fixed document has a Y label");
    y_label.nodes = vec![LabelNode::Variable(y_column)];
    Ok(())
}

fn formal_series(
    axes: &crate::AxesRecord,
    project: &ProjectDocument,
    legend_labels: &BTreeMap<String, String>,
    project_ids: &mut BTreeMap<NodeId, String>,
) -> Result<Vec<Series>, DocumentLayoutError> {
    let mut output = Vec::new();
    for artist_id in &axes.artist_ids {
        let artist = project
            .figure
            .artists
            .iter()
            .find(|artist| artist.id == *artist_id)
            .ok_or_else(|| DocumentLayoutError::MissingArtist(artist_id.clone()))?;
        if !artist.visible {
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
                    filled: true,
                }),
                Vec::new(),
                None,
                marker.color_id.as_str(),
            ),
            ArtistProperties::ErrorBar {
                binding,
                y_error_column,
                cap_width_pt,
                stroke,
            } => {
                let points = bound_points(binding, project)?;
                let values = bound_column(&binding.data_source_id, y_error_column, project)?;
                if points.len() != values.len() {
                    return Err(DocumentLayoutError::ColumnLengthMismatch {
                        source: binding.data_source_id.clone(),
                        x: points.len(),
                        y: values.len(),
                    });
                }
                (
                    points,
                    None,
                    None,
                    values
                        .into_iter()
                        .map(|value| ErrorBar {
                            x_minus: 0.0,
                            x_plus: 0.0,
                            y_minus: value,
                            y_plus: value,
                        })
                        .collect(),
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
                stroke,
            } => {
                let points = match orientation {
                    ReferenceOrientation::Horizontal => vec![
                        DataPoint {
                            x: axes.x.minimum,
                            y: *value,
                        },
                        DataPoint {
                            x: axes.x.maximum,
                            y: *value,
                        },
                    ],
                    ReferenceOrientation::Vertical => vec![
                        DataPoint {
                            x: *value,
                            y: axes.y.minimum,
                        },
                        DataPoint {
                            x: *value,
                            y: axes.y.maximum,
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
        output.push(Series {
            id: register_node_id(&artist.id, project_ids)?,
            label: legend_labels.get(&artist.id).cloned().unwrap_or_default(),
            points,
            line,
            marker,
            errors,
            error_style,
            color: palette_color(color_id, &project.palette)?,
        });
    }
    Ok(output)
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
        if !artist.visible {
            continue;
        }
        let ArtistProperties::Annotation {
            label_id,
            x_pt,
            y_pt,
        } = &artist.properties
        else {
            continue;
        };
        output.push(Annotation {
            id: register_node_id(&artist.id, project_ids)?,
            label: semantic_label(label_id, &project.semantic_registry)?,
            position: AnnotationPosition::FigurePoints { x: *x_pt, y: *y_pt },
            offset_pt: (0.0, 0.0),
        });
    }
    Ok(output)
}

fn legend_spec(
    axes: &crate::AxesRecord,
    project: &ProjectDocument,
    project_ids: &mut BTreeMap<NodeId, String>,
) -> Result<(Option<LegendSpec>, BTreeMap<String, String>), DocumentLayoutError> {
    for artist_id in &axes.artist_ids {
        let artist = project
            .figure
            .artists
            .iter()
            .find(|artist| artist.id == *artist_id)
            .ok_or_else(|| DocumentLayoutError::MissingArtist(artist_id.clone()))?;
        if !artist.visible {
            continue;
        }
        let ArtistProperties::Legend {
            entries,
            x_pt,
            y_pt,
        } = &artist.properties
        else {
            continue;
        };
        let labels = entries
            .iter()
            .filter(|entry| {
                project
                    .figure
                    .artists
                    .iter()
                    .any(|candidate| candidate.id == entry.artist_id && candidate.visible)
            })
            .map(|entry| {
                Ok((
                    entry.artist_id.clone(),
                    semantic_label(&entry.label_id, &project.semantic_registry)?.normalized_text(),
                ))
            })
            .collect::<Result<_, DocumentLayoutError>>()?;
        return Ok((
            Some(LegendSpec {
                id: register_node_id(&artist.id, project_ids)?,
                position: LegendPosition::FigurePoints { x: *x_pt, y: *y_pt },
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
            .map(|column| column.values.as_slice())
            .ok_or_else(|| DocumentLayoutError::MissingColumn {
                source: source.id.clone(),
                column: name.to_owned(),
            })
    };
    let x = column(&binding.x_column)?;
    let y = column(&binding.y_column)?;
    if x.len() != y.len() {
        return Err(DocumentLayoutError::ColumnLengthMismatch {
            source: source.id.clone(),
            x: x.len(),
            y: y.len(),
        });
    }
    Ok(x.iter()
        .zip(y)
        .enumerate()
        .filter(|(index, _)| alive.get(*index).copied().unwrap_or(true))
        .map(|(_, (x, y))| DataPoint { x: *x, y: *y })
        .collect())
}

fn bound_column(
    source_id: &str,
    column_name: &str,
    project: &ProjectDocument,
) -> Result<Vec<f64>, DocumentLayoutError> {
    let source = project
        .data_sources
        .iter()
        .find(|source| source.id == source_id)
        .ok_or_else(|| DocumentLayoutError::MissingDataSource(source_id.to_owned()))?;
    let DataSourcePayload::Embedded { columns, alive, .. } = &source.payload else {
        return Err(DocumentLayoutError::ExternalDataUnavailable(
            source.id.clone(),
        ));
    };
    columns
        .iter()
        .find(|column| column.name == column_name)
        .map(|column| {
            column
                .values
                .iter()
                .enumerate()
                .filter(|(index, _)| alive.get(*index).copied().unwrap_or(true))
                .map(|(_, value)| *value)
                .collect()
        })
        .ok_or_else(|| DocumentLayoutError::MissingColumn {
            source: source.id.clone(),
            column: column_name.to_owned(),
        })
}

fn semantic_label(label_id: &str, labels: &[SemanticLabel]) -> Result<Label, DocumentLayoutError> {
    labels
        .iter()
        .find(|label| label.id == label_id)
        .map(|label| Label::Group(label.nodes.iter().map(label_node).collect()))
        .ok_or_else(|| DocumentLayoutError::MissingLabel(label_id.to_owned()))
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
        MarkerShape::Diamond => LayoutMarkerShape::Diamond,
    }
}

fn axis_spec(
    axis: &AxisRecord,
    available_pt: f64,
    labels: &[SemanticLabel],
    project_ids: &mut BTreeMap<NodeId, String>,
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
    Ok(AxisSpec {
        id: register_node_id(&axis.id, project_ids)?,
        label: Label::Group(semantic.nodes.iter().map(label_node).collect()),
        minimum: axis.minimum,
        maximum: axis.maximum,
        scale,
        locator,
        formatter,
        grid: GridSpec {
            major: true,
            minor: false,
        },
    })
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
mod tests {
    use super::*;
    use instplot_layout::SelectableRole;
    use studio_render_spike::{Color, DisplayItem};

    #[test]
    fn fixed_document_compiles_to_one_deterministic_display_list() {
        let document = FigureDocument::fixed();
        let display = document.compile().unwrap();
        assert!(display.validation_errors().is_empty());
        assert_eq!(document.series().len(), 6);
        assert_eq!(document.series()[1].kind, SeriesKind::Line);
        assert_eq!(document.series()[1].id, "node-11");
    }

    #[test]
    fn axis_edits_mutate_document_state_and_recompile() {
        let mut document = FigureDocument::fixed();
        let ranges = AxisRanges {
            x_min: -4.0,
            x_max: 4.0,
            y_min: -3.0,
            y_max: 3.0,
        };
        document.set_axis_ranges(ranges).unwrap();
        assert_eq!(document.axis_ranges(), ranges);
        assert!(document.compile().is_ok());
    }

    #[test]
    fn invalid_axis_edits_do_not_change_the_document() {
        let mut document = FigureDocument::fixed();
        let before = document.axis_ranges();
        let invalid = AxisRanges {
            x_min: 2.0,
            x_max: 1.0,
            ..before
        };
        assert!(document.set_axis_ranges(invalid).is_err());
        assert_eq!(document.axis_ranges(), before);
    }

    #[test]
    fn project_round_trip_preserves_the_resolved_display_list() {
        let document = FigureDocument::fixed();
        let encoded = serde_json::to_vec(document.project()).unwrap();
        let decoded = crate::project::decode_project(&encoded).unwrap();
        let reopened = FigureDocument::from_project(decoded).unwrap();
        assert_eq!(document.compile().unwrap(), reopened.compile().unwrap());
    }

    #[test]
    fn formal_axes_layout_is_deterministic_and_retains_project_identity() {
        let document = FigureDocument::fixed();
        let first = document.layout_axes().unwrap();
        let second = document.layout_axes().unwrap();

        assert_eq!(first.result.snapshot(), second.result.snapshot());
        assert_eq!(first.data_clip, first.result.axes);
        assert!(first.result.display_list.validation_errors().is_empty());
        assert!(first.result.warnings.is_empty());
        assert_eq!(first.project_ids.get(&NodeId(2)).unwrap(), "node-2");
        assert_eq!(first.project_ids.get(&NodeId(3)).unwrap(), "node-3");
        assert_eq!(first.project_ids.get(&NodeId(4)).unwrap(), "node-4");

        let grid_count = first
            .result
            .display_list
            .items
            .iter()
            .filter(|item| match item {
                DisplayItem::Path {
                    stroke: Some(stroke),
                    ..
                } => stroke.color == Color(218, 221, 224, 255),
                _ => false,
            })
            .count();
        assert_eq!(
            grid_count,
            first.result.x_axis.major.len() + first.result.y_axis.major.len()
        );

        let semantic_runs: Vec<_> = first
            .result
            .display_list
            .items
            .iter()
            .filter_map(|item| match item {
                DisplayItem::GlyphRun(run)
                    if run.source == NodeId(3) || run.source == NodeId(4) =>
                {
                    Some((
                        run.source,
                        run.label.normalized_text(),
                        run.rotation_degrees,
                    ))
                }
                _ => None,
            })
            .collect();
        assert!(semantic_runs.iter().any(|(source, text, rotation)| {
            *source == NodeId(3) && text.contains("μ0HDL") && *rotation == 0.0
        }));
        assert!(semantic_runs.iter().any(|(source, text, rotation)| {
            *source == NodeId(4) && text.contains("Current density") && *rotation == -90.0
        }));
    }

    #[test]
    fn formal_axes_layout_honors_ranges_locators_and_formatters() {
        let mut project = ProjectDocument::fixed_fixture();
        let axes = &mut project.figure.axes[0];
        axes.x.minimum = -1.0;
        axes.x.maximum = 1.0;
        axes.x.locator = LocatorSpec::Fixed {
            values: vec![-1.0, 0.0, 1.0],
        };
        axes.x.formatter = FormatterSpec::Decimal { precision: 2 };
        let document = FigureDocument::from_project(project).unwrap();
        let output = document.layout_axes().unwrap();

        assert_eq!(
            output
                .result
                .x_axis
                .major
                .iter()
                .map(|tick| (tick.value, tick.label.as_str()))
                .collect::<Vec<_>>(),
            vec![(-1.0, "-1"), (0.0, "0"), (1.0, "1")]
        );
    }

    #[test]
    fn non_numeric_project_ids_have_a_stable_reverse_mapping() {
        let mut project = ProjectDocument::fixed_fixture();
        project.figure.axes[0].id = "axes-primary".to_owned();
        project.figure.axes[0].x.id = "axis-horizontal".to_owned();
        project.figure.axes[0].y.id = "axis-vertical".to_owned();
        let document = FigureDocument::from_project(project).unwrap();
        let first = document.layout_axes().unwrap();
        let second = document.layout_axes().unwrap();

        assert_eq!(first.project_ids, second.project_ids);
        assert!(first.project_ids.values().any(|id| id == "axes-primary"));
        assert!(first.project_ids.values().any(|id| id == "axis-horizontal"));
        assert!(first.project_ids.values().any(|id| id == "axis-vertical"));
    }

    #[test]
    fn formal_figure_layout_resolves_basic_line_and_scatter_artists() {
        let output = FigureDocument::fixed().layout_figure().unwrap();

        assert_eq!(output.project_ids.get(&NodeId(11)).unwrap(), "node-11");
        assert_eq!(output.project_ids.get(&NodeId(13)).unwrap(), "node-13");
        assert_eq!(output.project_ids.get(&NodeId(12)).unwrap(), "node-12");
        assert!(
            output
                .result
                .hit_map
                .items
                .iter()
                .any(|item| { item.node == NodeId(11) && item.role == SelectableRole::Series })
        );
        assert_eq!(
            output
                .result
                .hit_map
                .items
                .iter()
                .filter(|item| {
                    item.node == NodeId(13) && item.role == SelectableRole::DataPoint
                })
                .count(),
            3
        );
        assert!(output.result.display_list.validation_errors().is_empty());
    }

    #[test]
    fn matching_line_and_scatter_bindings_form_a_combined_visual_series() {
        let mut project = ProjectDocument::fixed_fixture();
        let scatter = project
            .figure
            .artists
            .iter_mut()
            .find(|artist| artist.id == "node-13")
            .unwrap();
        let ArtistProperties::Scatter { binding, .. } = &mut scatter.properties else {
            panic!("fixed node-13 must remain a scatter artist")
        };
        binding.x_column = "line_x".to_owned();
        binding.y_column = "line_y".to_owned();

        let output = FigureDocument::from_project(project)
            .unwrap()
            .layout_figure()
            .unwrap();
        let line_points = output
            .result
            .hit_map
            .items
            .iter()
            .find(|item| item.node == NodeId(11) && item.role == SelectableRole::Series)
            .unwrap()
            .path_proximity
            .clone();
        let marker_points: Vec<_> = output
            .result
            .hit_map
            .items
            .iter()
            .filter(|item| item.node == NodeId(13) && item.role == SelectableRole::DataPoint)
            .flat_map(|item| item.path_proximity.iter().copied())
            .collect();
        assert_eq!(line_points, marker_points);
    }

    #[test]
    fn series_management_is_valid_deterministic_and_round_trips() {
        let mut document = FigureDocument::fixed();
        let created = document
            .create_series(
                "fixture-data",
                "line_x",
                "line_y",
                SeriesCreationStyle::LineAndMarker,
            )
            .unwrap();
        assert_eq!(created, ["series-1", "series-2"]);
        document.project().validate().unwrap();
        document.layout_figure().unwrap();

        document.set_series_visible(&created[0], false).unwrap();
        assert!(
            !document
                .series()
                .iter()
                .find(|item| item.id == created[0])
                .unwrap()
                .visible
        );
        let duplicate = document.duplicate_series(&created[1]).unwrap();
        assert_eq!(duplicate, "series-3");
        document
            .move_series(&duplicate, MoveDirection::Earlier)
            .unwrap();
        document
            .rebind_series(&duplicate, "fixture-data", "scatter_x", "scatter_y", None)
            .unwrap();
        document.delete_series(&created[1]).unwrap();
        document.project().validate().unwrap();

        let encoded = serde_json::to_vec(document.project()).unwrap();
        let reopened =
            FigureDocument::from_project(crate::project::decode_project(&encoded).unwrap())
                .unwrap();
        assert_eq!(reopened.project(), document.project());
        assert!(
            !reopened
                .series()
                .iter()
                .find(|item| item.id == created[0])
                .unwrap()
                .visible
        );
    }

    #[test]
    fn data_source_deletion_requires_explicit_cascade_and_cleans_dependencies() {
        let mut document = FigureDocument::fixed();
        let before = document.clone();
        assert!(document.delete_data_source("fixture-data", false).is_err());
        assert_eq!(document, before);

        document.delete_data_source("fixture-data", true).unwrap();
        assert!(document.project().data_sources.is_empty());
        assert!(document.project().figure.artists.iter().all(|artist| {
            !matches!(
                artist.kind,
                ArtistKind::Line | ArtistKind::Scatter | ArtistKind::ErrorBar | ArtistKind::Legend
            )
        }));
        document.project().validate().unwrap();
        document.layout_figure().unwrap();
    }

    #[test]
    fn ordinary_import_is_embedded_before_series_creation() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("smoke.csv");
        let datasets = instplot_io::read_data_file(&fixture).unwrap();
        let imported_id = datasets[0].plot_id.clone();
        let mut document = FigureDocument::fixed();
        document.sync_datasets(&datasets).unwrap();
        let source = document
            .project()
            .data_sources
            .iter()
            .find(|source| source.id == imported_id)
            .unwrap();
        assert!(matches!(source.payload, DataSourcePayload::Embedded { .. }));
        document
            .create_series(
                &imported_id,
                "field",
                "response",
                SeriesCreationStyle::Scatter,
            )
            .unwrap();
        document.project().validate().unwrap();
        document.layout_figure().unwrap();
    }

    #[test]
    fn formal_figure_layout_includes_remaining_single_axes_artists() {
        let output = FigureDocument::fixed().layout_figure().unwrap();

        assert!(
            output
                .result
                .hit_map
                .items
                .iter()
                .any(|item| { item.node == NodeId(10) && item.role == SelectableRole::Series })
        );
        assert_eq!(
            output
                .result
                .hit_map
                .items
                .iter()
                .filter(|item| item.node == NodeId(12) && item.role == SelectableRole::ErrorBar)
                .count(),
            3
        );
        assert!(
            output
                .result
                .hit_map
                .items
                .iter()
                .any(|item| { item.node == NodeId(14) && item.role == SelectableRole::Annotation })
        );
        assert!(
            output
                .result
                .hit_map
                .items
                .iter()
                .any(|item| { item.node == NodeId(15) && item.role == SelectableRole::Legend })
        );
        assert_eq!(output.project_ids.get(&NodeId(14)).unwrap(), "node-14");
        assert_eq!(output.project_ids.get(&NodeId(15)).unwrap(), "node-15");

        let labels: Vec<_> = output
            .result
            .display_list
            .items
            .iter()
            .filter_map(|item| match item {
                DisplayItem::GlyphRun(run) => Some((run.source, run.label.normalized_text())),
                _ => None,
            })
            .collect();
        assert!(labels.contains(&(NodeId(14), "T ≤ 300 K".to_owned())));
        assert!(labels.contains(&(NodeId(13), "Experiment".to_owned())));
        assert!(labels.contains(&(NodeId(11), "Fit".to_owned())));
    }

    #[test]
    fn formal_figure_layout_supports_positive_log_data() {
        let mut document = FigureDocument::fixed();
        let axes = &mut document.project.figure.axes[0];
        axes.x.minimum = 0.1;
        axes.x.maximum = 10.0;
        axes.x.scale = AxisScale::Log10;
        axes.y.minimum = 0.1;
        axes.y.maximum = 10.0;
        axes.y.scale = AxisScale::Log10;
        if let ArtistProperties::ReferenceLine { value, .. } =
            &mut document.project.figure.artists[0].properties
        {
            *value = 1.0;
        }
        let DataSourcePayload::Embedded { columns, .. } =
            &mut document.project.data_sources[0].payload
        else {
            unreachable!()
        };
        for column in columns {
            match column.name.as_str() {
                "line_x" | "scatter_x" => column.values = vec![0.1, 1.0, 10.0],
                "line_y" | "scatter_y" => column.values = vec![0.2, 2.0, 8.0],
                _ => {}
            }
        }

        let output = document.layout_figure().unwrap();
        assert_eq!(
            output
                .result
                .x_axis
                .major
                .iter()
                .map(|tick| tick.value)
                .collect::<Vec<_>>(),
            vec![0.1, 1.0, 10.0]
        );
        assert!(output.result.display_list.validation_errors().is_empty());
    }
}
