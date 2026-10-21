use crate::domain::food::FoodId;

/// Errors that stop a calculation (ARCHITECTURE.md §5).
///
/// Incomplete nutrient data is not an error: it is reported in the result.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("quantity must be a positive number of grams, got {grams}")]
    InvalidQuantity { grams: i64 },

    #[error("number of servings must be a positive integer, got {servings}")]
    InvalidServings { servings: i64 },

    #[error("nutrient amount must not be negative, got {milli} thousandths")]
    NegativeAmount { milli: i64 },

    #[error("food {food_id} is not in the catalog")]
    FoodNotFound { food_id: FoodId },

    #[error("food {food_id} is already in the catalog")]
    DuplicateFood { food_id: FoodId },

    #[error("food {food_id} has a reference quantity of {grams} g; only 100 g is supported")]
    UnsupportedReferenceQuantity { food_id: FoodId, grams: u32 },

    #[error("recipe has no ingredients")]
    EmptyRecipe,

    #[error("no ingredient at position {index}; the recipe has {len}")]
    IngredientIndexOutOfRange { index: usize, len: usize },

    #[error("the recipe's cooked weight is unknown, so a portion in grams cannot be calculated")]
    CookedWeightUnknown,

    #[error("nutrient {code} appears more than once in the nutrient list")]
    DuplicateNutrient { code: String },

    #[error("nutrient {code} was not calculated")]
    NutrientNotCalculated { code: String },

    #[error("calculation overflow")]
    Overflow,
}
