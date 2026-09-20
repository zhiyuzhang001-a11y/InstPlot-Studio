#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scale {
    Linear,
    Log10,
}

impl Scale {
    pub fn map(self, value: f64, minimum: f64, maximum: f64) -> Option<f64> {
        if !value.is_finite() || !minimum.is_finite() || !maximum.is_finite() || minimum >= maximum
        {
            return None;
        }
        match self {
            Self::Linear => Some((value - minimum) / (maximum - minimum)),
            Self::Log10 if value > 0.0 && minimum > 0.0 => {
                Some((value.log10() - minimum.log10()) / (maximum.log10() - minimum.log10()))
            }
            Self::Log10 => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Locator {
    Auto { target_spacing_pt: f64 },
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
            Self::Fixed(values) => values
                .iter()
                .copied()
                .filter(|value| value.is_finite() && *value >= minimum && *value <= maximum)
                .collect(),
            Self::Auto { target_spacing_pt } => match scale {
                Scale::Linear => {
                    let target_count = (length_pt / target_spacing_pt.max(1.0)).floor().max(2.0);
                    let step = nice_step((maximum - minimum) / target_count);
                    linear_ticks(minimum, maximum, step)
                }
                Scale::Log10 if minimum > 0.0 => {
                    let first = minimum.log10().ceil() as i32;
                    let last = maximum.log10().floor() as i32;
                    (first..=last).map(|power| 10_f64.powi(power)).collect()
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
            for pair in major.windows(2) {
                let step = (pair[1] - pair[0]) / 5.0;
                for index in 1..5 {
                    let value = pair[0] + step * index as f64;
                    if value > minimum && value < maximum {
                        ticks.push(clean_zero(value));
                    }
                }
            }
        }
        Scale::Log10 => {
            let first = minimum.log10().floor() as i32;
            let last = maximum.log10().ceil() as i32;
            for power in first..=last {
                let decade = 10_f64.powi(power);
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

#[derive(Clone, Debug, PartialEq)]
pub struct FormattedTicks {
    pub labels: Vec<String>,
    pub shared_exponent: Option<i32>,
}

pub fn format_ticks(values: &[f64], step: Option<f64>) -> FormattedTicks {
    let max_abs = values.iter().copied().map(f64::abs).fold(0.0, f64::max);
    let exponent = if max_abs >= 10_000.0 || (max_abs > 0.0 && max_abs < 0.001) {
        Some((max_abs.log10().floor() as i32).div_euclid(3) * 3)
    } else {
        None
    };
    let scale = exponent.map_or(1.0, |power| 10_f64.powi(power));
    let scaled_step = step.map(|value| value / scale);
    let precision = scaled_step.map(precision_from_step).unwrap_or(3);
    let labels = values
        .iter()
        .map(|value| {
            let value = clean_zero(*value / scale);
            trim_number(format!("{value:.precision$}"))
        })
        .collect();
    FormattedTicks {
        labels,
        shared_exponent: exponent,
    }
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

fn linear_ticks(minimum: f64, maximum: f64, step: f64) -> Vec<f64> {
    let first = (minimum / step).ceil() as i64;
    let last = (maximum / step).floor() as i64;
    (first..=last)
        .map(|index| clean_zero(index as f64 * step))
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
    precision.min(8)
}

fn clean_zero(value: f64) -> f64 {
    if value.abs() < 1e-12 { 0.0 } else { value }
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
