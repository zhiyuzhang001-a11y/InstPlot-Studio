use layout_engine_spike::{Locator, Scale, collision_stride, format_ticks, minor_ticks};

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
    let fixed = Locator::Fixed(vec![-4.0, -1.5, 0.0, 2.5, 4.0]);
    assert_eq!(
        fixed.major_ticks(Scale::Linear, -3.0, 3.0, 180.0),
        vec![-1.5, 0.0, 2.5]
    );
    assert_eq!(minor_ticks(Scale::Linear, &[0.0, 1.0], 0.0, 1.0).len(), 4);
    assert_eq!(minor_ticks(Scale::Log10, &[1.0, 10.0], 1.0, 10.0).len(), 8);
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
fn collision_detection_selects_a_repeatable_stride() {
    assert_eq!(
        collision_stride(&[0.0, 10.0, 20.0, 30.0], &[14.0; 4], 2.0),
        2
    );
}
