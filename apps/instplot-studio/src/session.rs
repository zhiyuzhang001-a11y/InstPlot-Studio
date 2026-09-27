use std::path::{Path, PathBuf};

use instplot_core::{DataSet, DataSetKind, FitLink, NumericColumn};
use instplot_io::ImportError;
#[cfg(test)]
use instplot_io::read_data_file;

use crate::{
    DataImporter, DataSourceKind, DataSourceOrigin, DataSourcePayload, ImportOutcome,
    ProjectDocument,
};

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
        let (datasets, outcome) = DataImporter::import_file(&self.datasets, path)
            .map_err(crate::DataDiagnostic::into_import_error)?;
        self.datasets = datasets;
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
            source: if matches!(
                source.origin,
                DataSourceOrigin::Manual | DataSourceOrigin::LegacyManual
            ) {
                PathBuf::from(format!("manual-data/{}", source.label))
            } else {
                source.origin_path.as_ref().map_or_else(
                    || PathBuf::from(format!("embedded://{}", source.id)),
                    PathBuf::from,
                )
            },
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
                    values: column
                        .values
                        .iter()
                        .enumerate()
                        .map(|(index, value)| {
                            if column.valid.get(index).copied().unwrap_or(true) {
                                *value
                            } else {
                                f64::NAN
                            }
                        })
                        .collect(),
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
            let imported = DataImporter::read_file(path)
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

    #[test]
    fn numeric_csv_column_names_remain_headers_and_all_rows_are_preserved() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("numeric-column-headers.csv");
        let mut session = StudioSession::default();
        session.import_data_file(&path).unwrap();
        let dataset = &session.datasets()[0];
        assert_eq!(dataset.row_count, 3);
        assert_eq!(dataset.columns.len(), 4);
        assert_eq!(dataset.columns[0].name, "Diameter (nm)");
        assert_eq!(dataset.columns[1].name, "0.00083");
        assert_eq!(dataset.columns[0].values, vec![0.1, 0.2, 0.3]);
        assert_eq!(dataset.alive, vec![true; 3]);
        crate::FigureDocument::from_datasets(session.datasets()).unwrap();
    }

    #[test]
    fn missing_values_import_and_remain_selectively_available() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("p0-missing-values.csv");
        let mut session = StudioSession::default();
        session.import_data_file(&path).unwrap();
        let document = crate::FigureDocument::from_datasets(session.datasets()).unwrap();
        let source = &document.project().data_sources[0];
        let crate::DataSourcePayload::Embedded { columns, .. } = &source.payload else {
            panic!("imported data must be embedded");
        };
        assert_eq!(columns[1].valid, [true, true, false, true]);
        document.layout_figure().unwrap();
    }

    #[test]
    fn xlsx_with_blank_cells_imports_like_lite() {
        let source_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("p0-missing-values.csv");
        let datasets = read_data_file(&source_path).unwrap();
        let workbook = std::env::temp_dir().join(format!(
            "instplot-studio-missing-{}.xlsx",
            std::process::id()
        ));
        let references = datasets.iter().collect::<Vec<_>>();
        instplot_io::save_workbook_refs_with_fits(&workbook, &references, &[]).unwrap();

        let mut session = StudioSession::default();
        let outcome = session.import_data_file(&workbook).unwrap();
        std::fs::remove_file(workbook).unwrap();
        assert_eq!(outcome.added, 1);
        assert!(session.datasets()[0].columns[1].values[2].is_nan());
        crate::FigureDocument::from_datasets(session.datasets())
            .unwrap()
            .layout_figure()
            .unwrap();
    }

    #[test]
    fn all_lite_text_extensions_follow_the_same_import_path() {
        for extension in ["csv", "tsv", "txt", "dat"] {
            let path = std::env::temp_dir().join(format!(
                "instplot-studio-import-{}-{extension}.{extension}",
                std::process::id()
            ));
            let separator = if extension == "csv" { "," } else { "\t" };
            std::fs::write(
                &path,
                format!("field{separator}signal\n0{separator}1\n1{separator}2\n"),
            )
            .unwrap();
            let mut session = StudioSession::default();
            let outcome = session.import_data_file(&path).unwrap();
            std::fs::remove_file(path).unwrap();
            assert_eq!(outcome.added, 1, "extension {extension}");
            crate::FigureDocument::from_datasets(session.datasets()).unwrap();
        }
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
                replaced: 0,
                skipped_unlinked_fits: 0,
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
                replaced: 1,
                skipped_unlinked_fits: 0,
            }
        );
        assert_eq!(session.dataset_count(), 1);
        assert_eq!(session.datasets()[0].plot_id, plot_id);
    }

    #[test]
    fn conflicting_identity_from_another_file_is_rejected_without_partial_changes() {
        let mut session = StudioSession::default();
        session
            .import_data_file(&lite_source_fit_fixture())
            .unwrap();
        let before_ids = session
            .datasets()
            .iter()
            .map(|dataset| dataset.plot_id.clone())
            .collect::<Vec<_>>();
        let copy = std::env::temp_dir().join(format!(
            "instplot-duplicate-identity-{}.txt",
            std::process::id()
        ));
        std::fs::copy(lite_source_fit_fixture(), &copy).unwrap();
        let error = session.import_data_file(&copy).unwrap_err();
        std::fs::remove_file(copy).unwrap();
        assert_eq!(error.code, "duplicate_dataset_id");
        assert_eq!(
            session
                .datasets()
                .iter()
                .map(|dataset| dataset.plot_id.clone())
                .collect::<Vec<_>>(),
            before_ids
        );
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
    fn lite_combined_csv_with_multiple_sources_imports_and_plots_every_section() {
        let mut source_session = StudioSession::default();
        source_session.import_data_file(&fixture()).unwrap();
        let first = source_session.datasets()[0].clone();
        let mut second = first.clone();
        second.plot_id = "second-source".to_owned();
        second.label = Some("Second source".to_owned());
        second.columns[1].values = vec![8.0, 6.0, 4.0];
        let path = std::env::temp_dir().join(format!(
            "instplot-lite-combined-import-{}.csv",
            std::process::id()
        ));
        instplot_io::save_text_combined(
            &path,
            &[&first, &second],
            instplot_io::TextExportFormat::Csv,
            &[],
        )
        .unwrap();

        let mut imported = StudioSession::default();
        let outcome = imported.import_data_file(&path).unwrap();
        assert_eq!(outcome.read, 2);
        assert_eq!(outcome.added, 2);
        assert_eq!(imported.dataset_count(), 2);
        let document = crate::FigureDocument::from_datasets(imported.datasets()).unwrap();
        assert_eq!(document.project().data_sources.len(), 2);
        let plotted = document
            .series()
            .into_iter()
            .filter(|series| series.binding.is_some())
            .count();
        assert_eq!(plotted, 2);
        document.layout_figure().unwrap();
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn aggregate_fit_without_parent_does_not_block_combined_source_import() {
        let mut source_session = StudioSession::default();
        source_session.import_data_file(&fixture()).unwrap();
        source_session
            .import_data_file(&lite_source_fit_fixture())
            .unwrap();
        let first = source_session.datasets()[0].clone();
        let second = source_session.datasets()[1].clone();
        let mut aggregate_fit = source_session.datasets()[2].clone();
        aggregate_fit.fit_link.as_mut().unwrap().parent_dataset_id = None;
        let path = std::env::temp_dir().join(format!(
            "instplot-lite-aggregate-fit-{}.csv",
            std::process::id()
        ));
        instplot_io::save_text_combined(
            &path,
            &[&first, &second, &aggregate_fit],
            instplot_io::TextExportFormat::Csv,
            &[],
        )
        .unwrap();
        let mut imported = StudioSession::default();
        let outcome = imported.import_data_file(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(outcome.read, 2);
        assert_eq!(outcome.skipped_unlinked_fits, 1);
        assert_eq!(imported.dataset_count(), 2);
        assert!(
            imported
                .datasets()
                .iter()
                .all(|dataset| dataset.kind == DataSetKind::Source)
        );
        crate::FigureDocument::from_datasets(imported.datasets()).unwrap();
    }

    #[test]
    fn reimport_rejects_removed_or_unlinked_sections_instead_of_leaving_stale_fit() {
        let mut source_session = StudioSession::default();
        source_session
            .import_data_file(&lite_source_fit_fixture())
            .unwrap();
        let source = source_session.datasets()[0].clone();
        let linked_fit = source_session.datasets()[1].clone();
        let path = std::env::temp_dir().join(format!(
            "instplot-reimport-stale-fit-{}.csv",
            std::process::id()
        ));
        instplot_io::save_text_combined(
            &path,
            &[&source, &linked_fit],
            instplot_io::TextExportFormat::Csv,
            &[],
        )
        .unwrap();
        let mut imported = StudioSession::default();
        imported.import_data_file(&path).unwrap();

        let mut aggregate_fit = linked_fit.clone();
        aggregate_fit.fit_link.as_mut().unwrap().parent_dataset_id = None;
        instplot_io::save_text_combined(
            &path,
            &[&source, &aggregate_fit],
            instplot_io::TextExportFormat::Csv,
            &[],
        )
        .unwrap();
        let error = imported.import_data_file(&path).unwrap_err();
        assert_eq!(error.code, "changed_dataset_sections");
        assert_eq!(imported.dataset_count(), 2);
        assert_eq!(
            imported.datasets()[1]
                .fit_link
                .as_ref()
                .unwrap()
                .parent_dataset_id
                .as_deref(),
            Some("source-a")
        );

        instplot_io::save_text_combined(&path, &[&source], instplot_io::TextExportFormat::Csv, &[])
            .unwrap();
        let error = imported.import_data_file(&path).unwrap_err();
        std::fs::remove_file(path).unwrap();
        assert_eq!(error.code, "changed_dataset_sections");
        assert_eq!(imported.dataset_count(), 2);
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
