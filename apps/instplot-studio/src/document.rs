use core::fmt;

use studio_render_spike::{
    Artist, CompileError, DisplayList, Figure, NodeId, compile, fixed_figure,
};

/// B1's in-memory editing model. Persistence, schema versions, migrations and
/// provenance are deliberately deferred to the formal B2 Figure Document.
#[derive(Clone, Debug, PartialEq)]
pub struct FigureDocument {
    figure: Figure,
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
    pub id: NodeId,
    pub kind: SeriesKind,
    pub label: String,
}

impl FigureDocument {
    pub fn fixed() -> Self {
        Self {
            figure: fixed_figure(),
        }
    }

    pub fn compile(&self) -> Result<DisplayList, CompileError> {
        compile(&self.figure)
    }

    pub fn axis_ranges(&self) -> AxisRanges {
        let axes = self
            .figure
            .axes
            .first()
            .expect("the B1 fixed document has one axes");
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
        let axes = self
            .figure
            .axes
            .first_mut()
            .expect("the B1 fixed document has one axes");
        axes.x.minimum = ranges.x_min;
        axes.x.maximum = ranges.x_max;
        axes.y.minimum = ranges.y_min;
        axes.y.maximum = ranges.y_max;
        Ok(())
    }

    pub fn series(&self) -> Vec<SeriesDescriptor> {
        self.figure
            .axes
            .iter()
            .flat_map(|axes| axes.artists.iter())
            .map(|artist| {
                let (id, kind) = match artist {
                    Artist::Line(value) => (value.id, SeriesKind::Line),
                    Artist::Scatter(value) => (value.id, SeriesKind::Scatter),
                    Artist::ErrorBar(value) => (value.id, SeriesKind::ErrorBar),
                    Artist::ReferenceLine(value) => (value.id, SeriesKind::ReferenceLine),
                    Artist::Text(value) => (value.id, SeriesKind::Annotation),
                    Artist::Legend(value) => (value.id, SeriesKind::Legend),
                };
                SeriesDescriptor {
                    id,
                    kind,
                    label: format!("{kind} · node {}", id.0),
                }
            })
            .collect()
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
}
