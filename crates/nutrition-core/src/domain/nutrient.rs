use crate::domain::quantity::Unit;
use crate::Error;

/// A nutrient: a substance under one definition, in one canonical unit.
///
/// Two values with different definitions are different nutrients and are never
/// summed together (DATA_MODEL.md §2). Nutrients are data, not an enum (D20).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Nutrient {
    code: String,
    name: String,
    unit: Unit,
}

impl Nutrient {
    /// A nutrient with a stable `code`, a display `name`, and its canonical `unit`.
    ///
    /// # Arguments
    ///
    /// * `code` - stable identifier including the definition, e.g. `carbohydrate_available`.
    /// * `name` - display name, e.g. `Available carbohydrate`.
    /// * `unit` - canonical unit of every amount of this nutrient (D14).
    pub fn new(code: impl Into<String>, name: impl Into<String>, unit: Unit) -> Self {
        Nutrient {
            code: code.into(),
            name: name.into(),
            unit,
        }
    }

    /// Stable identifier including the definition, e.g. `carbohydrate_available`.
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Display name, e.g. `Available carbohydrate`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Canonical unit in which all amounts of this nutrient are expressed (D14).
    pub fn unit(&self) -> Unit {
        self.unit
    }
}

/// The initial nutrient set (D7), with codes and canonical units from D14.
pub fn initial_nutrients() -> Vec<Nutrient> {
    use Unit::*;
    [
        ("energy_kj", "Energy", Kilojoule),
        ("energy_kcal", "Energy", Kilocalorie),
        ("protein", "Protein", Gram),
        ("carbohydrate_available", "Available carbohydrate", Gram),
        ("sugars", "Sugars", Gram),
        ("fibre", "Fibre", Gram),
        ("fat", "Fat", Gram),
        ("fat_saturated", "Saturated fat", Gram),
        ("salt", "Salt", Gram),
        ("calcium", "Calcium", Milligram),
        ("iron", "Iron", Milligram),
        ("vitamin_c", "Vitamin C", Milligram),
        ("vitamin_d", "Vitamin D", Microgram),
        ("vitamin_b12", "Vitamin B12", Microgram),
    ]
    .into_iter()
    .map(|(code, name, unit)| Nutrient::new(code, name, unit))
    .collect()
}

/// A non-negative amount from a composition record, in thousandths of the
/// canonical unit per reference quantity.
///
/// It can only be built through [`NutrientValue::known`] and
/// [`NutrientValue::below_limit`], which validate it (D18).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MeasuredAmount(i64);

impl MeasuredAmount {
    /// The amount in thousandths of the canonical unit.
    pub fn milli(self) -> i64 {
        self.0
    }
}

/// The value of one nutrient in a food composition (DATA_MODEL.md §4).
///
/// `Known` always holds a positive amount: a known 0 is `Zero` (D18).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NutrientValue {
    Known(MeasuredAmount),
    Zero,
    Trace,
    BelowLimit { limit: MeasuredAmount },
    Unknown,
}

impl NutrientValue {
    /// A measured amount. 0 becomes [`NutrientValue::Zero`] (D18); negative is an error.
    ///
    /// # Arguments
    ///
    /// * `milli` - measured amount per reference quantity, in thousandths of the canonical unit.
    pub fn known(milli: i64) -> Result<Self, Error> {
        match milli {
            0 => Ok(NutrientValue::Zero),
            m if m > 0 => Ok(NutrientValue::Known(MeasuredAmount(m))),
            _ => Err(Error::NegativeAmount { milli }),
        }
    }

    /// Below a detection or reporting limit; `limit` is kept for provenance.
    ///
    /// # Arguments
    ///
    /// * `limit` - detection or reporting limit per reference quantity, in thousandths of the canonical unit.
    pub fn below_limit(limit: i64) -> Result<Self, Error> {
        if limit < 0 {
            return Err(Error::NegativeAmount { milli: limit });
        }
        Ok(NutrientValue::BelowLimit {
            limit: MeasuredAmount(limit),
        })
    }

    /// Contribution to a total, in thousandths per reference quantity (D8).
    pub(crate) fn contribution(self) -> i64 {
        match self {
            NutrientValue::Known(amount) => amount.0,
            NutrientValue::Zero
            | NutrientValue::Trace
            | NutrientValue::BelowLimit { .. }
            | NutrientValue::Unknown => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A known 0 becomes `Zero` and a negative amount is rejected (D18).
    #[test]
    fn known_zero_is_normalized_to_zero_state() {
        assert_eq!(NutrientValue::known(0), Ok(NutrientValue::Zero));
        assert!(matches!(NutrientValue::known(5), Ok(NutrientValue::Known(a)) if a.milli() == 5));
        assert_eq!(
            NutrientValue::known(-1),
            Err(Error::NegativeAmount { milli: -1 })
        );
    }

    /// The initial nutrient set has 14 distinct codes (D7).
    #[test]
    fn initial_set_has_unique_codes() {
        let nutrients = initial_nutrients();
        let mut codes: Vec<_> = nutrients.iter().map(Nutrient::code).collect();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), 14);
    }
}
