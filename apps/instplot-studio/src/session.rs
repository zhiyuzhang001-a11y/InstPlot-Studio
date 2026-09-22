use std::path::Path;

use instplot_core::DataSet;
use instplot_io::{ImportError, read_data_file};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImportOutcome {
    pub read: usize,
    pub added: usize,
    pub replaced: usize,
}

#[derive(Default)]
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
}
