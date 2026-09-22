use core::fmt;
use std::path::Path;

use instplot_core::{DataSet, DataSetKind};
use studio_render_spike::{CompileError, DisplayList, compile, fixed_figure};

use crate::{
    DataSourceKind, FitIdentity, OpenProjectReport, ProjectDocument, ProjectError, open_project,
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
