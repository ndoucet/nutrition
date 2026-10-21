//! Domain model and nutrient calculations for the Meal Nutrient Estimator.
//!
//! This crate does no I/O: it receives structured inputs and returns structured
//! results. Amounts are integer fixed-point values (thousandths of the nutrient's
//! canonical unit) and quantities are integer grams (DECISIONS.md, D1).

pub mod calculations;
pub mod domain;
mod error;

pub use calculations::recipe_nutrients::{
    calculate_recipe, IngredientLine, NutrientTotal, RecipeComposition,
};
pub use domain::food::{Composition, Compositions, Food, FoodCatalog, FoodId, State};
pub use domain::nutrient::{initial_nutrients, MeasuredAmount, Nutrient, NutrientValue};
pub use domain::quantity::{Amount, Grams, Unit};
pub use domain::recipe::{Recipe, RecipeItem, Servings};
pub use error::Error;
