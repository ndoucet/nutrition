use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt;

use crate::domain::nutrient::NutrientValue;
use crate::domain::quantity::Grams;
use crate::Error;

/// Integer identifier of a food (D2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FoodId(pub i64);

impl fmt::Display for FoodId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

/// Preparation state of a food (D9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum State {
    Raw,
    Cooked,
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            State::Raw => "raw",
            State::Cooked => "cooked",
        })
    }
}

/// Nutrient values of a food in one state, per reference quantity of edible portion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Composition {
    reference_quantity: Grams,
    values: HashMap<String, NutrientValue>,
}

impl Composition {
    pub fn new(reference_quantity: Grams) -> Self {
        Composition {
            reference_quantity,
            values: HashMap::new(),
        }
    }

    /// A composition per 100 g, the default reference quantity.
    pub fn per_100g() -> Self {
        Composition::new(Grams::HUNDRED)
    }

    /// Builder form of [`Composition::set`].
    pub fn with(mut self, nutrient_code: impl Into<String>, value: NutrientValue) -> Self {
        self.set(nutrient_code, value);
        self
    }

    pub fn set(&mut self, nutrient_code: impl Into<String>, value: NutrientValue) {
        self.values.insert(nutrient_code.into(), value);
    }

    /// The value for a nutrient; a missing record is [`NutrientValue::Unknown`].
    pub fn value(&self, nutrient_code: &str) -> NutrientValue {
        self.values
            .get(nutrient_code)
            .copied()
            .unwrap_or(NutrientValue::Unknown)
    }

    pub fn reference_quantity(&self) -> Grams {
        self.reference_quantity
    }
}

/// The compositions of a food: one per state, or one shared by both when the
/// source does not distinguish raw and cooked (DATA_MODEL.md §3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Compositions {
    Shared(Composition),
    Distinct {
        raw: Composition,
        cooked: Composition,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Food {
    id: FoodId,
    name: String,
    compositions: Compositions,
}

impl Food {
    pub fn new(id: FoodId, name: impl Into<String>, compositions: Compositions) -> Self {
        Food {
            id,
            name: name.into(),
            compositions,
        }
    }

    pub fn id(&self) -> FoodId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn compositions(&self) -> &Compositions {
        &self.compositions
    }

    /// The composition for `state`, falling back to the shared one.
    pub fn composition(&self, state: State) -> &Composition {
        match (&self.compositions, state) {
            (Compositions::Shared(composition), _) => composition,
            (Compositions::Distinct { raw, .. }, State::Raw) => raw,
            (Compositions::Distinct { cooked, .. }, State::Cooked) => cooked,
        }
    }

    /// True when raw and cooked use the same composition.
    pub fn is_shared(&self) -> bool {
        matches!(self.compositions, Compositions::Shared(_))
    }
}

/// In-memory set of foods passed to calculations. The persistence layer fills it.
#[derive(Debug, Clone, Default)]
pub struct FoodCatalog {
    foods: HashMap<FoodId, Food>,
}

impl FoodCatalog {
    pub fn new() -> Self {
        FoodCatalog::default()
    }

    pub fn insert(&mut self, food: Food) -> Result<(), Error> {
        match self.foods.entry(food.id()) {
            Entry::Occupied(entry) => Err(Error::DuplicateFood {
                food_id: *entry.key(),
            }),
            Entry::Vacant(entry) => {
                entry.insert(food);
                Ok(())
            }
        }
    }

    pub fn get(&self, id: FoodId) -> Option<&Food> {
        self.foods.get(&id)
    }

    pub fn len(&self) -> usize {
        self.foods.len()
    }

    pub fn is_empty(&self) -> bool {
        self.foods.is_empty()
    }
}
