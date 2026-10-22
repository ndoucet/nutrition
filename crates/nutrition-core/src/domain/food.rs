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
    /// Formats as `#<id>`, e.g. `#42`.
    ///
    /// # Arguments
    ///
    /// * `f` - formatter that receives the text.
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
    /// Formats as `raw` or `cooked`.
    ///
    /// # Arguments
    ///
    /// * `f` - formatter that receives the text.
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
    /// An empty composition per `reference_quantity` of edible portion.
    ///
    /// # Arguments
    ///
    /// * `reference_quantity` - quantity of edible portion the values refer to, in grams.
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
    ///
    /// # Arguments
    ///
    /// * `nutrient_code` - code of the nutrient, e.g. `protein`.
    /// * `value` - value per reference quantity; amounts are in thousandths of the nutrient's canonical unit.
    pub fn with(mut self, nutrient_code: impl Into<String>, value: NutrientValue) -> Self {
        self.set(nutrient_code, value);
        self
    }

    /// Sets the value of a nutrient, replacing any previous one.
    ///
    /// # Arguments
    ///
    /// * `nutrient_code` - code of the nutrient, e.g. `protein`.
    /// * `value` - value per reference quantity; amounts are in thousandths of the nutrient's canonical unit.
    pub fn set(&mut self, nutrient_code: impl Into<String>, value: NutrientValue) {
        self.values.insert(nutrient_code.into(), value);
    }

    /// The value for a nutrient; a missing record is [`NutrientValue::Unknown`].
    ///
    /// # Arguments
    ///
    /// * `nutrient_code` - code of the nutrient, e.g. `protein`.
    pub fn value(&self, nutrient_code: &str) -> NutrientValue {
        self.values
            .get(nutrient_code)
            .copied()
            .unwrap_or(NutrientValue::Unknown)
    }

    /// Quantity of edible portion the values refer to, usually 100 g.
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
    /// A food with its identifier, display name, and compositions.
    ///
    /// # Arguments
    ///
    /// * `id` - identifier of the food, unique within a catalog.
    /// * `name` - display name of the food.
    /// * `compositions` - nutrient values, shared or per state.
    pub fn new(id: FoodId, name: impl Into<String>, compositions: Compositions) -> Self {
        Food {
            id,
            name: name.into(),
            compositions,
        }
    }

    /// Identifier of the food.
    pub fn id(&self) -> FoodId {
        self.id
    }

    /// Display name of the food.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The compositions of the food, shared or per state.
    pub fn compositions(&self) -> &Compositions {
        &self.compositions
    }

    /// The composition for `state`, falling back to the shared one.
    ///
    /// # Arguments
    ///
    /// * `state` - preparation state of the food as used in the recipe.
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
    /// An empty catalog.
    pub fn new() -> Self {
        FoodCatalog::default()
    }

    /// Adds a food; a food with the same identifier already present is an error.
    ///
    /// # Arguments
    ///
    /// * `food` - the food to add.
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

    /// The food with identifier `id`, if present.
    ///
    /// # Arguments
    ///
    /// * `id` - identifier of the food.
    pub fn get(&self, id: FoodId) -> Option<&Food> {
        self.foods.get(&id)
    }

    /// Number of foods in the catalog.
    pub fn len(&self) -> usize {
        self.foods.len()
    }

    /// True when the catalog holds no food.
    pub fn is_empty(&self) -> bool {
        self.foods.is_empty()
    }
}
