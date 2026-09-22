use std::path::{Path, PathBuf};

use instplot_core::{DataSet, DataSetKind, FitLink, NumericColumn};
use instplot_io::{ImportError, read_data_file};

use crate::{DataSourceKind, DataSourcePayload, ProjectDocument};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImportOutcome {
    pub read: usize,
    pub added: usize,
    pub replaced: usize,
}

#[derive(Clone, Default)]
pub struct StudioSession {
    datasets: Vec<DataSet>,
}

impl StudioSession {
    pub fn dataset_count(&self) -> usize {
        self.datasets.len()
    }

    pub fn datasets(&self) -> &[DataSet] {
        &self.datasets
    }

    pub fn replace_datasets(&mut self, datasets: Vec<DataSet>) {
        self.datasets = datasets;
    }

    /// Rebuild the non-authoritative UI session from a saved project.
    ///
    /// Embedded sources are restored exactly. External sources are restored only when their
    /// recorded fingerprint still matches; missing or changed files remain represented by the
    /// Figure Document and are reported as warnings instead of leaking data from the prior
    /// workspace into the newly opened one.
    pub fn from_project(project: &ProjectDocument) -> (Self, Vec<String>) {
        let mut datasets = Vec::new();
        let mut warnings = Vec::new();
        for source in &project.data_sources {
            match dataset_from_record(source) {
                Ok(dataset) => datasets.push(dataset),
                Err(warning) => warnings.push(warning),
            }
        }
        (Self { datasets }, warnings)
    }

    pub fn import_data_file(&mut self, path: &Path) -> Result<ImportOutcome, ImportError> {
        let imported = read_data_file(path)?;
        let mut outcome = ImportOutcome {
            read: imported.len(),
            ..ImportOutcome::default()
        };
        for dataset in imported {
            if let Some(existing) = self
                .datasets
                .iter_mut()
                .find(|existing| existing.plot_id == dataset.plot_id)
            {
                *existing = dataset;
                outcome.replaced += 1;
            } else {
                self.datasets.push(dataset);
                outcome.added += 1;
            }
        }
        Ok(outcome)
    }
}

fn dataset_from_record(source: &crate::DataSourceRecord) -> Result<DataSet, String> {
    let kind = match source.kind {
        DataSourceKind::Source => DataSetKind::Source,
        DataSourceKind::Fit => DataSetKind::Fit,
    };
    let fit_link = source.fit.as_ref().map(|fit| FitLink {
        parent_dataset_id: Some(fit.parent_data_source_id.clone()),
        source_x_column: fit.source_x_column.clone(),
        source_y_column: fit.source_y_column.clone(),
        equation: fit.equation.clone(),
        display_equation: fit.display_equation.clone(),
    });
    match &source.payload {
        DataSourcePayload::Embedded {
            columns,
            row_count,
            alive,
            ..
        } => Ok(DataSet {
            source: PathBuf::from(format!("embedded://{}", source.id)),
            label: Some(source.label.clone()),
            kind,
            plot_id: source.id.clone(),
            fit_link,
            encoding: "embedded".to_owned(),
            separator: String::new(),
            columns: columns
                .iter()
                .map(|column| NumericColumn {
                    name: column.name.clone(),
                    values: column.values.clone(),
                })
                .collect(),
            row_count: *row_count,
            alive: if alive.is_empty() {
                vec![true; *row_count]
            } else {
                alive.clone()
            },
        }),
        DataSourcePayload::External { path, fingerprint } => {
            let path = Path::new(path);
            let actual = crate::project::fingerprint(path)
                .map_err(|error| format!("无法恢复外部数据源 {}：{error}", source.label))?;
            if &actual != fingerprint {
                return Err(format!(
                    "外部数据源 {} 已变化，未载入到当前工作区",
                    source.label
                ));
            }
            let imported = read_data_file(path)
                .map_err(|error| format!("无法恢复外部数据源 {}：{error}", source.label))?;
            let mut dataset = imported
                .into_iter()
                .find(|dataset| dataset.plot_id == source.id)
                .ok_or_else(|| format!("外部数据源 {} 中找不到保存的稳定标识", source.label))?;
            dataset.label = Some(source.label.clone());
            dataset.kind = kind;
            dataset.fit_link = fit_link;
            Ok(dataset)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("smoke.csv")
    }

    fn lite_source_fit_fixture() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("lite-source-fit.txt")
    }

    #[test]
    fn imports_shared_io_data_without_copying_the_parser() {
        let mut session = StudioSession::default();
        let outcome = session.import_data_file(&fixture()).unwrap();
        assert_eq!(
            outcome,
            ImportOutcome {
                read: 1,
                added: 1,
                replaced: 0
            }
        );
        assert_eq!(session.dataset_count(), 1);
        assert_eq!(session.datasets()[0].columns[0].name, "field");
        assert_eq!(session.datasets()[0].columns[1].values, [2.0, 4.0, 8.0]);
    }

    #[test]
    fn reimport_replaces_the_same_stable_dataset_identity() {
        let mut session = StudioSession::default();
        session.import_data_file(&fixture()).unwrap();
        let plot_id = session.datasets()[0].plot_id.clone();
        let outcome = session.import_data_file(&fixture()).unwrap();
        assert_eq!(
            outcome,
            ImportOutcome {
                read: 1,
                added: 0,
                replaced: 1
            }
        );
        assert_eq!(session.dataset_count(), 1);
        assert_eq!(session.datasets()[0].plot_id, plot_id);
    }

    #[test]
    fn lite_partitioned_export_preserves_explicit_fit_identity_and_equations() {
        let mut session = StudioSession::default();
        let outcome = session
            .import_data_file(&lite_source_fit_fixture())
            .unwrap();
        assert_eq!(outcome.read, 2);
        let source = &session.datasets()[0];
        let fit = &session.datasets()[1];
        assert_eq!(source.plot_id, "source-a");
        assert_eq!(fit.plot_id, "fit-a");
        let link = fit.fit_link.as_ref().unwrap();
        assert_eq!(link.parent_dataset_id.as_deref(), Some("source-a"));
        assert_eq!(link.source_x_column, "field");
        assert_eq!(link.source_y_column, "response");
        assert_eq!(link.equation.as_deref(), Some("y=1+x"));
        assert_eq!(link.display_equation.as_deref(), Some("y = 1 + x"));

        let source_path = lite_source_fit_fixture();
        let source_before = std::fs::read(&source_path).unwrap();
        let mut document = crate::FigureDocument::from_datasets(session.datasets()).unwrap();
        document
            .set_axis_ranges(crate::AxisRanges {
                x_min: -1.0,
                x_max: 3.0,
                y_min: 0.0,
                y_max: 4.0,
            })
            .unwrap();
        let output = std::env::temp_dir().join(format!(
            "instplot-no-writeback-{}.instplot",
            std::process::id()
        ));
        document.save(&output).unwrap();
        assert_eq!(std::fs::read(&source_path).unwrap(), source_before);
        std::fs::remove_file(output).unwrap();
    }

    #[test]
    fn lite_xlsx_round_trip_preserves_the_same_fit_link() {
        let mut source_session = StudioSession::default();
        source_session
            .import_data_file(&lite_source_fit_fixture())
            .unwrap();
        let workbook =
            std::env::temp_dir().join(format!("instplot-lite-handoff-{}.xlsx", std::process::id()));
        let references = source_session.datasets().iter().collect::<Vec<_>>();
        instplot_io::save_workbook_refs_with_fits(&workbook, &references, &[]).unwrap();

        let mut imported = StudioSession::default();
        let outcome = imported.import_data_file(&workbook).unwrap();
        assert_eq!(outcome.read, 2);
        let fit = imported
            .datasets()
            .iter()
            .find(|dataset| dataset.kind == instplot_core::DataSetKind::Fit)
            .unwrap();
        let link = fit.fit_link.as_ref().unwrap();
        assert_eq!(link.parent_dataset_id.as_deref(), Some("source-a"));
        assert_eq!(link.source_x_column, "field");
        assert_eq!(link.source_y_column, "response");
        assert_eq!(link.display_equation.as_deref(), Some("y = 1 + x"));
        std::fs::remove_file(workbook).unwrap();
    }

    #[test]
    fn opening_a_project_replaces_session_data_with_embedded_sources() {
        let document = crate::FigureDocument::fixed();
        let (session, warnings) = StudioSession::from_project(document.project());
        assert!(warnings.is_empty());
        assert_eq!(
            session.dataset_count(),
            document.project().data_sources.len()
        );
        assert_eq!(
            session.datasets()[0].plot_id,
            document.project().data_sources[0].id
        );
        assert_eq!(
            session.datasets()[0].label.as_deref(),
            Some(document.project().data_sources[0].label.as_str())
        );
    }
}
