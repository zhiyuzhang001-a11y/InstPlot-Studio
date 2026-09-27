use std::path::PathBuf;

use instplot_core::{DataSet, DataSetKind, NumericColumn};
use instplot_studio::{
    AutoscalePolicy, AxisDimension, DataBounds, FigureDocument, LabelNode, compute_data_bounds,
};

fn error_dataset() -> DataSet {
    DataSet {
        source: PathBuf::from("domain/error.csv"),
        label: Some("error.csv".to_owned()),
        kind: DataSetKind::Source,
        plot_id: "domain-error".to_owned(),
        fit_link: None,
        encoding: "UTF-8".to_owned(),
        separator: ",".to_owned(),
        columns: vec![
            NumericColumn {
                name: "x".to_owned(),
                values: vec![0.0, 1.0],
            },
            NumericColumn {
                name: "y".to_owned(),
                values: vec![10.0, 20.0],
            },
            NumericColumn {
                name: "dy".to_owned(),
                values: vec![2.0, 3.0],
            },
        ],
        row_count: 2,
        alive: vec![true, true],
    }
}

#[test]
fn autoscale_data_bounds_separate_error_policy_annotations_and_visual_marker_padding() {
    let dataset = error_dataset();
    let mut document = FigureDocument::from_datasets(std::slice::from_ref(&dataset)).unwrap();
    document
        .create_error_bars(&dataset.plot_id, "x", "y", "dy", None)
        .unwrap();
    document
        .add_annotation(vec![LabelNode::Text("far-away note".to_owned())])
        .unwrap();

    let without_errors = compute_data_bounds(
        document.project(),
        AxisDimension::Y,
        AutoscalePolicy {
            include_error_bars: false,
            marker_padding_pt: 100.0,
            ..AutoscalePolicy::default()
        },
    )
    .unwrap();
    assert_eq!(
        without_errors,
        DataBounds {
            minimum: 10.0,
            maximum: 20.0
        }
    );

    let with_errors = compute_data_bounds(
        document.project(),
        AxisDimension::Y,
        AutoscalePolicy::default(),
    )
    .unwrap();
    assert_eq!(
        with_errors,
        DataBounds {
            minimum: 8.0,
            maximum: 23.0
        }
    );
}
