use crate::{AxisRanges, FigureDocument, ProjectDocument};

const MAX_HISTORY: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditGroup {
    AxisXMinimum,
    AxisXMaximum,
    AxisYMinimum,
    AxisYMaximum,
}

pub enum EditCommand {
    SetAxisRanges(AxisRanges),
}

impl EditCommand {
    fn description(&self) -> &'static str {
        match self {
            Self::SetAxisRanges(_) => "Change axes ranges",
        }
    }

    fn apply(self, document: &mut FigureDocument) -> Result<(), String> {
        match self {
            Self::SetAxisRanges(ranges) => {
                document.set_axis_ranges(ranges).map_err(ToOwned::to_owned)
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditOutcome {
    pub changed: bool,
    pub description: String,
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
        candidate
            .layout_figure()
            .map_err(|error| format!("edited document cannot be laid out: {error}"))?;
        if candidate.project() == document.project() {
            return Ok(EditOutcome {
                changed: false,
                description,
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
}
