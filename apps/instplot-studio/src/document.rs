use core::fmt;
use std::collections::BTreeMap;
use std::path::Path;

use instplot_core::{DataSet, DataSetKind};
use instplot_layout::{
    AxisSpec, Bounds, Chart, DashStyle, DataPoint, Formatter, GridSpec, LayoutError, LayoutResult,
    LineStyle, Locator, MarkerShape as LayoutMarkerShape, MarkerStyle as LayoutMarkerStyle, Scale,
    Series, layout,
};
use studio_render_spike::{Color, CompileError, DisplayList, NodeId, compile, fixed_figure};
use text_shaping_spike::Label;

use crate::{
    ArtistProperties, AxisRecord, AxisScale, DataBinding, DataSourceKind, DataSourcePayload,
    FitIdentity, FormatterSpec, LabelNode, LocatorSpec, MarkerShape, OpenProjectReport,
    PaletteRegistry, ProjectDocument, ProjectError, SemanticLabel, StrokeStyle, open_project,
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
    pub label: String,
}

impl FigureDocument {
    pub fn fixed() -> Self {
        Self {
            project: ProjectDocument::fixed_fixture(),
        }
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
        include_basic_artists: bool,
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
        let series = if include_basic_artists {
            basic_series(stored, &self.project, &mut project_ids)?
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
            annotations: Vec::new(),
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
                    label: format!("{kind} · {}", artist.id),
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

    pub fn sync_external_datasets(&mut self, datasets: &[DataSet]) -> Result<(), ProjectError> {
        let mut candidate = self.project.clone();
        for dataset in datasets
            .iter()
            .filter(|dataset| dataset.kind == DataSetKind::Source)
        {
            candidate.upsert_external_source(
                dataset.plot_id.clone(),
                dataset.display_name(),
                &dataset.source,
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
            candidate.upsert_external_source(
                dataset.plot_id.clone(),
                dataset.display_name(),
                &dataset.source,
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
                }),
            )?;
        }
        self.project = candidate;
        Ok(())
    }
}

fn basic_series(
    axes: &crate::AxesRecord,
    project: &ProjectDocument,
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
        let (binding, line, marker, color_id) = match &artist.properties {
            ArtistProperties::Line { binding, stroke } => (
                binding,
                Some(LineStyle {
                    width: stroke.width_pt,
                    dash: dash_style(stroke),
                }),
                None,
                stroke.color_id.as_str(),
            ),
            ArtistProperties::Scatter { binding, marker } => (
                binding,
                None,
                Some(LayoutMarkerStyle {
                    shape: marker_shape(marker.shape),
                    size: marker.size_pt,
                    filled: true,
                }),
                marker.color_id.as_str(),
            ),
            _ => continue,
        };
        output.push(Series {
            id: register_node_id(&artist.id, project_ids)?,
            label: artist.id.clone(),
            points: bound_points(binding, project)?,
            line,
            marker,
            errors: Vec::new(),
            color: palette_color(color_id, &project.palette)?,
        });
    }
    Ok(output)
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
    let DataSourcePayload::Embedded { columns, .. } = &source.payload else {
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
        .map(|(x, y)| DataPoint { x: *x, y: *y })
        .collect())
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
        assert!(!output.project_ids.contains_key(&NodeId(12)));
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
}
