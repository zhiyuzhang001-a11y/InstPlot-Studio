use super::*;

fn gap_document(x: Vec<f64>, y: Vec<f64>, alive: Vec<bool>) -> FigureDocument {
    FigureDocument::from_datasets(&[DataSet {
        source: PathBuf::from("autoscale-gaps.csv"),
        label: None,
        kind: DataSetKind::Source,
        plot_id: "gaps".to_owned(),
        fit_link: None,
        encoding: "UTF-8".to_owned(),
        separator: "comma".to_owned(),
        row_count: x.len(),
        columns: vec![
            NumericColumn {
                name: "x".to_owned(),
                values: x,
            },
            NumericColumn {
                name: "y".to_owned(),
                values: y,
            },
        ],
        alive,
    }])
    .unwrap()
}

fn bounds(document: &FigureDocument, identity: AxisIdentity) -> DataBounds {
    compute_axis_data_bounds(document.project(), identity, AutoscalePolicy::default()).unwrap()
}

#[test]
fn autoscale_missing_tail_does_not_pull_a_narrow_signal_to_zero() {
    let mut document = gap_document(
        vec![0.0, 1.0, 2.0, 3.0, 4.0],
        vec![244.45285, 244.81309, f64::NAN, f64::NAN, f64::NAN],
        vec![true; 5],
    );
    // Include both line and scatter: all must agree with the renderer's pairs.
    document
        .create_series("gaps", "x", "y", SeriesCreationStyle::LineAndMarker)
        .unwrap();
    document.refresh_autoscale().unwrap();
    assert_eq!(
        bounds(&document, AxisIdentity::X1),
        DataBounds {
            minimum: 0.0,
            maximum: 1.0
        }
    );
    assert_eq!(
        bounds(&document, AxisIdentity::Y1),
        DataBounds {
            minimum: 244.45285,
            maximum: 244.81309
        }
    );
    let y = document.axis_record(AxisDimension::Y);
    assert!((y.minimum - 244.434838).abs() < 1e-9, "{y:?}");
    assert!((y.maximum - 244.831102).abs() < 1e-9, "{y:?}");
    let layout = document.layout_figure().unwrap();
    let major = &layout.result.y_axis.major;
    assert!(major.len() >= 2 && major.len() < 32);
    assert!(
        major
            .windows(2)
            .all(|pair| pair[0].value < pair[1].value && pair[0].label != pair[1].label)
    );
    assert!(layout.result.display_list.validation_errors().is_empty());
}

#[test]
fn autoscale_filters_invalid_counterparts_and_deleted_rows_on_both_axes() {
    let document = gap_document(
        vec![10.0, 20.0, 1e8, f64::NAN, -1e9],
        vec![100.0, 200.0, f64::NAN, -1e8, -1e9],
        vec![true, true, true, true, false],
    );
    assert_eq!(
        bounds(&document, AxisIdentity::X1),
        DataBounds {
            minimum: 10.0,
            maximum: 20.0
        }
    );
    assert_eq!(
        bounds(&document, AxisIdentity::Y1),
        DataBounds {
            minimum: 100.0,
            maximum: 200.0
        }
    );
}

#[test]
fn autoscale_keeps_real_zero_and_log_rejects_it_atomically() {
    let mut document = gap_document(
        vec![1.0, 2.0, 3.0],
        vec![0.0, 10.0, f64::NAN],
        vec![true; 3],
    );
    assert_eq!(bounds(&document, AxisIdentity::Y1).minimum, 0.0);
    let before = document.project().clone();
    let mut y = document.axis_record(AxisDimension::Y);
    y.autoscale = true;
    y.scale = AxisScale::Log10;
    assert!(document.set_axis_record(AxisDimension::Y, y).is_err());
    assert_eq!(document.project(), &before);
}

#[test]
fn log_autoscale_accepts_positive_pairs_despite_missing_cells() {
    let mut document = gap_document(
        vec![1.0, 2.0, 3.0, f64::NAN],
        vec![10.0, 100.0, f64::NAN, -10.0],
        vec![true; 4],
    );
    let mut y = document.axis_record(AxisDimension::Y);
    y.autoscale = true;
    y.scale = AxisScale::Log10;
    document.set_axis_record(AxisDimension::Y, y).unwrap();
    let y = document.axis_record(AxisDimension::Y);
    assert!(y.minimum > 0.0 && y.minimum < 10.0 && y.maximum > 100.0);
    document.layout_figure().unwrap();
}

#[test]
fn secondary_axes_use_the_same_valid_pair_filter() {
    for (mode, binding, identity, expected) in [
        (
            AxisMode::DualY,
            AxisBinding {
                x: XAxisSlot::X1,
                y: YAxisSlot::Y2,
            },
            AxisIdentity::Y2,
            DataBounds {
                minimum: 10.0,
                maximum: 20.0,
            },
        ),
        (
            AxisMode::DualX,
            AxisBinding {
                x: XAxisSlot::X2,
                y: YAxisSlot::Y1,
            },
            AxisIdentity::X2,
            DataBounds {
                minimum: 1.0,
                maximum: 2.0,
            },
        ),
    ] {
        let mut document = gap_document(
            vec![1.0, 2.0, 1e8, f64::NAN],
            vec![10.0, 20.0, f64::NAN, 1e8],
            vec![true; 4],
        );
        document.set_axis_mode(mode).unwrap();
        document
            .set_series_axis_binding("handoff-artist-gaps", binding)
            .unwrap();
        assert_eq!(bounds(&document, identity), expected);
        document.layout_figure().unwrap();
    }
}

#[test]
fn manual_axis_and_project_validity_survive_refresh_and_round_trip() {
    let mut document = gap_document(
        vec![1.0, 2.0, 3.0],
        vec![10.0, 20.0, f64::NAN],
        vec![true; 3],
    );
    let mut y = document.axis_record(AxisDimension::Y);
    y.autoscale = false;
    y.minimum = -5.0;
    y.maximum = 100.0;
    document
        .set_axis_record(AxisDimension::Y, y.clone())
        .unwrap();
    document.refresh_autoscale().unwrap();
    assert_eq!(document.axis_record(AxisDimension::Y), y);
    let bytes = serde_json::to_vec(document.project()).unwrap();
    let reopened =
        FigureDocument::from_project(crate::project::decode_project(&bytes).unwrap()).unwrap();
    assert_eq!(
        bounds(&reopened, AxisIdentity::Y1),
        DataBounds {
            minimum: 10.0,
            maximum: 20.0
        }
    );
    assert_eq!(reopened.axis_record(AxisDimension::Y), y);
}

#[test]
fn imported_constant_small_signal_uses_relative_not_unit_padding() {
    let document = gap_document(vec![1.0, 2.0], vec![1e-9, 1e-9], vec![true; 2]);
    let y = document.axis_record(AxisDimension::Y);
    assert!(y.minimum > 0.0 && y.minimum < 1e-9);
    assert!(y.maximum > 1e-9 && y.maximum < 2e-9);
    let layout = document.layout_figure().unwrap();
    assert!(!layout.result.y_axis.duplicate_tick_labels);
}

#[test]
fn imported_opposite_extreme_values_do_not_overflow_padding() {
    let document = gap_document(vec![1.0, 2.0], vec![-1e308, 1e308], vec![true; 2]);
    let y = document.axis_record(AxisDimension::Y);
    assert!(y.minimum.is_finite() && y.minimum <= -1e308);
    assert!(y.maximum.is_finite() && y.maximum >= 1e308);
}

#[test]
fn all_missing_pairs_do_not_produce_fake_zero_data_bounds() {
    let mut document = gap_document(vec![1.0, 2.0], vec![10.0, 20.0], vec![true; 2]);
    let DataSourcePayload::Embedded { columns, .. } = &document.project.data_sources[0].payload
    else {
        panic!("embedded fixture");
    };
    let mut columns = columns.clone();
    columns[1].values = vec![0.0; 2];
    columns[1].valid = vec![false; 2];
    document
        .project
        .upsert_embedded_source(
            "gaps",
            "gaps",
            columns,
            vec![true; 2],
            DataSourceKind::Source,
            None,
        )
        .unwrap();
    assert!(
        compute_axis_data_bounds(
            document.project(),
            AxisIdentity::Y1,
            AutoscalePolicy::default()
        )
        .is_err()
    );
    document.refresh_autoscale().unwrap();
    let y = document.axis_record(AxisDimension::Y);
    assert_eq!((y.minimum, y.maximum), (0.0, 1.0));
}

#[test]
fn error_bars_skip_invalid_errors_and_incomplete_xy_pairs() {
    let mut document = gap_document(
        vec![1.0, 2.0, 3.0],
        vec![10.0, 20.0, f64::NAN],
        vec![true; 3],
    );
    let DataSourcePayload::Embedded { columns, .. } = &document.project.data_sources[0].payload
    else {
        panic!("embedded fixture");
    };
    let mut columns = columns.clone();
    columns.push(EmbeddedColumn {
        name: "err".to_owned(),
        values: vec![1.0, 0.0, 1e6],
        valid: vec![true, false, true],
    });
    document
        .project
        .upsert_embedded_source(
            "gaps",
            "gaps",
            columns,
            vec![true; 3],
            DataSourceKind::Source,
            None,
        )
        .unwrap();
    document
        .create_error_bars("gaps", "x", "y", "err", None)
        .unwrap();
    assert_eq!(
        bounds(&document, AxisIdentity::Y1),
        DataBounds {
            minimum: 9.0,
            maximum: 20.0
        }
    );
    document.layout_figure().unwrap();
}
