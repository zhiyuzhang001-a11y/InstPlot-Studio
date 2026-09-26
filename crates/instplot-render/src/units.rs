use core::fmt;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mm(f64);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pt(f64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Px(u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnitError {
    NonFinite,
    Negative,
}

impl fmt::Display for UnitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFinite => formatter.write_str("unit value must be finite"),
            Self::Negative => formatter.write_str("unit value must not be negative"),
        }
    }
}

impl std::error::Error for UnitError {}

impl Mm {
    pub fn new(value: f64) -> Result<Self, UnitError> {
        finite_non_negative(value).map(Self)
    }

    pub const fn get(self) -> f64 {
        self.0
    }

    pub fn to_pt(self) -> Pt {
        Pt(self.0 * 72.0 / 25.4)
    }
}

impl Pt {
    pub fn new(value: f64) -> Result<Self, UnitError> {
        if value.is_finite() {
            Ok(Self(value))
        } else {
            Err(UnitError::NonFinite)
        }
    }

    pub const fn get(self) -> f64 {
        self.0
    }

    pub fn to_px(self, dpi: f64) -> Result<Px, UnitError> {
        let pixels = self.0 * dpi / 72.0;
        finite_non_negative(pixels)?;
        Ok(Px(round_half_away(pixels) as u32))
    }
}

impl Px {
    pub const fn get(self) -> u32 {
        self.0
    }
}

fn finite_non_negative(value: f64) -> Result<f64, UnitError> {
    if !value.is_finite() {
        Err(UnitError::NonFinite)
    } else if value < 0.0 {
        Err(UnitError::Negative)
    } else {
        Ok(value)
    }
}

fn round_half_away(value: f64) -> f64 {
    if value >= 0.0 {
        (value + 0.5).floor()
    } else {
        (value - 0.5).ceil()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_dimensions_match_a1() {
        let width = Mm::new(89.0).unwrap().to_pt();
        let height = Mm::new(65.0).unwrap().to_pt();
        assert!((width.get() - 252.28346).abs() < 0.00001);
        assert!((height.get() - 184.25197).abs() < 0.00001);
        assert_eq!(width.to_px(300.0).unwrap().get(), 1051);
        assert_eq!(height.to_px(300.0).unwrap().get(), 768);
    }
}
