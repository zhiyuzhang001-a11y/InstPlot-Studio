use layout_engine_spike::{
    Formatter, Locator, Scale, collision_stride, format_ticks, format_ticks_with, minor_ticks,
    minor_ticks_with_interval,
};

#[test]
fn linear_log_fixed_and_minor_locators_are_deterministic() {
    let auto = Locator::Auto {
        target_spacing_pt: 30.0,
    };
    assert_eq!(
        auto.major_ticks(Scale::Linear, -3.0, 3.0, 180.0),
        vec![-3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0]
    );
    assert_eq!(
        auto.major_ticks(Scale::Log10, 1e-3, 1e3, 180.0),
        vec![1e-3, 1e-2, 1e-1, 1.0, 10.0, 100.0, 1000.0]
    );
    let interval = Locator::Interval { step: 0.5 };
    assert_eq!(
        interval.major_ticks(Scale::Linear, -1.2, 1.2, 180.0),
        vec![-1.0, -0.5, 0.0, 0.5, 1.0]
    );
    assert!(
        interval
            .major_ticks(Scale::Log10, 1.0, 100.0, 180.0)
            .is_empty()
    );
    let fixed = Locator::Fixed(vec![-4.0, -1.5, 0.0, 2.5, 4.0]);
    assert_eq!(
        fixed.major_ticks(Scale::Linear, -3.0, 3.0, 180.0),
        vec![-1.5, 0.0, 2.5]
    );
    assert_eq!(minor_ticks(Scale::Linear, &[0.0, 1.0], 0.0, 1.0).len(), 4);
    assert_eq!(minor_ticks(Scale::Log10, &[1.0, 10.0], 1.0, 10.0).len(), 8);
}

#[test]
fn interval_locator_includes_decimal_endpoint_without_adding_an_outside_tick() {
    let interval = Locator::Interval { step: 0.05 };
    let ticks = interval.major_ticks(Scale::Linear, 0.45, 0.7, 180.0);
    assert_eq!(ticks.len(), 6);
    for (actual, expected) in ticks.iter().zip([0.45, 0.5, 0.55, 0.6, 0.65, 0.7]) {
        assert!((*actual - expected).abs() < 1.0e-14);
    }
    assert_eq!(ticks.last(), Some(&0.7));
    let clipped = interval.major_ticks(Scale::Linear, 0.45, 0.699, 180.0);
    assert_eq!(clipped.len(), 5);
    assert!(clipped
        .iter()
        .zip([0.45, 0.5, 0.55, 0.6, 0.65])
        .all(|(actual, expected)| (*actual - expected).abs() < 1.0e-14));
}

#[test]
fn interval_endpoint_tolerance_is_scale_aware_across_signs_and_magnitudes() {
    assert_eq!(
        Locator::Interval { step: 0.2 }.major_ticks(Scale::Linear, -0.6, 0.6, 180.0),
        vec![-0.6, -0.4, -0.2, 0.0, 0.2, 0.4, 0.6]
    );
    assert_eq!(
        Locator::Interval { step: 1.0e-12 }.major_ticks(
            Scale::Linear,
            -2.0e-12,
            2.0e-12,
            180.0,
        ),
        vec![-2.0e-12, -1.0e-12, 0.0, 1.0e-12, 2.0e-12]
    );
    assert_eq!(
        Locator::Interval { step: 1.0e12 }.major_ticks(
            Scale::Linear,
            -2.0e12,
            2.0e12,
            180.0,
        ),
        vec![-2.0e12, -1.0e12, 0.0, 1.0e12, 2.0e12]
    );
}

#[test]
fn linear_minor_ticks_extend_past_outer_major_ticks_and_clip_to_axis() {
    let minor = minor_ticks(Scale::Linear, &[-2.0, 0.0, 2.0], -3.0, 3.0);
    let tenths: Vec<i32> = minor
        .iter()
        .map(|value| (value * 10.0).round() as i32)
        .collect();
    assert_eq!(tenths, [-28, -24, -16, -12, -8, -4, 4, 8, 12, 16, 24, 28]);
    assert!(minor.iter().all(|value| *value > -3.0 && *value < 3.0));

    let wider = minor_ticks(Scale::Linear, &[0.0, 1.0], -1.2, 2.2);
    assert_eq!(wider.len(), 12);
    assert!(wider.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(wider.iter().all(|value| *value > -1.2 && *value < 2.2));
    assert!(minor_ticks(Scale::Linear, &[0.0], -1.0, 1.0).is_empty());
}

#[test]
fn explicit_minor_interval_uses_data_units_without_drawing_on_major_ticks() {
    let minor = minor_ticks_with_interval(Scale::Linear, &[-2.0, 0.0, 2.0], -3.0, 3.0, Some(0.5));
    assert_eq!(minor, [-2.5, -1.5, -1.0, -0.5, 0.5, 1.0, 1.5, 2.5]);
    assert_eq!(
        minor_ticks_with_interval(Scale::Linear, &[-2.0, 0.0, 2.0], -3.0, 3.0, None),
        minor_ticks(Scale::Linear, &[-2.0, 0.0, 2.0], -3.0, 3.0)
    );
    assert!(minor_ticks_with_interval(Scale::Log10, &[1.0, 10.0], 1.0, 10.0, Some(0.5)).is_empty());
}

#[test]
fn scalar_and_shared_exponent_formatting_suppress_negative_zero() {
    let plain = format_ticks(&[-0.000_000_000_000_1, 0.25, 0.5], Some(0.25));
    assert_eq!(plain.labels, ["0", "0.25", "0.5"]);
    assert_eq!(plain.shared_exponent, None);

    let scientific = format_ticks(&[10_000.0, 20_000.0], Some(10_000.0));
    assert_eq!(scientific.shared_exponent, Some(3));
    assert_eq!(scientific.labels, ["10", "20"]);
}

#[test]
fn negative_tick_labels_use_the_mathematical_minus_sign() {
    let automatic = format_ticks(&[-1.0, 0.0, 1.0], Some(1.0));
    assert_eq!(automatic.labels, ["−1", "0", "1"]);

    let decimal = format_ticks_with(&[-1.5], None, &Formatter::Decimal { precision: 1 });
    assert_eq!(decimal.labels, ["−1.5"]);

    let scientific = format_ticks_with(
        &[-1.0e-3],
        None,
        &Formatter::Scientific { precision: 1 },
    );
    assert_eq!(scientific.labels, ["−1.0e−3"]);
}

#[test]
fn explicit_decimal_and_scientific_formatters_are_deterministic() {
    let decimal = format_ticks_with(
        &[-0.000_000_1, 1.25],
        None,
        &Formatter::Decimal { precision: 2 },
    );
    assert_eq!(decimal.labels, ["0", "1.25"]);
    assert_eq!(decimal.shared_exponent, None);

    let scientific = format_ticks_with(
        &[0.0, 12_500.0],
        None,
        &Formatter::Scientific { precision: 2 },
    );
    assert_eq!(scientific.labels, ["0.00e0", "1.25e4"]);
    assert_eq!(scientific.shared_exponent, None);
}

#[test]
fn collision_detection_selects_a_repeatable_stride() {
    assert_eq!(
        collision_stride(&[0.0, 10.0, 20.0, 30.0], &[14.0; 4], 2.0),
        2
    );
}
