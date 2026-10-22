//! Phase 1 integration tests: recipe nutrient calculation.
//!
//! Expected values are derived by hand from docs/CALCULATION.md §2–4, §11–13 and
//! docs/DECISIONS.md (D1, D6–D9, D11–D20), not from the implementation.
//! All foods are synthetic (D13); see `common/mod.rs`.
//!
//! Notation: amounts are thousandths of the canonical unit. `S_p` is the
//! unrounded sum of products `sum(N_i × Q_i)`; every derived value divides
//! `S_p` once and rounds half away from zero (D11, D12).

mod common;

use common::*;
use nutrition_core::{
    calculate_recipe, initial_nutrients, Amount, Composition, Compositions, Error, Food,
    FoodCatalog, FoodId, Grams, Nutrient, NutrientValue, Recipe, RecipeItem, Servings, State, Unit,
};

/// A recipe of `s` servings from `(food, state, grams)` items, in order.
///
/// # Arguments
///
/// * `items` - ingredients as (food, state, quantity in grams), in recipe order.
/// * `s` - number of servings.
fn recipe(items: &[(FoodId, State, i64)], s: i64) -> Recipe {
    let mut recipe = Recipe::new("Synthetic recipe", servings(s));
    for &(food, state, g) in items {
        recipe.add_ingredient(food, state, grams(g));
    }
    recipe
}

// ---------------------------------------------------------------------------
// §2 Scaling by reference quantity
// ---------------------------------------------------------------------------

/// The CALCULATION.md §2 example: 31 g / 100 g at 150 g is 46.5 g.
#[test]
fn scaling_reproduces_spec_example_31g_per_100g_times_150g() {
    // CALCULATION.md §2: 31000 × 150 = 4_650_000; / 100 = 46_500 (46.5 g)
    let result = calculate(&recipe(&[(BEAN, State::Raw, 150)], 1));
    assert_eq!(total(&result, "protein"), 46_500);
    assert_eq!(
        result
            .nutrient("protein")
            .unwrap()
            .total()
            .display(Unit::Gram)
            .to_string(),
        "46.5 g"
    );
}

/// 100 g of a food gives its per-100 g value unchanged.
#[test]
fn scaling_100g_returns_the_per_100g_value() {
    // 31000 × 100 / 100 = 31000
    let result = calculate(&recipe(&[(BEAN, State::Raw, 100)], 1));
    assert_eq!(total(&result, "protein"), 31_000);
}

/// 1 g, the smallest quantity, scales to one hundredth of the value.
#[test]
fn scaling_one_gram_is_the_smallest_quantity() {
    // 31000 × 1 / 100 = 310
    let result = calculate(&recipe(&[(BEAN, State::Raw, 1)], 1));
    assert_eq!(total(&result, "protein"), 310);
}

/// A large quantity (5 kg) scales without loss.
#[test]
fn scaling_large_realistic_quantity() {
    // Oil fat 100.000 g / 100 g, 5 kg: 100_000 × 5000 / 100 = 5_000_000 (5000 g)
    let result = calculate(&recipe(&[(OIL, State::Raw, 5_000)], 1));
    assert_eq!(total(&result, "fat"), 5_000_000);
}

/// Scaling works for a milligram nutrient as for a gram one.
#[test]
fn scaling_works_in_milligram_nutrient() {
    // Pepper raw vitamin C 120.000 mg / 100 g, 25 g: 120_000 × 25 / 100 = 30_000 (30 mg)
    let result = calculate(&recipe(&[(PEPPER, State::Raw, 25)], 1));
    assert_eq!(total(&result, "vitamin_c"), 30_000);
}

// ---------------------------------------------------------------------------
// §2, §12, D11 Rounding
// ---------------------------------------------------------------------------

/// An exact half rounds away from zero, not down or to even (D11).
#[test]
fn rounding_exact_half_goes_away_from_zero() {
    // D11: 12345 × 10 / 100 = 1234.5 → 1235 (truncation and banker's give 1234)
    let result = calculate(&recipe(&[(HALF, State::Raw, 10)], 1));
    assert_eq!(total(&result, "protein"), 1_235);
}

/// Below half rounds down (D11).
#[test]
fn rounding_below_half_goes_down() {
    // D11: 12345 × 7 / 100 = 864.15 → 864
    let result = calculate(&recipe(&[(HALF, State::Raw, 7)], 1));
    assert_eq!(total(&result, "protein"), 864);
}

/// Amounts below one thousandth round to 1 at half and to 0 below it.
#[test]
fn rounding_of_sub_thousandth_amounts() {
    // 1 × 50 / 100 = 0.5 → 1 ; 1 × 49 / 100 = 0.49 → 0
    let half = calculate(&recipe(&[(TINY, State::Raw, 50)], 1));
    assert_eq!(total(&half, "protein"), 1);
    let below = calculate(&recipe(&[(TINY, State::Raw, 49)], 1));
    assert_eq!(total(&below, "protein"), 0);
}

// ---------------------------------------------------------------------------
// §3 Multi-ingredient summation, single final division
// ---------------------------------------------------------------------------

/// Ingredients are summed before the single division, not rounded one by one
/// (D12).
#[test]
fn summation_divides_once_not_per_ingredient() {
    // S_p = 12345 × 10 + 1 × 50 = 123_450 + 50 = 123_500; / 100 = 1235
    // Per-ingredient rounding would give round(1234.5) + round(0.5) = 1235 + 1 = 1236.
    let result = calculate(&recipe(
        &[(HALF, State::Raw, 10), (TINY, State::Raw, 50)],
        1,
    ));
    assert_eq!(total(&result, "protein"), 1_235);
}

/// The same food twice is two items, summed before the single division.
#[test]
fn summation_same_food_twice_divides_once() {
    // D17: the same food may appear twice.
    // S_p = 123_450 + 123_450 = 246_900; / 100 = 2469
    // Per-ingredient rounding would give 1235 + 1235 = 2470.
    let result = calculate(&recipe(
        &[(HALF, State::Raw, 10), (HALF, State::Raw, 10)],
        1,
    ));
    assert_eq!(total(&result, "protein"), 2_469);
    assert_eq!(result.ingredients().len(), 2);
    assert_eq!(result.ingredients()[0].position, 0);
    assert_eq!(result.ingredients()[1].position, 1);
}

/// Each nutrient is summed over items in different states independently.
#[test]
fn summation_over_several_nutrients_and_states() {
    // pasta raw 200 g, bean 100 g, oil 10 g
    // protein: 12000×200 + 31000×100 + zero = 2_400_000 + 3_100_000 = 5_500_000 → 55_000
    // fat:     1500×200 + trace + 100000×10 = 300_000 + 0 + 1_000_000 = 1_300_000 → 13_000
    // vit C:   zero + unknown(bean, missing record) + zero → 0, unknown at [1]
    let result = calculate(&recipe(
        &[
            (PASTA, State::Raw, 200),
            (BEAN, State::Raw, 100),
            (OIL, State::Raw, 10),
        ],
        1,
    ));
    assert_eq!(total(&result, "protein"), 55_000);
    assert_eq!(total(&result, "fat"), 13_000);
    assert_eq!(total(&result, "vitamin_c"), 0);
    assert_eq!(unknown_items(&result, "vitamin_c"), vec![1]);
    assert!(unknown_items(&result, "protein").is_empty());
    assert!(unknown_items(&result, "fat").is_empty());
}

/// Totals come out in the order of the requested nutrient list.
#[test]
fn results_follow_input_nutrient_order() {
    let order = nutrients(&["vitamin_c", "protein", "fat"]);
    let result =
        calculate_recipe(&recipe(&[(BEAN, State::Raw, 100)], 1), &catalog(), &order).unwrap();
    let codes: Vec<&str> = result
        .nutrients()
        .iter()
        .map(|t| t.nutrient().code())
        .collect();
    assert_eq!(codes, vec!["vitamin_c", "protein", "fat"]);
}

/// A nutrient that was not requested has no total.
#[test]
fn nutrient_not_requested_is_absent_from_result() {
    let result = calculate(&recipe(&[(BEAN, State::Raw, 100)], 1));
    assert!(result.nutrient("iron").is_none());
}

/// All 14 initial nutrients are calculated; those without records are
/// unknown.
#[test]
fn full_initial_nutrient_set_is_calculated() {
    // 14 nutrients; bean has only protein (known) and fat (trace); all others unknown.
    let all = initial_nutrients();
    let result =
        calculate_recipe(&recipe(&[(BEAN, State::Raw, 100)], 1), &catalog(), &all).unwrap();
    assert_eq!(result.nutrients().len(), 14);
    for (t, n) in result.nutrients().iter().zip(&all) {
        assert_eq!(t.nutrient(), n);
    }
    assert_eq!(total(&result, "protein"), 31_000);
    assert_eq!(total(&result, "energy_kcal"), 0);
    assert_eq!(unknown_items(&result, "energy_kcal"), vec![0]);
    assert!(unknown_items(&result, "fat").is_empty());
}

// ---------------------------------------------------------------------------
// §4, D12 Per-serving values
// ---------------------------------------------------------------------------

/// The per-serving value divides the unrounded sum, not the rounded total
/// (D12).
#[test]
fn per_serving_divides_the_unrounded_sum_once() {
    // S_p = 123_450; total = 1235.
    // per serving (S = 2) = 123_450 / (100 × 2) = 617.25 → 617
    // (from the rounded total: 1235 / 2 = 617.5 → 618, which D12 forbids)
    let result = calculate(&recipe(&[(HALF, State::Raw, 10)], 2));
    assert_eq!(total(&result, "protein"), 1_235);
    assert_eq!(per_serving(&result, "protein"), 617);
}

/// The per-serving value rounds half away from zero.
#[test]
fn per_serving_rounds_half_away_from_zero() {
    // bean 3 g: S_p = 93_000; S = 4 → 93_000 / 400 = 232.5 → 233
    let result = calculate(&recipe(&[(BEAN, State::Raw, 3)], 4));
    assert_eq!(per_serving(&result, "protein"), 233);
}

/// With one serving, the per-serving value equals the total.
#[test]
fn one_serving_equals_total() {
    let result = calculate(&recipe(&[(BEAN, State::Raw, 150)], 1));
    assert_eq!(per_serving(&result, "protein"), 46_500);
    assert_eq!(result.servings(), servings(1));
}

/// Changing the servings changes the per-serving value but not the total.
#[test]
fn set_servings_changes_per_serving_value() {
    // 4_650_000 / (100 × 3) = 15_500
    let mut r = recipe(&[(BEAN, State::Raw, 150)], 1);
    r.set_servings(servings(3));
    let result = calculate(&r);
    assert_eq!(result.servings(), servings(3));
    assert_eq!(total(&result, "protein"), 46_500);
    assert_eq!(per_serving(&result, "protein"), 15_500);
}

// ---------------------------------------------------------------------------
// §4, D12 Per-portion values
// ---------------------------------------------------------------------------

/// A portion divides the unrounded sum, not the rounded total (D12).
#[test]
fn portion_divides_the_unrounded_sum_once() {
    // S_p = 123_450, W = 300, P = 150
    // 123_450 × 150 / (100 × 300) = 18_517_500 / 30_000 = 617.25 → 617
    // (from the rounded total: 1235 × 150 / 300 = 617.5 → 618)
    let mut r = recipe(&[(HALF, State::Raw, 10)], 1);
    r.set_cooked_weight(Some(grams(300)));
    let result = calculate(&r);
    assert_eq!(
        result.portion("protein", grams(150)).unwrap(),
        Amount::from_milli(617)
    );
}

/// A portion value rounds half away from zero.
#[test]
fn portion_rounds_half_away_from_zero() {
    // S_p = 3_100_000, W = 400, P = 1 → 3_100_000 / 40_000 = 77.5 → 78
    let mut r = recipe(&[(BEAN, State::Raw, 100)], 1);
    r.set_cooked_weight(Some(grams(400)));
    let result = calculate(&r);
    assert_eq!(
        result.portion("protein", grams(1)).unwrap(),
        Amount::from_milli(78)
    );
}

/// A portion equal to the cooked weight is the whole total.
#[test]
fn portion_equal_to_cooked_weight_is_the_total() {
    // 4_650_000 × 250 / (100 × 250) = 46_500
    let mut r = recipe(&[(BEAN, State::Raw, 150)], 1);
    r.set_cooked_weight(Some(grams(250)));
    let result = calculate(&r);
    assert_eq!(
        result.portion("protein", grams(250)).unwrap(),
        Amount::from_milli(46_500)
    );
}

/// A portion needs a known cooked weight.
#[test]
fn portion_without_cooked_weight_is_an_error() {
    let result = calculate(&recipe(&[(BEAN, State::Raw, 150)], 1));
    assert_eq!(
        result.portion("protein", grams(100)),
        Err(Error::CookedWeightUnknown)
    );
}

/// Clearing the cooked weight makes it unknown again.
#[test]
fn portion_after_clearing_cooked_weight_is_an_error() {
    let mut r = recipe(&[(BEAN, State::Raw, 150)], 1);
    r.set_cooked_weight(Some(grams(300)));
    r.set_cooked_weight(None);
    let result = calculate(&r);
    assert_eq!(
        result.portion("protein", grams(100)),
        Err(Error::CookedWeightUnknown)
    );
}

/// A portion of a nutrient absent from the result is an error.
#[test]
fn portion_of_nutrient_not_calculated_is_an_error() {
    let mut r = recipe(&[(BEAN, State::Raw, 150)], 1);
    r.set_cooked_weight(Some(grams(300)));
    let result = calculate(&r);
    assert_eq!(
        result.portion("iron", grams(100)),
        Err(Error::NutrientNotCalculated {
            code: "iron".to_string()
        })
    );
}

// ---------------------------------------------------------------------------
// §3, §11, D8 Value states
// ---------------------------------------------------------------------------

const KNOWN_MILLI: i64 = 2_000; // 2.000 g / 100 g; at 50 g contributes 2000 × 50 / 100 = 1000
const STATE_GRAMS: i64 = 50;

/// One value of each state, named for assertion messages; `known` comes first
/// and `unknown` last.
fn states() -> [(&'static str, NutrientValue); 5] {
    [
        ("known", known(KNOWN_MILLI)),
        ("zero", NutrientValue::Zero),
        ("trace", NutrientValue::Trace),
        // limit 0.500 g must NOT contribute (would add 500 × 50 / 100 = 250)
        ("below_limit", NutrientValue::below_limit(500).unwrap()),
        ("unknown", NutrientValue::Unknown),
    ]
}

/// A catalog of shared-composition foods with ids from 100, one per value,
/// each holding that value for protein.
///
/// # Arguments
///
/// * `values` - protein value of each food, per 100 g; amounts are in thousandths of a gram.
fn state_catalog(values: &[NutrientValue]) -> FoodCatalog {
    let mut catalog = FoodCatalog::new();
    for (i, v) in values.iter().enumerate() {
        catalog
            .insert(Food::new(
                FoodId(100 + i as i64),
                format!("Synthetic state food {i}"),
                Compositions::Shared(Composition::per_100g().with("protein", *v)),
            ))
            .unwrap();
    }
    catalog
}

/// A one-serving recipe of 50 g of each of the first `n` state foods.
///
/// # Arguments
///
/// * `n` - number of state foods to use, from id 100.
fn state_recipe(n: usize) -> Recipe {
    let mut r = Recipe::new("States", servings(1));
    for i in 0..n {
        r.add_ingredient(FoodId(100 + i as i64), State::Raw, grams(STATE_GRAMS));
    }
    r
}

/// Only a known value contributes, and only unknown is listed (D8).
#[test]
fn each_value_state_alone() {
    // known → 1000; zero, trace, below_limit, unknown → 0 (D8, initial phase).
    // Only unknown is listed as an unknown item.
    let expected: [(i64, Vec<usize>); 5] = [
        (1_000, vec![]),
        (0, vec![]),
        (0, vec![]),
        (0, vec![]),
        (0, vec![0]),
    ];
    for ((name, value), (exp_total, exp_unknown)) in states().into_iter().zip(expected) {
        let result = calculate_recipe(
            &state_recipe(1),
            &state_catalog(&[value]),
            &nutrients(&["protein"]),
        )
        .unwrap();
        assert_eq!(total(&result, "protein"), exp_total, "state {name}");
        assert_eq!(per_serving(&result, "protein"), exp_total, "state {name}");
        assert_eq!(
            unknown_items(&result, "protein"),
            exp_unknown,
            "state {name}"
        );
    }
}

/// Contributions and unknown positions combine correctly for every pair of
/// states.
#[test]
fn every_pair_of_value_states() {
    // Each ingredient 50 g; total = 1000 × (number of known items),
    // unknown_items = positions holding the unknown state.
    let s = states();
    for (i, (name_a, a)) in s.iter().enumerate() {
        for (j, (name_b, b)) in s.iter().enumerate() {
            let result = calculate_recipe(
                &state_recipe(2),
                &state_catalog(&[*a, *b]),
                &nutrients(&["protein"]),
            )
            .unwrap();
            let known_count = [i, j].iter().filter(|&&k| k == 0).count() as i64;
            let mut exp_unknown = vec![];
            if i == 4 {
                exp_unknown.push(0);
            }
            if j == 4 {
                exp_unknown.push(1);
            }
            assert_eq!(
                total(&result, "protein"),
                1_000 * known_count,
                "pair ({name_a}, {name_b})"
            );
            assert_eq!(
                unknown_items(&result, "protein"),
                exp_unknown,
                "pair ({name_a}, {name_b})"
            );
        }
    }
}

/// A missing record is unknown and listed, not treated as zero.
#[test]
fn missing_record_is_unknown_not_zero() {
    // DATA_MODEL §4: a missing record is equivalent to unknown.
    let composition = Composition::per_100g();
    assert_eq!(composition.value("protein"), NutrientValue::Unknown);
    assert_eq!(composition.reference_quantity(), Grams::HUNDRED);
    // bean has no vitamin C record → listed as unknown
    let result = calculate(&recipe(&[(BEAN, State::Raw, 100)], 1));
    assert_eq!(unknown_items(&result, "vitamin_c"), vec![0]);
}

/// A known 0 becomes the `Zero` state (D18).
#[test]
fn known_zero_is_normalized_to_zero_state() {
    // D18
    assert_eq!(NutrientValue::known(0), Ok(NutrientValue::Zero));
    assert!(matches!(NutrientValue::known(5), Ok(NutrientValue::Known(a)) if a.milli() == 5));
}

/// Negative known amounts and limits are rejected (D18).
#[test]
fn negative_amounts_are_rejected() {
    // D18
    assert_eq!(
        NutrientValue::known(-1),
        Err(Error::NegativeAmount { milli: -1 })
    );
    assert_eq!(
        NutrientValue::below_limit(-10),
        Err(Error::NegativeAmount { milli: -10 })
    );
}

// ---------------------------------------------------------------------------
// §11, D8 Unknown-items list
// ---------------------------------------------------------------------------

/// Every unknown position is listed, whether explicit or a missing record,
/// including a repeated food.
#[test]
fn unknown_items_lists_every_position_per_nutrient() {
    // 0: pasta cooked (vit C explicit unknown), 1: bean (vit C missing),
    // 2: oil (vit C zero), 3: bean again (vit C missing)
    let result = calculate(&recipe(
        &[
            (PASTA, State::Cooked, 100),
            (BEAN, State::Raw, 100),
            (OIL, State::Raw, 10),
            (BEAN, State::Raw, 50),
        ],
        1,
    ));
    assert_eq!(unknown_items(&result, "vitamin_c"), vec![0, 1, 3]);
    assert!(unknown_items(&result, "protein").is_empty());
    // fat: 600×100 + trace + 100000×10 + trace = 60_000 + 1_000_000 = 1_060_000 → 10_600
    assert!(unknown_items(&result, "fat").is_empty());
    assert_eq!(total(&result, "fat"), 10_600);
}

// ---------------------------------------------------------------------------
// §2, D9 Raw / cooked selection and shared fallback
// ---------------------------------------------------------------------------

/// A raw item uses the raw composition.
#[test]
fn raw_state_uses_raw_composition() {
    // 12000 × 200 / 100 = 24_000
    let result = calculate(&recipe(&[(PASTA, State::Raw, 200)], 1));
    assert_eq!(total(&result, "protein"), 24_000);
    assert!(unknown_items(&result, "vitamin_c").is_empty()); // raw vit C is zero
}

/// A cooked item uses the cooked composition.
#[test]
fn cooked_state_uses_cooked_composition() {
    // 5000 × 200 / 100 = 10_000
    let result = calculate(&recipe(&[(PASTA, State::Cooked, 200)], 1));
    assert_eq!(total(&result, "protein"), 10_000);
    assert_eq!(unknown_items(&result, "vitamin_c"), vec![0]); // cooked vit C unknown
}

/// One food can appear both raw and cooked in a recipe (D17).
#[test]
fn same_food_raw_and_cooked_in_one_recipe() {
    // D17: 12000 × 100 + 5000 × 100 = 1_700_000 → 17_000
    let result = calculate(&recipe(
        &[(PASTA, State::Raw, 100), (PASTA, State::Cooked, 100)],
        1,
    ));
    assert_eq!(total(&result, "protein"), 17_000);
    assert_eq!(result.ingredients()[0].state, State::Raw);
    assert_eq!(result.ingredients()[1].state, State::Cooked);
    assert!(result.shared_composition_items().is_empty());
}

/// A shared composition gives the same values in both states; a distinct one
/// does not.
#[test]
fn shared_food_uses_same_composition_for_both_states() {
    // bean cooked 150 g → same 46_500 as raw
    let result = calculate(&recipe(&[(BEAN, State::Cooked, 150)], 1));
    assert_eq!(total(&result, "protein"), 46_500);
    let bean = catalog();
    let bean = bean.get(BEAN).unwrap();
    assert!(bean.is_shared());
    assert_eq!(
        bean.composition(State::Raw),
        bean.composition(State::Cooked)
    );
    let catalog = catalog();
    let pasta = catalog.get(PASTA).unwrap();
    assert!(!pasta.is_shared());
    assert_ne!(
        pasta.composition(State::Raw),
        pasta.composition(State::Cooked)
    );
}

/// Ingredients whose food has a shared composition are listed by position.
#[test]
fn shared_composition_items_lists_shared_foods_by_position() {
    // ASSUMPTION (DATA_MODEL §3): every ingredient whose food has a shared
    // composition is listed, whichever state was selected.
    // 0 pasta raw (distinct), 1 bean cooked (shared), 2 pepper cooked (distinct), 3 oil raw (shared)
    let result = calculate(&recipe(
        &[
            (PASTA, State::Raw, 100),
            (BEAN, State::Cooked, 100),
            (PEPPER, State::Cooked, 100),
            (OIL, State::Raw, 10),
        ],
        1,
    ));
    assert_eq!(result.shared_composition_items(), vec![1, 3]);
    let flags: Vec<bool> = result
        .ingredients()
        .iter()
        .map(|l| l.shared_composition)
        .collect();
    assert_eq!(flags, vec![false, true, false, true]);
    // pepper cooked vit C 60_000 × 100 / 100 = 60_000 (not the raw 120_000)
    assert_eq!(total(&result, "vitamin_c"), 60_000);
}

/// Ingredient lines carry position, food, name, state, quantity, and the
/// shared flag.
#[test]
fn ingredient_lines_describe_the_recipe() {
    let result = calculate(&recipe(
        &[(PASTA, State::Cooked, 200), (BEAN, State::Raw, 150)],
        1,
    ));
    let lines = result.ingredients();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].position, 0);
    assert_eq!(lines[0].food_id, PASTA);
    assert_eq!(lines[0].food_name, "Synthetic pasta");
    assert_eq!(lines[0].state, State::Cooked);
    assert_eq!(lines[0].quantity, grams(200));
    assert!(!lines[0].shared_composition);
    assert_eq!(lines[1].position, 1);
    assert_eq!(lines[1].food_id, BEAN);
    assert_eq!(lines[1].quantity, grams(150));
    assert!(lines[1].shared_composition);
}

// ---------------------------------------------------------------------------
// D17 Removal by position
// ---------------------------------------------------------------------------

/// Adding an ingredient returns its 0-based position.
#[test]
fn add_ingredient_returns_its_position() {
    let mut r = Recipe::new("Positions", servings(1));
    assert_eq!(r.add_ingredient(BEAN, State::Raw, grams(10)), 0);
    assert_eq!(r.add_ingredient(BEAN, State::Raw, grams(20)), 1);
    assert_eq!(r.add_ingredient(OIL, State::Raw, grams(5)), 2);
    assert_eq!(r.items().len(), 3);
}

/// Removing an ingredient returns it, shifts later positions, and removes its
/// contribution.
#[test]
fn remove_ingredient_by_position_shifts_later_positions() {
    // before: 0 bean 100, 1 oil 10, 2 pasta cooked 100; remove 1
    // after:  0 bean 100, 1 pasta cooked 100
    // protein: 31000×100 + 5000×100 = 3_600_000 → 36_000
    // fat:     trace + 600×100 = 60_000 → 600 (oil no longer counted)
    // vit C unknown: bean (0) and pasta cooked (1)
    let mut r = recipe(
        &[
            (BEAN, State::Raw, 100),
            (OIL, State::Raw, 10),
            (PASTA, State::Cooked, 100),
        ],
        1,
    );
    let removed = r.remove_ingredient(1).unwrap();
    assert_eq!(
        removed,
        RecipeItem {
            food_id: OIL,
            state: State::Raw,
            quantity: grams(10)
        }
    );
    assert_eq!(r.items().len(), 2);
    let result = calculate(&r);
    assert_eq!(total(&result, "protein"), 36_000);
    assert_eq!(total(&result, "fat"), 600);
    assert_eq!(unknown_items(&result, "vitamin_c"), vec![0, 1]);
    let ids: Vec<FoodId> = result.ingredients().iter().map(|l| l.food_id).collect();
    assert_eq!(ids, vec![BEAN, PASTA]);
    assert_eq!(result.ingredients()[1].position, 1);
}

/// Removing one of two identical items keeps the other.
#[test]
fn remove_one_of_two_identical_items() {
    // HALF 10 g twice, remove 0 → 1235 left
    let mut r = recipe(&[(HALF, State::Raw, 10), (HALF, State::Raw, 10)], 1);
    r.remove_ingredient(0).unwrap();
    assert_eq!(total(&calculate(&r), "protein"), 1_235);
}

/// Removing past the end is an error and leaves the recipe unchanged.
#[test]
fn remove_out_of_range_is_an_error() {
    let mut r = recipe(&[(BEAN, State::Raw, 1), (OIL, State::Raw, 1)], 1);
    assert_eq!(
        r.remove_ingredient(2),
        Err(Error::IngredientIndexOutOfRange { index: 2, len: 2 })
    );
    assert_eq!(r.items().len(), 2);
    let mut empty = Recipe::new("Empty", servings(1));
    assert_eq!(
        empty.remove_ingredient(0),
        Err(Error::IngredientIndexOutOfRange { index: 0, len: 0 })
    );
}

/// A recipe left with no ingredient is an empty-recipe error.
#[test]
fn removing_last_ingredient_leaves_empty_recipe() {
    let mut r = recipe(&[(BEAN, State::Raw, 100)], 1);
    r.remove_ingredient(0).unwrap();
    assert_eq!(
        calculate_recipe(&r, &catalog(), &test_nutrients()),
        Err(Error::EmptyRecipe)
    );
}

// ---------------------------------------------------------------------------
// Errors (§4, §12 "validate inputs", D6, D19, DATA_MODEL §11)
// ---------------------------------------------------------------------------

/// An ingredient whose food is not in the catalog is an error.
#[test]
fn unknown_food_is_an_error() {
    let r = recipe(&[(BEAN, State::Raw, 100), (FoodId(99), State::Raw, 10)], 1);
    assert_eq!(
        calculate_recipe(&r, &catalog(), &test_nutrients()),
        Err(Error::FoodNotFound {
            food_id: FoodId(99)
        })
    );
}

/// A recipe with no ingredient cannot be calculated.
#[test]
fn empty_recipe_is_an_error() {
    let r = Recipe::new("Empty", servings(1));
    assert_eq!(
        calculate_recipe(&r, &catalog(), &test_nutrients()),
        Err(Error::EmptyRecipe)
    );
}

/// A reference quantity other than 100 g is rejected (D19).
#[test]
fn non_100g_reference_quantity_is_an_error() {
    // D19
    let r = recipe(&[(ODD_REFERENCE, State::Raw, 100)], 1);
    assert_eq!(
        calculate_recipe(&r, &catalog(), &test_nutrients()),
        Err(Error::UnsupportedReferenceQuantity {
            food_id: ODD_REFERENCE,
            grams: 50
        })
    );
}

/// Quantities must be positive whole grams (D6).
#[test]
fn zero_and_negative_quantities_are_rejected() {
    assert_eq!(Grams::new(0), Err(Error::InvalidQuantity { grams: 0 }));
    assert_eq!(Grams::new(-1), Err(Error::InvalidQuantity { grams: -1 }));
    assert_eq!(
        Grams::new(-250),
        Err(Error::InvalidQuantity { grams: -250 })
    );
    assert_eq!(Grams::new(1).map(Grams::get), Ok(1));
    assert_eq!(Grams::HUNDRED.get(), 100);
}

/// Servings must be positive.
#[test]
fn zero_and_negative_servings_are_rejected() {
    assert_eq!(
        Servings::new(0),
        Err(Error::InvalidServings { servings: 0 })
    );
    assert_eq!(
        Servings::new(-3),
        Err(Error::InvalidServings { servings: -3 })
    );
    assert_eq!(Servings::new(1).map(Servings::get), Ok(1));
}

/// Inserting a duplicate food id is an error and keeps the original food.
#[test]
fn duplicate_food_in_catalog_is_an_error() {
    let mut c = catalog();
    let dup = Food::new(
        BEAN,
        "Another bean",
        Compositions::Shared(Composition::per_100g()),
    );
    assert_eq!(c.insert(dup), Err(Error::DuplicateFood { food_id: BEAN }));
    assert_eq!(
        c.get(BEAN)
            .unwrap()
            .composition(State::Raw)
            .value("protein"),
        known(31_000)
    );
}

/// A food without any nutrient record calculates, with every value unknown
/// (C5).
#[test]
fn incomplete_data_is_not_an_error() {
    // C5: a food with no nutrient record at all still calculates.
    let mut c = FoodCatalog::new();
    c.insert(Food::new(
        FoodId(7),
        "Synthetic blank",
        Compositions::Shared(Composition::per_100g()),
    ))
    .unwrap();
    let r = recipe(&[(FoodId(7), State::Cooked, 100)], 1);
    let result = calculate_recipe(&r, &c, &test_nutrients()).unwrap();
    for t in result.nutrients() {
        assert_eq!(t.total(), Amount::ZERO);
        assert_eq!(t.unknown_items(), &[0]);
    }
}

// ---------------------------------------------------------------------------
// D14, D16, D20 Nutrients, units and display precision
// ---------------------------------------------------------------------------

/// The initial set has the D14 codes and units, each with a name.
#[test]
fn initial_nutrients_match_d14() {
    let expected = [
        ("energy_kj", Unit::Kilojoule),
        ("energy_kcal", Unit::Kilocalorie),
        ("protein", Unit::Gram),
        ("carbohydrate_available", Unit::Gram),
        ("sugars", Unit::Gram),
        ("fibre", Unit::Gram),
        ("fat", Unit::Gram),
        ("fat_saturated", Unit::Gram),
        ("salt", Unit::Gram),
        ("calcium", Unit::Milligram),
        ("iron", Unit::Milligram),
        ("vitamin_c", Unit::Milligram),
        ("vitamin_d", Unit::Microgram),
        ("vitamin_b12", Unit::Microgram),
    ];
    let all = initial_nutrients();
    assert_eq!(all.len(), 14);
    for (code, unit) in expected {
        let n = all
            .iter()
            .find(|n| n.code() == code)
            .unwrap_or_else(|| panic!("{code} missing from initial set"));
        assert_eq!(n.unit(), unit, "{code}");
        assert!(!n.name().is_empty(), "{code}");
    }
}

/// A nutrient defined by the caller is calculated like any other (D20).
#[test]
fn custom_nutrient_is_data() {
    // D20: a nutrient not in the initial set can be calculated.
    let custom = vec![Nutrient::new("protein", "Custom protein", Unit::Gram)];
    let result =
        calculate_recipe(&recipe(&[(BEAN, State::Raw, 150)], 1), &catalog(), &custom).unwrap();
    assert_eq!(result.nutrients()[0].nutrient().name(), "Custom protein");
    assert_eq!(total(&result, "protein"), 46_500);
}

/// Unit symbols and display decimals follow D16.
#[test]
fn unit_symbols_and_display_decimals_match_d16() {
    assert_eq!(Unit::Gram.symbol(), "g");
    assert_eq!(Unit::Milligram.symbol(), "mg");
    assert_eq!(Unit::Microgram.symbol(), "\u{00b5}g");
    assert_eq!(Unit::Kilojoule.symbol(), "kJ");
    assert_eq!(Unit::Kilocalorie.symbol(), "kcal");
    assert_eq!(Unit::Gram.display_decimals(), 1);
    assert_eq!(Unit::Milligram.display_decimals(), 1);
    assert_eq!(Unit::Microgram.display_decimals(), 2);
    assert_eq!(Unit::Kilojoule.display_decimals(), 0);
    assert_eq!(Unit::Kilocalorie.display_decimals(), 0);
}

/// Display rounds half away from zero at each unit's precision (D16).
#[test]
fn amount_display_rounds_half_away_from_zero() {
    let show = |milli: i64, unit: Unit| Amount::from_milli(milli).display(unit).to_string();
    assert_eq!(show(46_500, Unit::Gram), "46.5 g");
    assert_eq!(show(1_250, Unit::Gram), "1.3 g"); // 1.25 → 1.3
    assert_eq!(show(1_249, Unit::Gram), "1.2 g"); // 1.249 → 1.2
    assert_eq!(show(1_240, Unit::Milligram), "1.2 mg");
    assert_eq!(show(1_005, Unit::Microgram), "1.01 \u{00b5}g"); // 1.005 → 1.01
    assert_eq!(show(1_500, Unit::Kilocalorie), "2 kcal"); // 1.5 → 2
    assert_eq!(show(2_499, Unit::Kilojoule), "2 kJ");
    assert_eq!(show(0, Unit::Gram), "0.0 g");
    assert_eq!(Amount::ZERO.milli(), 0);
}

// ---------------------------------------------------------------------------
// D15 Display smoke test
// ---------------------------------------------------------------------------

/// The display prints the recipe line by line, numbering ingredients from 1.
#[test]
fn display_prints_recipe_line_by_line() {
    // bean 300 g raw (pos 0), oil 40 g raw (pos 1), 2 servings
    // protein: 31000 × 300 = 9_300_000 → 93_000 "93.0 g"; per serving /200 → 46_500 "46.5 g"
    // fat: trace + 100000 × 40 = 4_000_000 → 40_000 "40.0 g"; per serving → 20_000 "20.0 g"
    let mut r = Recipe::new("Synthetic bean salad", servings(2));
    r.add_ingredient(BEAN, State::Raw, grams(300));
    r.add_ingredient(OIL, State::Raw, grams(40));
    let result = calculate(&r);
    let text = result.to_string();

    assert!(text.lines().count() > 3, "printed line by line:\n{text}");
    assert!(text.contains("Synthetic bean salad"), "{text}");
    for expected in ["93.0 g", "46.5 g", "40.0 g", "20.0 g"] {
        assert!(text.contains(expected), "missing {expected:?} in:\n{text}");
    }
    let protein_name = result.nutrient("protein").unwrap().nutrient().name();
    assert!(text.contains(protein_name), "{text}");

    // Ingredients are numbered from 1 in the display (positions are 0-based in the API).
    let bean_line = text
        .lines()
        .find(|l| l.contains("Synthetic bean") && !l.contains("salad"))
        .unwrap_or_else(|| panic!("bean line in:\n{text}"));
    assert!(bean_line.contains('1'), "bean numbered 1: {bean_line:?}");
    let oil_line = text
        .lines()
        .find(|l| l.contains("Synthetic oil"))
        .unwrap_or_else(|| panic!("oil line in:\n{text}"));
    assert!(oil_line.contains('2'), "oil numbered 2: {oil_line:?}");
}
