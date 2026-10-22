//! Recipe nutrient aggregation (CALCULATION.md §2–4, §11).

use std::fmt;

use crate::domain::food::{FoodCatalog, FoodId, State};
use crate::domain::nutrient::{Nutrient, NutrientValue};
use crate::domain::quantity::{div_round, Amount, Grams};
use crate::domain::recipe::{Recipe, Servings};
use crate::Error;

/// One ingredient as used in a calculation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngredientLine {
    pub position: usize,
    pub food_id: FoodId,
    pub food_name: String,
    pub state: State,
    pub quantity: Grams,
    /// True when the source does not distinguish raw and cooked, so the same
    /// composition was used for the selected state (D9).
    pub shared_composition: bool,
}

/// The aggregated amount of one nutrient in a recipe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NutrientTotal {
    nutrient: Nutrient,
    /// `sum(N_i × Q_i)` before division by the reference quantity, kept so that
    /// derived values are divided once (D12).
    sum_of_products: i128,
    total: Amount,
    per_serving: Amount,
    unknown_items: Vec<usize>,
}

impl NutrientTotal {
    /// The nutrient this total is for.
    pub fn nutrient(&self) -> &Nutrient {
        &self.nutrient
    }

    /// Amount in the whole recipe, rounded once from the unrounded sum (D12).
    pub fn total(&self) -> Amount {
        self.total
    }

    /// Amount per serving, divided once from the unrounded sum (D12).
    pub fn per_serving(&self) -> Amount {
        self.per_serving
    }

    /// Positions of the ingredients whose value for this nutrient is unknown.
    /// They contributed 0 to the total (D8).
    pub fn unknown_items(&self) -> &[usize] {
        &self.unknown_items
    }
}

/// Result of [`calculate_recipe`]: totals, per-serving values, and data-quality lists.
///
/// Its `Display` prints the recipe line by line; ingredients are numbered from 1
/// there, while the API uses positions from 0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipeComposition {
    recipe_name: String,
    servings: Servings,
    cooked_weight: Option<Grams>,
    ingredients: Vec<IngredientLine>,
    nutrients: Vec<NutrientTotal>,
}

impl RecipeComposition {
    /// Name of the recipe the result was calculated for.
    pub fn recipe_name(&self) -> &str {
        &self.recipe_name
    }

    /// Number of servings used for the per-serving values.
    pub fn servings(&self) -> Servings {
        self.servings
    }

    /// Ingredients in recipe order, with their positions from 0.
    pub fn ingredients(&self) -> &[IngredientLine] {
        &self.ingredients
    }

    /// Totals in the order of the nutrient list given to the calculation.
    pub fn nutrients(&self) -> &[NutrientTotal] {
        &self.nutrients
    }

    /// The total for the nutrient with `code`, if it was calculated.
    ///
    /// # Arguments
    ///
    /// * `code` - nutrient code, e.g. `protein`.
    pub fn nutrient(&self, code: &str) -> Option<&NutrientTotal> {
        self.nutrients.iter().find(|t| t.nutrient.code() == code)
    }

    /// Positions of the ingredients that used the raw/cooked shared composition.
    pub fn shared_composition_items(&self) -> Vec<usize> {
        self.ingredients
            .iter()
            .filter(|i| i.shared_composition)
            .map(|i| i.position)
            .collect()
    }

    /// Amount of a nutrient in a portion of `portion` grams of the finished dish:
    /// `sum(N_i × Q_i) × P / (R × W)` (CALCULATION.md §4, D12).
    ///
    /// # Arguments
    ///
    /// * `nutrient_code` - code of a nutrient included in this result, e.g. `protein`.
    /// * `portion` - weight of the portion of finished dish, in grams.
    pub fn portion(&self, nutrient_code: &str, portion: Grams) -> Result<Amount, Error> {
        let cooked_weight = self.cooked_weight.ok_or(Error::CookedWeightUnknown)?;
        let total = self
            .nutrient(nutrient_code)
            .ok_or_else(|| Error::NutrientNotCalculated {
                code: nutrient_code.to_owned(),
            })?;
        let dividend = total
            .sum_of_products
            .checked_mul(i128::from(portion.get()))
            .ok_or(Error::Overflow)?;
        let divisor = i128::from(SUPPORTED_REFERENCE.get()) * i128::from(cooked_weight.get());
        to_amount(div_round(dividend, divisor))
    }
}

/// Reference quantity supported in phase 1 (D19).
const SUPPORTED_REFERENCE: Grams = Grams::HUNDRED;

/// Calculates the totals and per-serving values of `nutrients` for `recipe`.
///
/// Every ingredient must be in `foods` with a 100 g reference quantity. Missing
/// nutrient values are not an error: they contribute 0 and are listed (D8, C5).
///
/// # Arguments
///
/// * `recipe` - the recipe to calculate; it must have at least one ingredient.
/// * `foods` - catalog holding every food the recipe uses.
/// * `nutrients` - nutrients to calculate, with distinct codes; results keep this order.
pub fn calculate_recipe(
    recipe: &Recipe,
    foods: &FoodCatalog,
    nutrients: &[Nutrient],
) -> Result<RecipeComposition, Error> {
    if recipe.items().is_empty() {
        return Err(Error::EmptyRecipe);
    }

    let mut ingredients = Vec::with_capacity(recipe.items().len());
    let mut compositions = Vec::with_capacity(recipe.items().len());
    for (position, item) in recipe.items().iter().enumerate() {
        let food = foods.get(item.food_id).ok_or(Error::FoodNotFound {
            food_id: item.food_id,
        })?;
        let composition = food.composition(item.state);
        let reference = composition.reference_quantity();
        if reference != SUPPORTED_REFERENCE {
            return Err(Error::UnsupportedReferenceQuantity {
                food_id: food.id(),
                grams: reference.get(),
            });
        }
        ingredients.push(IngredientLine {
            position,
            food_id: food.id(),
            food_name: food.name().to_owned(),
            state: item.state,
            quantity: item.quantity,
            shared_composition: food.is_shared(),
        });
        compositions.push(composition);
    }

    for (i, nutrient) in nutrients.iter().enumerate() {
        if nutrients[..i].iter().any(|n| n.code() == nutrient.code()) {
            return Err(Error::DuplicateNutrient {
                code: nutrient.code().to_owned(),
            });
        }
    }

    let reference = i128::from(SUPPORTED_REFERENCE.get());
    let servings = i128::from(recipe.servings().get());
    let nutrients = nutrients
        .iter()
        .map(|nutrient| {
            let mut sum_of_products: i128 = 0;
            let mut unknown_items = Vec::new();
            for (position, (item, composition)) in
                recipe.items().iter().zip(&compositions).enumerate()
            {
                let value = composition.value(nutrient.code());
                if value == NutrientValue::Unknown {
                    unknown_items.push(position);
                }
                // i64 × u32 fits in 96 bits, so the sum cannot overflow i128.
                sum_of_products +=
                    i128::from(value.contribution()) * i128::from(item.quantity.get());
            }
            Ok(NutrientTotal {
                nutrient: nutrient.clone(),
                sum_of_products,
                total: to_amount(div_round(sum_of_products, reference))?,
                per_serving: to_amount(div_round(sum_of_products, reference * servings))?,
                unknown_items,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;

    Ok(RecipeComposition {
        recipe_name: recipe.name().to_owned(),
        servings: recipe.servings(),
        cooked_weight: recipe.cooked_weight(),
        ingredients,
        nutrients,
    })
}

/// Width in characters of the longest cell, or 0 when there is none.
///
/// # Arguments
///
/// * `cells` - text of each cell in the column.
fn column_width<'a>(cells: impl Iterator<Item = &'a str>) -> usize {
    cells.map(|c| c.chars().count()).max().unwrap_or(0)
}

/// Converts a rounded `i128` amount to an [`Amount`]; outside `i64` is
/// [`Error::Overflow`].
///
/// # Arguments
///
/// * `milli` - amount in thousandths of the canonical unit.
fn to_amount(milli: i128) -> Result<Amount, Error> {
    i64::try_from(milli)
        .map(Amount::from_milli)
        .map_err(|_| Error::Overflow)
}

impl fmt::Display for RecipeComposition {
    /// Prints the recipe header, the ingredients numbered from 1, the aligned
    /// nutrient table, and the unknown and shared-composition notes.
    ///
    /// # Arguments
    ///
    /// * `f` - formatter that receives the text.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({} servings", self.recipe_name, self.servings.get())?;
        if let Some(weight) = self.cooked_weight {
            write!(f, ", cooked weight {weight}")?;
        }
        writeln!(f, ")")?;

        writeln!(f, "Ingredients:")?;
        for line in &self.ingredients {
            let marker = if line.shared_composition { " *" } else { "" };
            writeln!(
                f,
                "  {}. {} ({}), {}{marker}",
                line.position + 1,
                line.food_name,
                line.state,
                line.quantity
            )?;
        }

        writeln!(f, "Nutrients (total / per serving):")?;
        let rows: Vec<(&NutrientTotal, String, String)> = self
            .nutrients
            .iter()
            .map(|t| {
                let unit = t.nutrient.unit();
                (
                    t,
                    t.total.display(unit).to_string(),
                    t.per_serving.display(unit).to_string(),
                )
            })
            .collect();
        let name_width = column_width(rows.iter().map(|r| r.0.nutrient.name()));
        let total_width = column_width(rows.iter().map(|r| r.1.as_str()));
        let serving_width = column_width(rows.iter().map(|r| r.2.as_str()));
        for (total, total_text, serving_text) in &rows {
            write!(
                f,
                "  {:name_width$}  {total_text:>total_width$} / {serving_text:>serving_width$}",
                total.nutrient.name(),
            )?;
            if !total.unknown_items.is_empty() {
                let items: Vec<String> = total
                    .unknown_items
                    .iter()
                    .map(|p| (p + 1).to_string())
                    .collect();
                write!(f, "  (unknown for ingredients {})", items.join(", "))?;
            }
            writeln!(f)?;
        }

        if self.ingredients.iter().any(|i| i.shared_composition) {
            writeln!(
                f,
                "* raw and cooked are not distinguished in the source; the same composition is used for both."
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::food::{Composition, Compositions, Food};
    use crate::domain::quantity::Unit;

    /// A one-ingredient recipe of `grams` of a food holding `milli` of nutrient `x`
    /// per 100 g.
    ///
    /// # Arguments
    ///
    /// * `milli` - amount of nutrient `x` per 100 g, in thousandths of a gram.
    /// * `grams` - quantity of the food in the recipe, in grams.
    fn single_food_recipe(milli: i64, grams: i64) -> (Recipe, FoodCatalog) {
        let mut foods = FoodCatalog::new();
        foods
            .insert(Food::new(
                FoodId(1),
                "f",
                Compositions::Shared(
                    Composition::per_100g().with("x", NutrientValue::known(milli).unwrap()),
                ),
            ))
            .unwrap();
        let mut recipe = Recipe::new("r", Servings::new(1).unwrap());
        recipe.add_ingredient(FoodId(1), State::Raw, Grams::new(grams).unwrap());
        (recipe, foods)
    }

    /// A total that does not fit in `i64` is an overflow error, not a wrapped value.
    #[test]
    fn total_above_i64_is_an_overflow_error() {
        // i64::MAX × 1000 g / 100 g does not fit in i64.
        let (recipe, foods) = single_food_recipe(i64::MAX, 1000);
        let x = Nutrient::new("x", "X", Unit::Gram);
        assert_eq!(
            calculate_recipe(&recipe, &foods, &[x]),
            Err(Error::Overflow)
        );
    }

    /// The same nutrient code twice in the requested list is an error.
    #[test]
    fn rejects_duplicate_nutrient_codes() {
        let (recipe, foods) = single_food_recipe(1, 100);
        let x = Nutrient::new("x", "X", Unit::Gram);
        assert_eq!(
            calculate_recipe(&recipe, &foods, &[x.clone(), x]),
            Err(Error::DuplicateNutrient { code: "x".into() })
        );
    }
}
