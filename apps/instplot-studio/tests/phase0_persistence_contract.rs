use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use instplot_studio::{
    DataSourcePayload, FigureDocument, OpenProjectSource, StudioSession, resolve_document,
};

struct TempDirectory(PathBuf);

impl TempDirectory {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must be after the Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "instplot-phase0-persistence-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create isolated persistence test directory");
        Self(path)
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn ordinary_imported_project_reopens_after_the_source_file_is_deleted() {
    let directory = TempDirectory::new();
    let imported_path = directory.0.join("measurement.csv");
    fs::copy(fixture("smoke.csv"), &imported_path).expect("copy import fixture");

    let mut session = StudioSession::default();
    let outcome = session
        .import_data_file(&imported_path)
        .expect("ordinary CSV import must succeed");
    assert_eq!(outcome.added, 1);
    let expected_columns = session.datasets()[0]
        .columns
        .iter()
        .map(|column| (column.name.clone(), column.values.clone()))
        .collect::<Vec<_>>();
    let expected_alive = session.datasets()[0].alive.clone();

    let document = FigureDocument::from_datasets(session.datasets())
        .expect("imported datasets must create a valid figure");
    assert!(
        document
            .project()
            .data_sources
            .iter()
            .all(|source| { matches!(source.payload, DataSourcePayload::Embedded { .. }) })
    );
    let project_path = directory.0.join("offline.instplot");
    document.save(&project_path).expect("save embedded project");

    fs::remove_file(&imported_path).expect("remove the original import source");
    let (reopened, report) =
        FigureDocument::open(&project_path).expect("project must reopen without source CSV");
    assert_eq!(report.source, OpenProjectSource::Primary);
    let (restored_session, warnings) = StudioSession::from_project(reopened.project());
    assert!(
        warnings.is_empty(),
        "embedded data must not need its source: {warnings:?}"
    );
    assert_eq!(restored_session.dataset_count(), 1);
    let restored_columns = restored_session.datasets()[0]
        .columns
        .iter()
        .map(|column| (column.name.clone(), column.values.clone()))
        .collect::<Vec<_>>();
    assert_eq!(restored_columns, expected_columns);
    assert_eq!(restored_session.datasets()[0].alive, expected_alive);

    let resolved = resolve_document(&reopened).expect("restored project must remain renderable");
    assert!(!resolved.display.items.is_empty());
}
