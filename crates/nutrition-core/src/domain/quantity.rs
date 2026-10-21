use std::fmt;

use crate::Error;

/// A positive mass in whole grams (D6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Grams(u32);

impl Grams {
    /// The default reference quantity of a food composition.
    pub const HUNDRED: Grams = Grams(100);

    /// Rejects zero, negative, and values above `u32::MAX`.
    pub fn new(grams: i64) -> Result<Self, Error> {
        u32::try_from(grams)
            .ok()
            .filter(|g| *g > 0)
            .map(Grams)
            .ok_or(Error::InvalidQuantity { grams })
    }

    pub fn get(self) -> u32 {
        self.0
    }
}

impl fmt::Display for Grams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} g", self.0)
    }
}

/// Canonical unit of a nutrient (D14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unit {
    Gram,
    Milligram,
    Microgram,
    Kilojoule,
    Kilocalorie,
}

impl Unit {
    pub fn symbol(self) -> &'static str {
        match self {
            Unit::Gram => "g",
            Unit::Milligram => "mg",
            Unit::Microgram => "µg",
            Unit::Kilojoule => "kJ",
            Unit::Kilocalorie => "kcal",
        }
    }

    /// Number of decimals shown for this unit (D16).
    pub fn display_decimals(self) -> u32 {
        match self {
            Unit::Gram | Unit::Milligram => 1,
            Unit::Microgram => 2,
            Unit::Kilojoule | Unit::Kilocalorie => 0,
        }
    }
}

/// Number of fixed-point decimals in an [`Amount`] (D1).
pub const AMOUNT_DECIMALS: u32 = 3;

/// A nutrient amount in thousandths of its canonical unit (D1).
///
/// `Amount::from_milli(12_345)` is 12.345 units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Amount(i64);

impl Amount {
    pub const ZERO: Amount = Amount(0);

    pub fn from_milli(milli: i64) -> Self {
        Amount(milli)
    }

    pub fn milli(self) -> i64 {
        self.0
    }

    /// Formats the amount with the unit's display precision (D16), e.g. `46.5 g`.
    pub fn display(self, unit: Unit) -> AmountDisplay {
        AmountDisplay { amount: self, unit }
    }
}

/// Display adapter returned by [`Amount::display`].
pub struct AmountDisplay {
    amount: Amount,
    unit: Unit,
}

impl fmt::Display for AmountDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let decimals = self.unit.display_decimals();
        let scale = 10_i128.pow(AMOUNT_DECIMALS - decimals);
        let rounded = div_round(i128::from(self.amount.0), scale);
        let sign = if rounded < 0 { "-" } else { "" };
        let rounded = rounded.abs();
        let unit_factor = 10_i128.pow(decimals);
        let whole = rounded / unit_factor;
        let text = if decimals == 0 {
            format!("{sign}{whole} {}", self.unit.symbol())
        } else {
            let frac = rounded % unit_factor;
            let width = decimals as usize;
            format!("{sign}{whole}.{frac:0width$} {}", self.unit.symbol())
        };
        // `pad` honours width and alignment flags, e.g. `{:>10}`.
        f.pad(&text)
    }
}

/// Integer division rounding half away from zero (D11). `divisor` must be positive.
pub(crate) fn div_round(dividend: i128, divisor: i128) -> i128 {
    debug_assert!(divisor > 0);
    let quotient = dividend / divisor;
    let remainder = dividend % divisor;
    if 2 * remainder.abs() >= divisor {
        quotient + dividend.signum()
    } else {
        quotient
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_zero_and_negative_grams() {
        assert_eq!(Grams::new(0), Err(Error::InvalidQuantity { grams: 0 }));
        assert_eq!(Grams::new(-5), Err(Error::InvalidQuantity { grams: -5 }));
        assert_eq!(Grams::new(150).map(Grams::get), Ok(150));
    }

    #[test]
    fn div_round_rounds_half_away_from_zero() {
        assert_eq!(div_round(86_415, 100), 864);
        assert_eq!(div_round(123_450, 100), 1235);
        assert_eq!(div_round(123_449, 100), 1234);
        assert_eq!(div_round(-123_450, 100), -1235);
        assert_eq!(div_round(0, 100), 0);
    }

    #[test]
    fn displays_with_unit_precision() {
        assert_eq!(
            Amount::from_milli(46_500).display(Unit::Gram).to_string(),
            "46.5 g"
        );
        assert_eq!(
            Amount::from_milli(46_550).display(Unit::Gram).to_string(),
            "46.6 g"
        );
        assert_eq!(
            Amount::from_milli(1_234)
                .display(Unit::Microgram)
                .to_string(),
            "1.23 µg"
        );
        assert_eq!(
            Amount::from_milli(1_235)
                .display(Unit::Microgram)
                .to_string(),
            "1.24 µg"
        );
        assert_eq!(
            Amount::from_milli(617_500)
                .display(Unit::Kilojoule)
                .to_string(),
            "618 kJ"
        );
        assert_eq!(
            Amount::from_milli(40).display(Unit::Milligram).to_string(),
            "0.0 mg"
        );
        assert_eq!(Amount::ZERO.display(Unit::Microgram).to_string(), "0.00 µg");
        assert_eq!(
            format!("{:>8}", Amount::from_milli(46_500).display(Unit::Gram)),
            "  46.5 g"
        );
    }
}
