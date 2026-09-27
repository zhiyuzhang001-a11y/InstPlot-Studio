use crate::{
    ArtistRecord, AxisDimension, AxisRanges, AxisRecord, DocumentLayout, ExportPreferences,
    FigureDocument, LabelNode, MoveDirection, ProjectDocument, SeriesCreationStyle,
};

const MAX_HISTORY: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditGroup {
    AxisXMinimum,
    AxisXMaximum,
    AxisYMinimum,
    AxisYMaximum,
    FigureWidth,
    FigureHeight,
    AxisXSettings,
    AxisYSettings,
    ArtistProperties,
    SemanticLabel,
}

pub enum EditCommand {
    SetAxisRanges(AxisRanges),
    AddAnnotation {
        nodes: Vec<LabelNode>,
    },
    CreateSeries {
        data_source_id: String,
        x_column: String,
        y_column: String,
        style: SeriesCreationStyle,
    },
    DuplicateSeries {
        artist_id: String,
    },
    DeleteSeries {
        artist_id: String,
    },
    SetSeriesVisible {
        artist_id: String,
        visible: bool,
    },
    SetSeriesStyle {
        artist_id: String,
        style: SeriesCreationStyle,
    },
    MoveSeries {
        artist_id: String,
        direction: MoveDirection,
    },
    RebindSeries {
        artist_id: String,
        data_source_id: String,
        x_column: String,
        y_column: String,
        y_error_column: Option<String>,
    },
    SetSeriesErrorColumns {
        artist_id: String,
        x_error_column: Option<String>,
        y_error_column: Option<String>,
    },
    DeleteDataSource {
        data_source_id: String,
        cascade: bool,
    },
    DeleteDataSources {
        data_source_ids: Vec<String>,
    },
    SetAxisRecord {
        dimension: AxisDimension,
        record: AxisRecord,
    },
    SetAxisLabel {
        dimension: AxisDimension,
        nodes: Vec<LabelNode>,
    },
    SetFigureSize {
        width_mm: f64,
        height_mm: f64,
    },
    SetArtistRecord(ArtistRecord),
    SetAllMarkerDensity {
        size_pt: f64,
        interval: usize,
    },
    SetAllMarkerSizes {
        size_pt: f64,
    },
    SetAllMarkerIntervals {
        interval: usize,
    },
    SetAllMarkerFilled {
        filled: bool,
    },
    SetPalette {
        palette_id: String,
    },
    SetSemanticLabel {
        label_id: String,
        nodes: Vec<LabelNode>,
    },
    SetExportPreferences(ExportPreferences),
}

impl EditCommand {
    fn description(&self) -> &'static str {
        match self {
            Self::SetAxisRanges(_) => "Change axes ranges",
            Self::AddAnnotation { .. } => "Add text annotation",
            Self::CreateSeries { .. } => "Create series",
            Self::DuplicateSeries { .. } => "Duplicate series",
            Self::DeleteSeries { .. } => "Delete series",
            Self::SetSeriesVisible { visible: true, .. } => "Show series",
            Self::SetSeriesVisible { visible: false, .. } => "Hide series",
            Self::SetSeriesStyle { .. } => "Change plot type",
            Self::MoveSeries { .. } => "Reorder series",
            Self::RebindSeries { .. } => "Change data binding",
            Self::SetSeriesErrorColumns { .. } => "Change error columns",
            Self::DeleteDataSource { .. } => "Delete data source",
            Self::DeleteDataSources { .. } => "Remove imported data",
            Self::SetAxisRecord { .. } => "Change axis settings",
            Self::SetAxisLabel { .. } => "Change axis label",
            Self::SetFigureSize { .. } => "Change figure size",
            Self::SetArtistRecord(_) => "Change artist properties",
            Self::SetAllMarkerDensity { .. } => "Change all marker density",
            Self::SetAllMarkerSizes { .. } => "Change all marker sizes",
            Self::SetAllMarkerIntervals { .. } => "Change all marker intervals",
            Self::SetAllMarkerFilled { .. } => "Change all marker fill styles",
            Self::SetPalette { .. } => "Change colour scheme",
            Self::SetSemanticLabel { .. } => "Change semantic label",
            Self::SetExportPreferences(_) => "Change export settings",
        }
    }

    fn apply(self, document: &mut FigureDocument) -> Result<(), String> {
        match self {
            Self::SetAxisRanges(ranges) => {
                document.set_axis_ranges(ranges).map_err(ToOwned::to_owned)
            }
            Self::AddAnnotation { nodes } => document.add_annotation(nodes).map(|_| ()),
            Self::CreateSeries {
                data_source_id,
                x_column,
                y_column,
                style,
            } => {
                document.create_series(&data_source_id, &x_column, &y_column, style)?;
                document.refresh_autoscale()
            }
            Self::DuplicateSeries { artist_id } => {
                document.duplicate_series(&artist_id).map(|_| ())
            }
            Self::DeleteSeries { artist_id } => document.delete_series(&artist_id),
            Self::SetSeriesVisible { artist_id, visible } => {
                document.set_series_visible(&artist_id, visible)
            }
            Self::SetSeriesStyle { artist_id, style } => {
                document.set_series_style(&artist_id, style)
            }
            Self::MoveSeries {
                artist_id,
                direction,
            } => document.move_series(&artist_id, direction),
            Self::RebindSeries {
                artist_id,
                data_source_id,
                x_column,
                y_column,
                y_error_column,
            } => document.rebind_series(
                &artist_id,
                &data_source_id,
                &x_column,
                &y_column,
                y_error_column.as_deref(),
            ),
            Self::SetSeriesErrorColumns {
                artist_id,
                x_error_column,
                y_error_column,
            } => document.set_series_error_columns(
                &artist_id,
                x_error_column.as_deref(),
                y_error_column.as_deref(),
            ),
            Self::DeleteDataSource {
                data_source_id,
                cascade,
            } => document.delete_data_source(&data_source_id, cascade),
            Self::DeleteDataSources { data_source_ids } => {
                document.delete_data_sources(&data_source_ids)
            }
            Self::SetAxisRecord { dimension, record } => {
                document.set_axis_record(dimension, record)
            }
            Self::SetAxisLabel { dimension, nodes } => document.set_axis_label(dimension, nodes),
            Self::SetFigureSize {
                width_mm,
                height_mm,
            } => document.set_figure_size_mm(width_mm, height_mm),
            Self::SetArtistRecord(record) => document.set_artist_record(record),
            Self::SetAllMarkerDensity { size_pt, interval } => {
                document.set_all_marker_density(size_pt, interval)
            }
            Self::SetAllMarkerSizes { size_pt } => document.set_all_marker_sizes(size_pt),
            Self::SetAllMarkerIntervals { interval } => document.set_all_marker_intervals(interval),
            Self::SetAllMarkerFilled { filled } => document.set_all_marker_filled(filled),
            Self::SetPalette { palette_id } => document.set_palette(&palette_id),
            Self::SetSemanticLabel { label_id, nodes } => {
                document.set_semantic_label_nodes(&label_id, nodes)
            }
            Self::SetExportPreferences(preferences) => document.set_export_preferences(preferences),
        }
    }
}

#[derive(Clone, Debug)]
pub struct EditOutcome {
    pub changed: bool,
    pub description: String,
    pub layout: DocumentLayout,
}

#[derive(Clone)]
struct HistoryEntry {
    document: FigureDocument,
    description: String,
    group: Option<EditGroup>,
}

/// Runtime edit history for one open Figure Document.
///
/// Commands always operate on a clone and validate it before the current
/// document is replaced. The saved snapshot is deliberately runtime-only:
/// undo state is not part of the portable project schema.
#[derive(Clone)]
pub struct EditHistory {
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    saved: Option<ProjectDocument>,
    active_group: Option<EditGroup>,
}

impl EditHistory {
    pub fn new(document: &FigureDocument, has_save_point: bool) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            saved: has_save_point.then(|| document.project().clone()),
            active_group: None,
        }
    }

    pub fn reset(&mut self, document: &FigureDocument, has_save_point: bool) {
        self.undo.clear();
        self.redo.clear();
        self.saved = has_save_point.then(|| document.project().clone());
        self.active_group = None;
    }

    /// Start a new undo baseline after a lifecycle operation such as data import.
    /// The existing save point is retained so the replacement remains dirty.
    pub fn rebase_after_external_change(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.active_group = None;
    }

    pub fn execute(
        &mut self,
        document: &mut FigureDocument,
        command: EditCommand,
        group: Option<EditGroup>,
    ) -> Result<EditOutcome, String> {
        let description = command.description().to_owned();
        let mut candidate = document.clone();
        command.apply(&mut candidate)?;
        candidate
            .project()
            .validate()
            .map_err(|error| error.to_string())?;
        let layout = candidate
            .layout_figure()
            .map_err(|error| format!("edited document cannot be laid out: {error}"))?;
        if candidate.project() == document.project() {
            return Ok(EditOutcome {
                changed: false,
                description,
                layout,
            });
        }

        let coalescing = group.is_some() && group == self.active_group;
        if !coalescing {
            self.undo.push(HistoryEntry {
                document: document.clone(),
                description: description.clone(),
                group,
            });
            if self.undo.len() > MAX_HISTORY {
                self.undo.remove(0);
            }
        }
        *document = candidate;
        self.redo.clear();
        self.active_group = group;
        Ok(EditOutcome {
            changed: true,
            description,
            layout,
        })
    }

    pub fn finish_coalescing(&mut self) {
        self.active_group = None;
    }

    pub fn undo(&mut self, document: &mut FigureDocument) -> Option<String> {
        self.finish_coalescing();
        let entry = self.undo.pop()?;
        let description = entry.description.clone();
        let current = std::mem::replace(document, entry.document);
        self.redo.push(HistoryEntry {
            document: current,
            description: entry.description,
            group: entry.group,
        });
        Some(description)
    }

    pub fn redo(&mut self, document: &mut FigureDocument) -> Option<String> {
        self.finish_coalescing();
        let entry = self.redo.pop()?;
        let description = entry.description.clone();
        let current = std::mem::replace(document, entry.document);
        self.undo.push(HistoryEntry {
            document: current,
            description: entry.description,
            group: entry.group,
        });
        Some(description)
    }

    pub fn mark_saved(&mut self, document: &FigureDocument) {
        self.finish_coalescing();
        self.saved = Some(document.project().clone());
    }

    pub fn is_dirty(&self, document: &FigureDocument) -> bool {
        self.saved
            .as_ref()
            .is_none_or(|saved| saved != document.project())
    }

    pub fn undo_description(&self) -> Option<&str> {
        self.undo.last().map(|entry| entry.description.as_str())
    }

    pub fn redo_description(&self) -> Option<&str> {
        self.redo.last().map(|entry| entry.description.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applying_marker_density_to_all_is_one_undoable_edit() {
        let mut document = FigureDocument::showcase();
        let before = document.clone();
        let mut history = EditHistory::new(&document, true);
        history
            .execute(
                &mut document,
                EditCommand::SetAllMarkerDensity {
                    size_pt: 3.5,
                    interval: 5,
                },
                None,
            )
            .unwrap();
        assert_ne!(document, before);
        history.undo(&mut document).unwrap();
        assert_eq!(document, before);
    }

    #[test]
    fn hidden_legend_and_new_annotations_have_recoverable_edit_paths() {
        let mut document = FigureDocument::showcase();
        let legend_id = document
            .series()
            .into_iter()
            .find(|series| series.kind == crate::SeriesKind::Legend)
            .unwrap()
            .id;
        assert!(!document.artist_record(&legend_id).unwrap().visible);
        let mut history = EditHistory::new(&document, true);
        history
            .execute(
                &mut document,
                EditCommand::SetSeriesVisible {
                    artist_id: legend_id.clone(),
                    visible: true,
                },
                None,
            )
            .unwrap();
        assert!(document.artist_record(&legend_id).unwrap().visible);
        history
            .execute(
                &mut document,
                EditCommand::AddAnnotation {
                    nodes: vec![LabelNode::Text("Text".to_owned())],
                },
                None,
            )
            .unwrap();
        let annotation = document.artist_record("annotation-1").unwrap();
        let crate::ArtistProperties::Annotation { label_id, .. } = annotation.properties else {
            panic!("new artist should be an annotation")
        };
        history
            .execute(
                &mut document,
                EditCommand::SetSemanticLabel {
                    label_id: label_id.clone(),
                    nodes: vec![LabelNode::Text("New note".to_owned())],
                },
                None,
            )
            .unwrap();
        assert_eq!(
            document.semantic_label_nodes(&label_id),
            Some([LabelNode::Text("New note".to_owned())].as_slice())
        );
        let encoded = serde_json::to_vec(document.project()).unwrap();
        let decoded = serde_json::from_slice(&encoded).unwrap();
        let reopened = FigureDocument::from_project(decoded).unwrap();
        reopened.layout_figure().unwrap();
        history.undo(&mut document).unwrap();
        assert_eq!(
            document.semantic_label_nodes(&label_id),
            Some([LabelNode::Text("Text".to_owned())].as_slice())
        );
        history.undo(&mut document).unwrap();
        assert!(document.artist_record("annotation-1").is_none());
        history.redo(&mut document).unwrap();
        history
            .execute(
                &mut document,
                EditCommand::DeleteSeries {
                    artist_id: "annotation-1".to_owned(),
                },
                None,
            )
            .unwrap();
        assert!(document.artist_record("annotation-1").is_none());
        assert!(document.semantic_label_nodes(&label_id).is_none());
        document.project().validate().unwrap();
    }

    fn ranges(x_max: f64) -> AxisRanges {
        AxisRanges {
            x_min: -3.0,
            x_max,
            y_min: -2.5,
            y_max: 2.5,
        }
    }

    #[test]
    fn valid_edits_undo_redo_and_save_points_are_deterministic() {
        let mut document = FigureDocument::fixed();
        let mut history = EditHistory::new(&document, true);
        assert!(!history.is_dirty(&document));

        let outcome = history
            .execute(&mut document, EditCommand::SetAxisRanges(ranges(4.0)), None)
            .unwrap();
        assert!(outcome.changed);
        assert_eq!(
            outcome.layout.result.snapshot(),
            document.layout_figure().unwrap().result.snapshot()
        );
        assert!(history.is_dirty(&document));
        assert_eq!(history.undo_description(), Some("Change axes ranges"));

        history.undo(&mut document).unwrap();
        assert_eq!(document.axis_ranges().x_max, 3.0);
        assert!(!history.is_dirty(&document));
        history.redo(&mut document).unwrap();
        assert_eq!(document.axis_ranges().x_max, 4.0);
        assert!(history.is_dirty(&document));

        history.mark_saved(&document);
        assert!(!history.is_dirty(&document));
    }

    #[test]
    fn invalid_edits_leave_document_and_history_unchanged() {
        let mut document = FigureDocument::fixed();
        let original = document.project().clone();
        let mut history = EditHistory::new(&document, true);
        let error = history
            .execute(
                &mut document,
                EditCommand::SetAxisRanges(ranges(-4.0)),
                None,
            )
            .unwrap_err();
        assert!(error.contains("minimum"));
        assert_eq!(document.project(), &original);
        assert!(history.undo_description().is_none());
        assert!(!history.is_dirty(&document));
    }

    #[test]
    fn continuous_values_coalesce_into_one_undo_step() {
        let mut document = FigureDocument::fixed();
        let mut history = EditHistory::new(&document, true);
        for value in [3.5, 4.0, 4.5] {
            history
                .execute(
                    &mut document,
                    EditCommand::SetAxisRanges(ranges(value)),
                    Some(EditGroup::AxisXMaximum),
                )
                .unwrap();
        }
        assert_eq!(document.axis_ranges().x_max, 4.5);
        history.undo(&mut document).unwrap();
        assert_eq!(document.axis_ranges().x_max, 3.0);
        assert!(history.undo_description().is_none());

        history.redo(&mut document).unwrap();
        history.finish_coalescing();
        history
            .execute(
                &mut document,
                EditCommand::SetAxisRanges(ranges(5.0)),
                Some(EditGroup::AxisXMaximum),
            )
            .unwrap();
        history.undo(&mut document).unwrap();
        assert_eq!(document.axis_ranges().x_max, 4.5);
    }

    #[test]
    fn unsaved_replacement_stays_dirty_until_saved() {
        let document = FigureDocument::fixed();
        let mut history = EditHistory::new(&document, true);
        history.reset(&document, false);
        assert!(history.is_dirty(&document));
        history.mark_saved(&document);
        assert!(!history.is_dirty(&document));
    }

    #[test]
    fn external_change_rebases_history_without_losing_the_save_point() {
        let mut document = FigureDocument::fixed();
        let mut history = EditHistory::new(&document, true);
        history
            .execute(&mut document, EditCommand::SetAxisRanges(ranges(4.0)), None)
            .unwrap();
        history.rebase_after_external_change();
        assert!(history.undo(&mut document).is_none());
        assert!(history.is_dirty(&document));
    }

    #[test]
    fn series_lifecycle_commands_are_atomic_and_undoable() {
        let mut document = FigureDocument::fixed();
        let mut history = EditHistory::new(&document, true);
        history
            .execute(
                &mut document,
                EditCommand::CreateSeries {
                    data_source_id: "fixture-data".to_owned(),
                    x_column: "line_x".to_owned(),
                    y_column: "line_y".to_owned(),
                    style: SeriesCreationStyle::LineAndMarker,
                },
                None,
            )
            .unwrap();
        assert!(
            document
                .series()
                .iter()
                .any(|series| series.id == "series-1")
        );
        assert!(history.is_dirty(&document));

        history
            .execute(
                &mut document,
                EditCommand::SetSeriesVisible {
                    artist_id: "series-1".to_owned(),
                    visible: false,
                },
                None,
            )
            .unwrap();
        assert!(
            !document
                .series()
                .iter()
                .find(|series| series.id == "series-1")
                .unwrap()
                .visible
        );
        history.undo(&mut document).unwrap();
        assert!(
            document
                .series()
                .iter()
                .find(|series| series.id == "series-1")
                .unwrap()
                .visible
        );
        history.undo(&mut document).unwrap();
        assert!(
            !document
                .series()
                .iter()
                .any(|series| series.id == "series-1")
        );
        assert!(!history.is_dirty(&document));
    }

    #[test]
    fn p4_through_p7_visible_edits_share_undo_redo_and_round_trip() {
        let mut document = FigureDocument::fixed();
        let original = document.project().clone();
        let mut history = EditHistory::new(&document, true);

        let mut x_axis = document.axis_record(AxisDimension::X);
        x_axis.minimum = -4.0;
        x_axis.maximum = 4.0;
        x_axis.formatter = crate::FormatterSpec::Scientific { precision: 2 };
        history
            .execute(
                &mut document,
                EditCommand::SetAxisRecord {
                    dimension: AxisDimension::X,
                    record: x_axis,
                },
                None,
            )
            .unwrap();
        history
            .execute(
                &mut document,
                EditCommand::SetAxisLabel {
                    dimension: AxisDimension::X,
                    nodes: vec![
                        LabelNode::GreekVariable('μ'),
                        LabelNode::UnitSeparator,
                        LabelNode::Unit("m".to_owned()),
                    ],
                },
                None,
            )
            .unwrap();
        history
            .execute(
                &mut document,
                EditCommand::SetFigureSize {
                    width_mm: 89.0,
                    height_mm: 65.0,
                },
                None,
            )
            .unwrap();

        let mut line = document.artist_record("node-11").unwrap();
        let crate::ArtistProperties::Line { stroke, .. } = &mut line.properties else {
            panic!("node-11 must remain a line")
        };
        stroke.width_pt = 1.25;
        stroke.dash_pt = vec![4.0, 2.4];
        history
            .execute(&mut document, EditCommand::SetArtistRecord(line), None)
            .unwrap();
        history
            .execute(
                &mut document,
                EditCommand::SetSemanticLabel {
                    label_id: "label-temperature".to_owned(),
                    nodes: vec![
                        LabelNode::Variable("T".to_owned()),
                        LabelNode::Operator("=".to_owned()),
                        LabelNode::Number("250".to_owned()),
                        LabelNode::Unit("K".to_owned()),
                    ],
                },
                None,
            )
            .unwrap();
        let mut export = document.export_preferences().clone();
        export.selected_raster_dpi = 600;
        export.transparent_background = true;
        history
            .execute(
                &mut document,
                EditCommand::SetExportPreferences(export),
                None,
            )
            .unwrap();

        let edited = document.project().clone();
        assert_ne!(edited, original);
        assert!(history.is_dirty(&document));
        for _ in 0..6 {
            history.undo(&mut document).unwrap();
        }
        assert_eq!(document.project(), &original);
        assert!(!history.is_dirty(&document));
        for _ in 0..6 {
            history.redo(&mut document).unwrap();
        }
        assert_eq!(document.project(), &edited);

        let encoded = serde_json::to_vec(document.project()).unwrap();
        let decoded = crate::project::decode_project(&encoded).unwrap();
        assert_eq!(decoded, edited);
        assert_eq!(
            document.compile().unwrap(),
            FigureDocument::from_project(decoded)
                .unwrap()
                .compile()
                .unwrap()
        );
    }

    #[test]
    fn palette_selection_is_one_atomic_undoable_edit() {
        let mut document = FigureDocument::showcase();
        let original = document.project().clone();
        let mut history = EditHistory::new(&document, true);
        history
            .execute(
                &mut document,
                EditCommand::SetPalette {
                    palette_id: "tol-burd-v1".to_owned(),
                },
                None,
            )
            .unwrap();
        assert_eq!(document.palette_id(), "tol-burd-v1");
        assert!(history.is_dirty(&document));
        history.undo(&mut document).unwrap();
        assert_eq!(document.project(), &original);
        history.redo(&mut document).unwrap();
        assert_eq!(document.palette_id(), "tol-burd-v1");
    }
}
