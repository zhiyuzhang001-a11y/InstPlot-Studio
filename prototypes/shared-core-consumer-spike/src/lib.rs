//! Verifies that Studio-side code can consume the shared data model without a
//! copied implementation or a machine-local path dependency.

use instplot_core::{DataSet, DataSetKind, NumericColumn, generated_plot_id};
use std::path::PathBuf;

pub fn sample_dataset() -> DataSet {
    let source = PathBuf::from("sample.csv");
    let columns = vec![
        NumericColumn {
            name: "x".to_owned(),
            values: vec![1.0, 2.0],
        },
        NumericColumn {
            name: "y".to_owned(),
            values: vec![3.0, 4.0],
        },
    ];
    let plot_id = generated_plot_id(&source, &columns);
    DataSet {
        source,
        label: Some("Studio consumer".to_owned()),
        kind: DataSetKind::Source,
        plot_id,
        fit_link: None,
        encoding: "UTF-8".to_owned(),
        separator: ",".to_owned(),
        row_count: 2,
        alive: vec![true; 2],
        columns,
    }
}

#[cfg(test)]
mod tests {
    use super::sample_dataset;

    #[test]
    fn studio_side_consumer_uses_the_shared_identity_contract() {
        let dataset = sample_dataset();
        assert_eq!(dataset.plot_id, "instplot-c2bb893aa33ffbd5");
        assert_eq!(
            dataset.plot_points(0, 1, 10, None),
            [[1.0, 3.0], [2.0, 4.0]]
        );
    }
}
