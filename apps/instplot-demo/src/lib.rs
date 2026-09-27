use std::error::Error;
use std::fmt;
use std::path::Path;

use instplot_studio::{
    AxisDimension, DataImporter, FigureDocument, LabelNode, SeriesKind, save_figure_svg,
};
use instplot_ui::{AppServices, Branding, FeatureSet, ShellEvent};

#[derive(Debug)]
pub struct DemoError(String);

impl fmt::Display for DemoError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for DemoError {}

pub struct QuickPlotApp {
    pub branding: Branding,
    pub features: FeatureSet,
    pub last_event: Option<ShellEvent>,
}

impl Default for QuickPlotApp {
    fn default() -> Self {
        Self {
            branding: Branding::new("InstPlot Quick", env!("CARGO_PKG_VERSION")),
            features: FeatureSet {
                data_import: true,
                manual_data: false,
                project_files: false,
                annotations: false,
                publication_check: false,
                export_pdf: false,
                export_png: false,
                export_svg: true,
            },
            last_event: None,
        }
    }
}

impl AppServices for QuickPlotApp {
    type Error = DemoError;

    fn dispatch(&mut self, event: ShellEvent) -> Result<(), Self::Error> {
        let enabled = match event {
            ShellEvent::ImportData => self.features.data_import,
            ShellEvent::ExportSvg => self.features.export_svg,
            ShellEvent::EnterData => self.features.manual_data,
            ShellEvent::OpenProject | ShellEvent::SaveProject | ShellEvent::NewProject => {
                self.features.project_files
            }
            ShellEvent::InsertAnnotation => self.features.annotations,
            ShellEvent::TogglePublicationCheck => self.features.publication_check,
            ShellEvent::ExportPdf => self.features.export_pdf,
            ShellEvent::ExportPng => self.features.export_png,
        };
        if !enabled {
            return Err(DemoError(format!("feature is disabled: {event:?}")));
        }
        self.last_event = Some(event);
        Ok(())
    }
}

impl QuickPlotApp {
    pub fn import_select_label_export(
        &mut self,
        input: &Path,
        x_column: &str,
        y_column: &str,
        x_label: &str,
        y_label: &str,
        output: &Path,
    ) -> Result<usize, DemoError> {
        self.dispatch(ShellEvent::ImportData)?;
        let datasets =
            DataImporter::read_file(input).map_err(|error| DemoError(error.to_string()))?;
        let first = datasets
            .first()
            .ok_or_else(|| DemoError("the imported file contains no datasets".to_owned()))?;
        for column in [x_column, y_column] {
            if !first
                .columns
                .iter()
                .any(|candidate| candidate.name == column)
            {
                return Err(DemoError(format!("column is missing: {column}")));
            }
        }
        let mut document = FigureDocument::from_datasets(&datasets)
            .map_err(|error| DemoError(error.to_string()))?;
        let series = document
            .series()
            .into_iter()
            .find(|series| {
                matches!(series.kind, SeriesKind::Line | SeriesKind::Scatter)
                    && series
                        .binding
                        .as_ref()
                        .is_some_and(|binding| binding.data_source_id == first.plot_id)
            })
            .ok_or_else(|| DemoError("no editable series was created".to_owned()))?;
        document
            .rebind_series(&series.id, &first.plot_id, x_column, y_column, None)
            .map_err(DemoError)?;
        document
            .set_axis_label(AxisDimension::X, vec![LabelNode::Text(x_label.to_owned())])
            .map_err(DemoError)?;
        document
            .set_axis_label(AxisDimension::Y, vec![LabelNode::Text(y_label.to_owned())])
            .map_err(DemoError)?;
        document.refresh_autoscale().map_err(DemoError)?;
        self.dispatch(ShellEvent::ExportSvg)?;
        save_figure_svg(&document, output).map_err(|error| DemoError(error.to_string()))
    }
}
