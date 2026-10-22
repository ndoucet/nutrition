//! Builds a recipe from synthetic foods and prints its composition.
//!
//! The foods are illustrative, not real composition data (D13).
//!
//! Run with `cargo run -p nutrition-core --example print_recipe`.

use nutrition_core::{
    calculate_recipe, initial_nutrients, Composition, Compositions, Error, Food, FoodCatalog,
    FoodId, Grams, NutrientValue, Recipe, Servings, State,
};

/// Builds the demo recipe, then prints its composition and a 300 g portion of
/// protein.
fn main() -> Result<(), Error> {
    let pasta = FoodId(1);
    let oil = FoodId(2);
    let tomato = FoodId(3);

    let mut foods = FoodCatalog::new();
    foods.insert(Food::new(
        pasta,
        "Pasta",
        Compositions::Distinct {
            raw: Composition::per_100g()
                .with("energy_kcal", NutrientValue::known(350_000)?)
                .with("protein", NutrientValue::known(12_000)?)
                .with("fat", NutrientValue::known(1_500)?),
            cooked: Composition::per_100g()
                .with("energy_kcal", NutrientValue::known(150_000)?)
                .with("protein", NutrientValue::known(5_000)?)
                .with("fat", NutrientValue::known(600)?),
        },
    ))?;
    foods.insert(Food::new(
        oil,
        "Olive oil",
        Compositions::Shared(
            Composition::per_100g()
                .with("energy_kcal", NutrientValue::known(900_000)?)
                .with("protein", NutrientValue::Zero)
                .with("fat", NutrientValue::known(100_000)?),
        ),
    ))?;
    foods.insert(Food::new(
        tomato,
        "Tomato",
        Compositions::Shared(
            Composition::per_100g()
                .with("energy_kcal", NutrientValue::known(20_000)?)
                .with("protein", NutrientValue::known(900)?)
                .with("fat", NutrientValue::Trace)
                .with("vitamin_c", NutrientValue::known(14_000)?),
        ),
    ))?;

    let mut recipe = Recipe::new("Pasta with tomato", Servings::new(2)?);
    recipe.add_ingredient(pasta, State::Cooked, Grams::new(250)?);
    recipe.add_ingredient(oil, State::Raw, Grams::new(15)?);
    recipe.add_ingredient(tomato, State::Raw, Grams::new(200)?);
    let extra = recipe.add_ingredient(oil, State::Raw, Grams::new(5)?);
    recipe.remove_ingredient(extra)?;
    recipe.set_cooked_weight(Some(Grams::new(450)?));

    let result = calculate_recipe(&recipe, &foods, &initial_nutrients())?;
    print!("{result}");
    // Format with the nutrient's own unit, never a hard-coded one.
    if let Some(protein) = result.nutrient("protein") {
        let nutrient = protein.nutrient();
        let portion = result.portion(nutrient.code(), Grams::new(300)?)?;
        println!(
            "{} in a 300 g portion: {}",
            nutrient.name(),
            portion.display(nutrient.unit())
        );
    }
    Ok(())
}
