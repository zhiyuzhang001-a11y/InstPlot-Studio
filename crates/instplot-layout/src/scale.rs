#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scale {
    Linear,
    Log10,
}

const MAX_GENERATED_TICKS: usize = 512;

impl Scale {
    pub fn map(self, value: f64, minimum: f64, maximum: f64) -> Option<f64> {
        if !value.is_finite() || !minimum.is_finite() || !maximum.is_finite() || minimum >= maximum
        {
            return None;
        }
        match self {
            Self::Linear => linear_fraction(value, minimum, maximum),
            Self::Log10 if value > 0.0 && minimum > 0.0 => {
                Some((value.log10() - minimum.log10()) / (maximum.log10() - minimum.log10()))
            }
            Self::Log10 => None,
        }
    }
}

/// Maps a finite value into a finite linear-axis fraction without first
/// subtracting endpoints that may span almost the entire `f64` domain.
pub fn linear_fraction(value: f64, minimum: f64, maximum: f64) -> Option<f64> {
    if !value.is_finite() || !minimum.is_finite() || !maximum.is_finite() || minimum >= maximum {
        return None;
    }
    let scale = minimum.abs().max(maximum.abs());
    if scale == 0.0 {
        return None;
    }
    let scaled_minimum = minimum / scale;
    let scaled_maximum = maximum / scale;
    let denominator = scaled_maximum - scaled_minimum;
    let result = (value / scale - scaled_minimum) / denominator;
    result.is_finite().then_some(result)
}

/// Performs the inverse of [`linear_fraction`] with a weighted sum so that
/// opposite-sign endpoints do not overflow during interpolation.
pub fn linear_value(fraction: f64, minimum: f64, maximum: f64) -> Option<f64> {
    if !fraction.is_finite() || !minimum.is_finite() || !maximum.is_finite() || minimum >= maximum {
        return None;
    }
    if fraction == 0.0 {
        return Some(minimum);
    }
    if fraction == 1.0 {
        return Some(maximum);
    }
    let scale = minimum.abs().max(maximum.abs());
    if scale == 0.0 {
        return None;
    }
    let scaled = (1.0 - fraction) * (minimum / scale) + fraction * (maximum / scale);
    let result = scaled * scale;
    result.is_finite().then_some(result)
}

pub fn finite_span(minimum: f64, maximum: f64) -> Option<f64> {
    if !minimum.is_finite() || !maximum.is_finite() || minimum >= maximum {
        return None;
    }
    let direct = maximum - minimum;
    if direct.is_finite() {
        return Some(direct);
    }
    Some(f64::MAX)
}

pub fn finite_midpoint(left: f64, right: f64) -> Option<f64> {
    if !left.is_finite() || !right.is_finite() {
        return None;
    }
    let midpoint = left * 0.5 + right * 0.5;
    midpoint.is_finite().then_some(midpoint)
}

pub fn checked_pow10(exponent: i32) -> Option<f64> {
    let value = 10_f64.powf(f64::from(exponent));
    (value.is_finite() && value != 0.0).then_some(value)
}

pub fn tick_count_exceeds(minimum: f64, maximum: f64, step: f64, limit: usize) -> bool {
    if !minimum.is_finite()
        || !maximum.is_finite()
        || minimum >= maximum
        || !step.is_finite()
        || step <= 0.0
    {
        return true;
    }
    let scale = minimum.abs().max(maximum.abs());
    let normalized_span = if scale == 0.0 {
        0.0
    } else {
        maximum / scale - minimum / scale
    };
    let normalized_step = step / scale;
    normalized_step == 0.0
        || !normalized_span.is_finite()
        || !normalized_step.is_finite()
        || normalized_span / normalized_step > limit as f64
}

#[derive(Clone, Debug, PartialEq)]
pub enum Locator {
    Auto { target_spacing_pt: f64 },
    Interval { step: f64 },
    Fixed(Vec<f64>),
}

impl Locator {
    pub fn major_ticks(
        &self,
        scale: Scale,
        minimum: f64,
        maximum: f64,
        length_pt: f64,
    ) -> Vec<f64> {
        match self {
            Self::Interval { step }
                if scale == Scale::Linear && step.is_finite() && *step > 0.0 =>
            {
                linear_ticks(minimum, maximum, *step)
            }
            Self::Interval { .. } => Vec::new(),
            Self::Fixed(values) => values
                .iter()
                .copied()
                .filter(|value| value.is_finite() && *value >= minimum && *value <= maximum)
                .collect(),
            Self::Auto { target_spacing_pt } => match scale {
                Scale::Linear => {
                    let target_count = (length_pt / target_spacing_pt.max(1.0))
                        .floor()
                        .clamp(2.0, 20.0) as usize;
                    let span = maximum - minimum;
                    if !span.is_finite() {
                        evenly_spaced_ticks(minimum, maximum, target_count)
                    } else {
                        let step = nice_step(span / target_count as f64);
                        let ticks = linear_ticks(minimum, maximum, step);
                        if ticks.is_empty() {
                            evenly_spaced_ticks(minimum, maximum, target_count)
                        } else {
                            ticks
                        }
                    }
                }
                Scale::Log10 if minimum > 0.0 => {
                    let first = minimum.log10().ceil() as i32;
                    let last = maximum.log10().floor() as i32;
                    (first..=last)
                        .take(MAX_GENERATED_TICKS)
                        .filter_map(checked_pow10)
                        .filter(|value| *value >= minimum && *value <= maximum)
                        .collect()
                }
                Scale::Log10 => Vec::new(),
            },
        }
    }
}

pub fn minor_ticks(scale: Scale, major: &[f64], minimum: f64, maximum: f64) -> Vec<f64> {
    let mut ticks = Vec::new();
    match scale {
        Scale::Linear => {
            if let [first, second, ..] = major {
                let step = (second - first) / 5.0;
                let mut before = linear_minor_edge(*first, -step, minimum, maximum);
                before.reverse();
                ticks.extend(before);
            }
            for pair in major.windows(2) {
                let step = (pair[1] - pair[0]) / 5.0;
                for index in 1..5 {
                    let value = pair[0] + step * index as f64;
                    if value > minimum && value < maximum {
                        ticks.push(clean_zero(value));
                    }
                }
            }
            if let [.., penultimate, last] = major {
                let step = (last - penultimate) / 5.0;
                ticks.extend(linear_minor_edge(*last, step, minimum, maximum));
            }
        }
        Scale::Log10 => {
            let first = minimum.log10().floor() as i32;
            let last = maximum.log10().ceil() as i32;
            for power in (first..=last).take(MAX_GENERATED_TICKS / 8) {
                let Some(decade) = checked_pow10(power) else {
                    continue;
                };
                for multiple in 2..10 {
                    let value = decade * multiple as f64;
                    if value > minimum && value < maximum {
                        ticks.push(value);
                    }
                }
            }
        }
    }
    ticks
}

pub fn minor_ticks_with_interval(
    scale: Scale,
    major: &[f64],
    minimum: f64,
    maximum: f64,
    interval: Option<f64>,
) -> Vec<f64> {
    let Some(step) = interval else {
        return minor_ticks(scale, major, minimum, maximum);
    };
    if scale != Scale::Linear
        || !step.is_finite()
        || step <= 0.0
        || !minimum.is_finite()
        || !maximum.is_finite()
        || minimum >= maximum
        || tick_count_exceeds(minimum, maximum, step, 500)
    {
        return Vec::new();
    }
    let origin = major.first().copied().unwrap_or(0.0);
    if !origin.is_finite() {
        return Vec::new();
    }
    let first = ((minimum - origin) / step).floor() + 1.0;
    let last = ((maximum - origin) / step).ceil() - 1.0;
    if !first.is_finite()
        || !last.is_finite()
        || first.abs() > i64::MAX as f64 / 4.0
        || last.abs() > i64::MAX as f64 / 4.0
    {
        return Vec::new();
    }
    (first as i64..=last as i64)
        .map(|index| origin + index as f64 * step)
        .filter(|value| {
            let tolerance = step * 1e-8 + value.abs() * f64::EPSILON * 16.0;
            value.is_finite()
                && *value > minimum
                && *value < maximum
                && !major
                    .iter()
                    .any(|major| (*value - *major).abs() <= tolerance)
        })
        .map(clean_zero)
        .collect()
}

fn linear_minor_edge(origin: f64, step: f64, minimum: f64, maximum: f64) -> Vec<f64> {
    if !origin.is_finite()
        || !step.is_finite()
        || step == 0.0
        || !minimum.is_finite()
        || !maximum.is_finite()
        || minimum >= maximum
    {
        return Vec::new();
    }
    let mut ticks = Vec::new();
    for index in 1..=10_000 {
        let value = origin + step * index as f64;
        if !value.is_finite() || value <= minimum || value >= maximum {
            break;
        }
        if index % 5 != 0 {
            ticks.push(clean_zero(value));
        }
    }
    ticks
}

#[derive(Clone, Debug, PartialEq)]
pub struct FormattedTicks {
    pub labels: Vec<String>,
    pub shared_exponent: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Formatter {
    Auto,
    Decimal { precision: usize },
    Scientific { precision: usize },
}

pub fn format_ticks(values: &[f64], step: Option<f64>) -> FormattedTicks {
    format_ticks_with(values, step, &Formatter::Auto)
}

pub fn format_ticks_with(
    values: &[f64],
    step: Option<f64>,
    formatter: &Formatter,
) -> FormattedTicks {
    match formatter {
        Formatter::Auto => format_ticks_auto(values, step),
        Formatter::Decimal { precision } => FormattedTicks {
            labels: values
                .iter()
                .map(|value| {
                    mathematical_minus(trim_number(format!("{:.precision$}", clean_zero(*value))))
                })
                .collect(),
            shared_exponent: None,
        },
        Formatter::Scientific { precision } => FormattedTicks {
            labels: values
                .iter()
                .map(|value| mathematical_minus(format!("{:.precision$e}", clean_zero(*value))))
                .collect(),
            shared_exponent: None,
        },
    }
}

pub fn automatic_display_exponent(values: &[f64], minimum: f64, maximum: f64) -> Option<i32> {
    let representative = if values.is_empty() {
        minimum.abs().max(maximum.abs())
    } else {
        values
            .iter()
            .copied()
            .filter(|value| value.is_finite())
            .map(f64::abs)
            .fold(0.0, f64::max)
    };
    if !representative.is_finite()
        || representative == 0.0
        || (1.0e-2..1.0e4).contains(&representative)
    {
        return None;
    }
    let mut exponent = representative.log10().floor() as i32;
    if checked_pow10(exponent).is_some_and(|decade| representative < decade) {
        exponent -= 1;
    }
    while checked_pow10(exponent).is_none() && exponent != 0 {
        exponent -= exponent.signum();
    }
    (exponent != 0).then_some(exponent)
}

pub fn format_ticks_with_exponent(
    values: &[f64],
    step: Option<f64>,
    formatter: &Formatter,
    exponent: i32,
) -> FormattedTicks {
    let scale = checked_pow10(exponent).unwrap_or(1.0);
    let scaled_step = step.map(|value| value / scale);
    match formatter {
        Formatter::Auto => {
            let mut precision = scaled_step.map(precision_from_step).unwrap_or(3);
            let mut labels = format_decimal_labels(values, scale, precision);
            while precision < 17 && duplicate_labels_for_distinct_values(values, &labels) {
                precision += 1;
                labels = format_decimal_labels(values, scale, precision);
            }
            FormattedTicks {
                labels,
                shared_exponent: (exponent != 0).then_some(exponent),
            }
        }
        Formatter::Decimal { precision } => FormattedTicks {
            labels: format_decimal_labels(values, scale, *precision),
            shared_exponent: (exponent != 0).then_some(exponent),
        },
        Formatter::Scientific { precision } => FormattedTicks {
            labels: values
                .iter()
                .map(|value| {
                    mathematical_minus(format!("{:.precision$e}", clean_zero(*value / scale)))
                })
                .collect(),
            shared_exponent: (exponent != 0).then_some(exponent),
        },
    }
}

fn format_ticks_auto(values: &[f64], step: Option<f64>) -> FormattedTicks {
    let exponent = automatic_display_exponent(values, 0.0, 0.0).unwrap_or(0);
    format_ticks_with_exponent(values, step, &Formatter::Auto, exponent)
}

fn format_decimal_labels(values: &[f64], scale: f64, precision: usize) -> Vec<String> {
    values
        .iter()
        .map(|value| {
            let value = clean_zero(*value / scale);
            mathematical_minus(trim_number(format!("{value:.precision$}")))
        })
        .collect()
}

fn duplicate_labels_for_distinct_values(values: &[f64], labels: &[String]) -> bool {
    values.iter().enumerate().any(|(left, left_value)| {
        values
            .iter()
            .enumerate()
            .skip(left + 1)
            .any(|(right, right_value)| left_value != right_value && labels[left] == labels[right])
    })
}

fn mathematical_minus(value: String) -> String {
    value.replace('-', "−")
}

pub fn collision_stride(positions: &[f64], widths: &[f64], gap: f64) -> usize {
    if positions.len() < 2 || positions.len() != widths.len() {
        return 1;
    }
    for stride in 1..positions.len() {
        let visible: Vec<usize> = (0..positions.len()).step_by(stride).collect();
        if visible.windows(2).all(|pair| {
            let left = pair[0];
            let right = pair[1];
            positions[right] - widths[right] / 2.0 >= positions[left] + widths[left] / 2.0 + gap
        }) {
            return stride;
        }
    }
    positions.len()
}

fn nice_step(raw: f64) -> f64 {
    let power = 10_f64.powf(raw.abs().max(f64::MIN_POSITIVE).log10().floor());
    let fraction = raw / power;
    let nice = if fraction <= 1.0 {
        1.0
    } else if fraction <= 2.0 {
        2.0
    } else if fraction <= 2.5 {
        2.5
    } else if fraction <= 5.0 {
        5.0
    } else {
        10.0
    };
    nice * power
}

fn evenly_spaced_ticks(minimum: f64, maximum: f64, intervals: usize) -> Vec<f64> {
    let intervals = intervals.clamp(1, MAX_GENERATED_TICKS - 1);
    let mut ticks = (0..=intervals)
        .filter_map(|index| linear_value(index as f64 / intervals as f64, minimum, maximum))
        .collect::<Vec<_>>();
    ticks.dedup_by(|left, right| *left == *right);
    ticks
}

fn linear_ticks(minimum: f64, maximum: f64, step: f64) -> Vec<f64> {
    if !minimum.is_finite()
        || !maximum.is_finite()
        || minimum >= maximum
        || !step.is_finite()
        || step <= 0.0
        || tick_count_exceeds(minimum, maximum, step, MAX_GENERATED_TICKS)
    {
        return Vec::new();
    }
    let minimum_index = minimum / step;
    let maximum_index = maximum / step;
    if !minimum_index.is_finite()
        || !maximum_index.is_finite()
        || minimum_index.abs() > i64::MAX as f64 / 2.0
        || maximum_index.abs() > i64::MAX as f64 / 2.0
    {
        return Vec::new();
    }
    let minimum_tolerance = minimum_index.abs() * f64::EPSILON * 32.0;
    let maximum_tolerance = maximum_index.abs() * f64::EPSILON * 32.0;
    let first = (minimum_index - minimum_tolerance).ceil() as i64;
    let last = (maximum_index + maximum_tolerance).floor() as i64;
    (first..=last)
        .map(|index| {
            let value = if index == 0 { 0.0 } else { index as f64 * step };
            let tolerance = value
                .abs()
                .max(minimum.abs())
                .max(maximum.abs())
                .max(step.abs())
                * f64::EPSILON
                * 64.0;
            if (value - minimum).abs() <= tolerance {
                minimum
            } else if (value - maximum).abs() <= tolerance {
                maximum
            } else {
                value
            }
        })
        .filter(|value| {
            let tolerance = value.abs().max(step) * f64::EPSILON * 64.0;
            *value >= minimum - tolerance && *value <= maximum + tolerance
        })
        .collect()
}

fn precision_from_step(step: f64) -> usize {
    if step == 0.0 || !step.is_finite() {
        return 3;
    }
    let mut precision = (-step.abs().log10().floor()).max(0.0) as usize;
    let scaled = step.abs() * 10_f64.powi(precision as i32);
    if (scaled - scaled.round()).abs() > 1e-9 {
        precision += 1;
    }
    precision.min(17)
}

fn clean_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

fn trim_number(mut value: String) -> String {
    if value.contains('.') {
        while value.ends_with('0') {
            value.pop();
        }
        if value.ends_with('.') {
            value.pop();
        }
    }
    if value == "-0" { "0".into() } else { value }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_mapping_handles_the_full_finite_domain() {
        let mapped = Scale::Linear
            .map(0.0, -f64::MAX, f64::MAX)
            .expect("the full finite domain is a valid linear axis");
        assert!(mapped.is_finite());
        assert!((mapped - 0.5).abs() <= f64::EPSILON);
    }

    #[test]
    fn minor_ticks_do_not_erase_real_tiny_values() {
        let major = vec![-2.0e-15, 0.0, 2.0e-15];
        let minor = minor_ticks(Scale::Linear, &major, -2.5e-15, 2.5e-15);
        assert!(minor.iter().any(|value| *value != 0.0), "{minor:?}");
        assert!(minor.iter().all(|value| value.is_finite()));
    }

    #[test]
    fn automatic_labels_distinguish_large_offset_small_steps() {
        let values = vec![1.0e12, 1.0e12 + 0.000_122_070_312_5];
        let formatted = format_ticks(&values, Some(values[1] - values[0]));
        assert_ne!(formatted.labels[0], formatted.labels[1], "{formatted:?}");
    }

    #[test]
    fn automatic_locator_keeps_extreme_finite_ranges_bounded() {
        let ticks = Locator::Auto {
            target_spacing_pt: 40.0,
        }
        .major_ticks(Scale::Linear, -f64::MAX, f64::MAX, 320.0);
        assert!(!ticks.is_empty());
        assert!(ticks.len() <= 32, "{} ticks", ticks.len());
        assert!(ticks.iter().all(|value| value.is_finite()));
        assert!(ticks.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn automatic_exponent_obeys_publication_thresholds_and_subnormal_fallback() {
        assert_eq!(automatic_display_exponent(&[0.01], 0.0, 0.01), None);
        assert_eq!(
            automatic_display_exponent(&[0.01_f64.next_down()], 0.0, 0.01),
            Some(-3)
        );
        assert_eq!(automatic_display_exponent(&[9_999.0], 0.0, 9_999.0), None);
        assert_eq!(
            automatic_display_exponent(&[10_000.0], 0.0, 10_000.0),
            Some(4)
        );
        assert_eq!(
            automatic_display_exponent(&[f64::from_bits(1)], 0.0, f64::from_bits(1)),
            Some(-323)
        );
    }

    #[test]
    fn automatic_exponent_only_falls_back_to_domain_when_ticks_are_absent() {
        assert_eq!(automatic_display_exponent(&[0.0], -1.0e9, 1.0e9), None);
        assert_eq!(automatic_display_exponent(&[], -1.0e9, 1.0e9), Some(9));
    }
}
