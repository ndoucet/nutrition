//! Shared fixtures for the recipe-calculation integration tests.
//!
//! SYNTHETIC TEST DATA (DECISIONS.md, D13): every food below is invented, with
//! round values chosen so that results can be checked by hand. None of these
//! values is real food composition data and none comes from CIQUAL.
//!
//! Amounts are fixed-point thousandths of the canonical unit per 100 g
//! (D1, D14): `31_000` in a gram nutrient means 31.000 g / 100 g.

#![allow(dead_code)]

use nutrition_core::{
    calculate_recipe, initial_nutrients, Composition, Compositions, Food, FoodCatalog, FoodId,
    Grams, Nutrient, NutrientValue, Recipe, RecipeComposition, Servings,
};

/// Distinct raw and cooked compositions.
/// raw:    protein 12.000 g, fat 1.500 g, vitamin C zero
/// cooked: protein  5.000 g, fat 0.600 g, vitamin C unknown (explicit)
pub const PASTA: FoodId = FoodId(1);
/// Shared composition (source does not distinguish raw/cooked).
/// protein 31.000 g (the CALCULATION.md §2 example), fat trace,
/// vitamin C missing record (= unknown, DATA_MODEL §4).
pub const BEAN: FoodId = FoodId(2);
/// Shared composition: fat 100.000 g, protein zero, vitamin C zero.
pub const OIL: FoodId = FoodId(3);
/// Shared composition: protein 12.345 g (the D11 rounding example).
pub const HALF: FoodId = FoodId(4);
/// Shared composition: protein 0.001 g (one thousandth, for rounding tests).
pub const TINY: FoodId = FoodId(5);
/// Distinct compositions, used for the shared/distinct list.
/// raw:    vitamin C 120.000 mg, protein below limit (< 0.500 g), fat trace
/// cooked: vitamin C  60.000 mg, protein below limit (< 0.500 g), fat trace
pub const PEPPER: FoodId = FoodId(6);
/// Shared composition with a 50 g reference quantity (unsupported, D19).
pub const ODD_REFERENCE: FoodId = FoodId(50);

/// A known fixture value of `milli` thousandths; panics on a negative amount.
///
/// # Arguments
///
/// * `milli` - amount in thousandths of the canonical unit.
pub fn known(milli: i64) -> NutrientValue {
    NutrientValue::known(milli).expect("non-negative fixture amount")
}

/// A fixture quantity of `g` grams; panics unless positive.
///
/// # Arguments
///
/// * `g` - quantity in grams.
pub fn grams(g: i64) -> Grams {
    Grams::new(g).expect("positive fixture quantity")
}

/// A fixture number of servings; panics unless positive.
///
/// # Arguments
///
/// * `s` - number of servings.
pub fn servings(s: i64) -> Servings {
    Servings::new(s).expect("positive fixture servings")
}

/// The catalog of all synthetic fixture foods described above.
pub fn catalog() -> FoodCatalog {
    let mut catalog = FoodCatalog::new();

    let pasta_raw = Composition::per_100g()
        .with("protein", known(12_000))
        .with("fat", known(1_500))
        .with("vitamin_c", NutrientValue::Zero);
    let pasta_cooked = Composition::per_100g()
        .with("protein", known(5_000))
        .with("fat", known(600))
        .with("vitamin_c", NutrientValue::Unknown);
    let foods = vec![
        Food::new(
            PASTA,
            "Synthetic pasta",
            Compositions::Distinct {
                raw: pasta_raw,
                cooked: pasta_cooked,
            },
        ),
        Food::new(
            BEAN,
            "Synthetic bean",
            Compositions::Shared(
                Composition::per_100g()
                    .with("protein", known(31_000))
                    .with("fat", NutrientValue::Trace),
            ),
        ),
        Food::new(
            OIL,
            "Synthetic oil",
            Compositions::Shared(
                Composition::per_100g()
                    .with("fat", known(100_000))
                    .with("protein", NutrientValue::Zero)
                    .with("vitamin_c", NutrientValue::Zero),
            ),
        ),
        Food::new(
            HALF,
            "Synthetic half",
            Compositions::Shared(Composition::per_100g().with("protein", known(12_345))),
        ),
        Food::new(
            TINY,
            "Synthetic tiny",
            Compositions::Shared(Composition::per_100g().with("protein", known(1))),
        ),
        Food::new(
            PEPPER,
            "Synthetic pepper",
            Compositions::Distinct {
                raw: Composition::per_100g()
                    .with("vitamin_c", known(120_000))
                    .with("protein", NutrientValue::below_limit(500).unwrap())
                    .with("fat", NutrientValue::Trace),
                cooked: Composition::per_100g()
                    .with("vitamin_c", known(60_000))
                    .with("protein", NutrientValue::below_limit(500).unwrap())
                    .with("fat", NutrientValue::Trace),
            },
        ),
        Food::new(
            ODD_REFERENCE,
            "Synthetic odd reference",
            Compositions::Shared(Composition::new(grams(50)).with("protein", known(10_000))),
        ),
    ];
    for food in foods {
        catalog.insert(food).expect("unique fixture ids");
    }
    catalog
}

/// The nutrients from the initial set (D14) with the given codes, in the given order.
///
/// # Arguments
///
/// * `codes` - nutrient codes from the initial set, e.g. `protein`.
pub fn nutrients(codes: &[&str]) -> Vec<Nutrient> {
    let all = initial_nutrients();
    codes
        .iter()
        .map(|code| {
            all.iter()
                .find(|n| n.code() == *code)
                .unwrap_or_else(|| panic!("{code} is in the initial set"))
                .clone()
        })
        .collect()
}

/// protein (g), fat (g), vitamin C (mg).
pub fn test_nutrients() -> Vec<Nutrient> {
    nutrients(&["protein", "fat", "vitamin_c"])
}

/// Calculates `recipe` against the fixture catalog and [`test_nutrients`];
/// panics on error.
///
/// # Arguments
///
/// * `recipe` - recipe that uses only fixture foods.
pub fn calculate(recipe: &Recipe) -> RecipeComposition {
    calculate_recipe(recipe, &catalog(), &test_nutrients()).expect("valid recipe")
}

/// Total (thousandths) of a nutrient in a result.
///
/// # Arguments
///
/// * `result` - result of a calculation.
/// * `code` - code of a calculated nutrient.
pub fn total(result: &RecipeComposition, code: &str) -> i64 {
    result
        .nutrient(code)
        .unwrap_or_else(|| panic!("{code} calculated"))
        .total()
        .milli()
}

/// Per-serving amount (thousandths) of a nutrient in a result.
///
/// # Arguments
///
/// * `result` - result of a calculation.
/// * `code` - code of a calculated nutrient.
pub fn per_serving(result: &RecipeComposition, code: &str) -> i64 {
    result
        .nutrient(code)
        .unwrap_or_else(|| panic!("{code} calculated"))
        .per_serving()
        .milli()
}

/// Positions of the ingredients with an unknown value for a nutrient.
///
/// # Arguments
///
/// * `result` - result of a calculation.
/// * `code` - code of a calculated nutrient.
pub fn unknown_items(result: &RecipeComposition, code: &str) -> Vec<usize> {
    result
        .nutrient(code)
        .unwrap_or_else(|| panic!("{code} calculated"))
        .unknown_items()
        .to_vec()
}
