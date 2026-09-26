use std::path::Path;

use instplot_studio::{
    DATA_FORMAT_CAPABILITIES, DataImporter, DataSourceKind, ErrorStatistic, ManualAxisInput,
    ManualDataInput, ManualYInput, ProjectDocument, StudioSession,
};

fn axis(name: &str, measurements: &[&str]) -> ManualAxisInput {
    ManualAxisInput {
        name: name.to_owned(),
        measurements: measurements
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
    }
}

#[test]
fn data_pipeline_imports_files_and_pasted_measurements_without_starting_the_ui() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/smoke.csv");
    let (datasets, outcome) = DataImporter::import_file(&[], &fixture).unwrap();
    assert_eq!(outcome.added, 1);
    assert_eq!(datasets.len(), 1);
    assert_eq!(datasets[0].columns.len(), 2);

    let manual = DataImporter::import_manual(&ManualDataInput {
        source_name: "Repeated measurement".to_owned(),
        x_inputs: vec![axis("Field", &["0 1 2"])],
        y_inputs: vec![ManualYInput {
            axis: axis("Signal", &["2 4 6", "4 6 8"]),
            x_index: 0,
        }],
        error_statistic: ErrorStatistic::StandardDeviation,
    })
    .unwrap();
    assert_eq!(manual.series.len(), 1);
    assert_eq!(
        manual.series[0].y_error_column.as_deref(),
        Some("Signal · SD")
    );
    assert!(
        manual
            .dataset
            .columns
            .iter()
            .any(|column| column.name == "Signal · 测量 1")
    );
    assert!(
        DATA_FORMAT_CAPABILITIES
            .iter()
            .flat_map(|format| format.extensions)
            .any(|extension| *extension == "xlsx")
    );
}

#[test]
fn data_pipeline_reports_structured_diagnostics_for_files_and_manual_input() {
    let file_error = DataImporter::import_file(
        &[],
        Path::new("/definitely-missing/instplot-data-pipeline.csv"),
    )
    .unwrap_err();
    assert!(!file_error.code.is_empty());
    assert!(!file_error.reason.is_empty());

    let manual_error = DataImporter::import_manual(&ManualDataInput {
        source_name: "empty".to_owned(),
        x_inputs: vec![axis("X", &[""])],
        y_inputs: vec![ManualYInput {
            axis: axis("Y", &["1 2"]),
            x_index: 0,
        }],
        error_statistic: ErrorStatistic::StandardDeviation,
    })
    .unwrap_err();
    assert_eq!(manual_error.code, "manual-input");
    assert!(manual_error.reason.contains("至少一个数值"));
}

#[test]
fn legacy_external_source_failure_is_explicit_and_does_not_substitute_data() {
    use std::time::{SystemTime, UNIX_EPOCH};

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "instplot-phase3-external-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let source = directory.join("legacy.csv");
    std::fs::write(&source, b"x,y\n1,2\n").unwrap();
    let mut project = ProjectDocument::fixed_fixture();
    project
        .upsert_external_source(
            "source-1",
            "legacy.csv",
            &source,
            DataSourceKind::Source,
            None,
        )
        .unwrap();
    std::fs::remove_file(&source).unwrap();

    let (session, warnings) = StudioSession::from_project(&project);
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("legacy.csv"))
    );
    assert!(
        session
            .datasets()
            .iter()
            .all(|dataset| dataset.plot_id != "source-1")
    );
    std::fs::remove_dir(directory).unwrap();
}
