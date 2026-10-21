# Nutritional Calculations

## 1. Purpose

This document defines the calculation rules used to estimate meal composition and daily nutritional requirements.

Every calculation must specify:

- Input quantities and units.
- The equation or method.
- Required inputs.
- Assumptions.
- Source or reference.
- Limitations.
- Rounding and missing-data behavior.

## 2. Food nutrient scaling

Let:

- `N` be the amount of a nutrient in the reference quantity.
- `R` be the reference food quantity (g).
- `Q` be the quantity used in the recipe (g), in the selected state (raw or cooked).

Then:

```
nutrient_amount = N * Q / R
```

For the default reference quantity of 100 g:

```
nutrient_amount = nutrient_per_100g * quantity_g / 100
```

Example: a food contains 31 g of protein per 100 g, and a recipe uses 150 g.

```
protein = 31 * 150 / 100
        = 46.5 g
```

This example is illustrative, not an authoritative composition record.

### Integer arithmetic

Amounts are stored as fixed-point integers with 3 decimals (31 g → `31000`), and quantities as integer grams. To avoid rounding on each ingredient, the engine sums the products first and divides once:

```
total = sum(N_i * Q_i) / R        # i128 sum, i64 result; R = 100 for every food
```

In the example: `31000 * 150 = 4_650_000`, divided by 100 → `46500`, i.e. 46.500 g.

When all foods share the same reference quantity, this is exact until the final division. If reference quantities differ, scale each product to a common denominator before summing. Phase 1 supports only R = 100 g (D19).

The final division rounds half away from zero (D11). Products are computed in `i128`.

### Raw and cooked

Each recipe item specifies a state (raw or cooked), and the matching composition is used. If the source does not distinguish the two states for a food, the same composition is used for both, and the result lists that ingredient (see [DATA_MODEL.md](DATA_MODEL.md) §3).

## 3. Recipe nutrient aggregation

For nutrient `k`, the total is the sum of the contributions from all recipe items:

```
recipe_nutrient[k] = sum(item_nutrient[k])
```

Contributions are aggregated according to the value state (see section 11):

| Value state | Contribution to the total |
|-------------|---------------------------|
| Known | `N * Q / R` |
| Zero | 0 |
| Trace | 0 |
| Below detection limit | 0 |
| Unknown | 0 (initial phase only) |

During the initial phase, unknown values are treated as zero. This is a deliberate simplification. The engine still records, for each nutrient, the items whose value was unknown, so that results can later report completeness without changing the calculation API.

All contributions to a total share the same nutrient definition and canonical unit. This is guaranteed by the data model: values with different definitions are different nutrients.

## 4. Per-serving values

Let `S` be the number of servings in the recipe.

```
nutrient_per_serving = recipe_nutrient / S
```

`S` must be a positive integer.

In integers, the division is applied once to the unrounded sum: `sum(N_i * Q_i) / (R * S)` (D12).

The application should explain that serving size is defined by the user and may not match a standard dietary portion.

If the recipe's cooked weight `W` is known, a portion of `P` grams of the finished dish is:

```
nutrient_in_portion = recipe_nutrient * P / W
```

In integers: `sum(N_i * Q_i) * P / (R * W)`, one division (D12). `P` and `W` are positive integer grams; without `W`, a portion in grams cannot be calculated and is an error.

## 5. Energy units

Energy may be displayed in kilocalories (kcal) or kilojoules (kJ).

```
1 kcal = 4.184 kJ
```

Prefer the energy value supplied by the food database and record its convention. CIQUAL provides energy according to EU Regulation 1169/2011.

Do not assume that energy calculated from macronutrients will exactly match database energy. Differences can arise from fibre, organic acids, polyols, specific conversion factors, and the energy calculation convention.

If energy is estimated from macronutrients, use the EU Regulation 1169/2011 factors and document them.

## 6. Unit conversion

Each nutrient has a canonical unit.

Examples:

- Protein: g.
- Vitamin C: mg.
- Vitamin D: µg.
- Energy: kJ and kcal, as two separate nutrients taken from the source (D14).

The full table is in [DECISIONS.md](DECISIONS.md), D14.

Mass conversions:

```
1 g  = 1000 mg
1 mg = 1000 µg
```

Conversions are applied at import time, so the engine only sees canonical units.

Unit conversions must not imply equivalence between different chemical forms or different nutrient measurement systems. For example, vitamin A activity units and vitamin A mass-equivalent units cannot be combined without the appropriate conversion convention and source information.

## 7. Daily energy expenditure

Daily energy expenditure is estimated in two stages:

1. Estimate the basal metabolic rate (BMR) with the **Henry (2005)** equations, as used by EFSA. They depend on sex, age band, weight, and height.
2. Multiply by the physical activity level (PAL):

```
total_daily_energy = BMR * PAL
```

The user selects an activity level that maps to a PAL value (EFSA uses values from 1.4 to 2.0 for adults). Activity is not tracked, and no separate exercise energy is added.

This calculation uses floats (`f64`). The equation coefficients and PAL values must be taken from the primary source, verified by a human, and stored with a method identifier and version.

Supported profiles: adults aged 18 and over, not pregnant or lactating. Other profiles return an "unsupported profile" result.

## 8. Macronutrient targets

Macronutrient targets may be expressed as:

- Grams per day.
- Grams per kilogram of body weight.
- A percentage of daily energy intake (EFSA reference intake ranges).

For a target expressed as fraction `f` of daily energy `E`:

```
macro_energy = E * f
```

A conversion to grams requires a documented energy-per-gram factor (EU Regulation 1169/2011).

For a protein target expressed relative to body weight:

```
protein_target_g = body_weight_kg * target_g_per_kg
```

The target values must come from a documented reference or an explicitly user-defined goal. They must not be presented as universal requirements.

## 9. Vitamins and minerals

Daily reference values are EFSA Dietary Reference Values, selected using:

- Age.
- Sex, where applicable.

Pregnancy and lactation are not supported in the first release.

Reference values represent different concepts: average requirement (AR), population reference intake (PRI), adequate intake (AI), and tolerable upper intake level (UL). The application must retain these distinctions in both its model and user interface.

## 10. Intake comparison

A simple comparison can be expressed as:

```
intake_percentage = intake / reference_value * 100
```

This percentage is meaningful only when the intake and reference value use compatible units and definitions.

- Label the selected reference type clearly; a percentage of the AR and a percentage of the PRI mean different things.
- Comparing one meal with a daily reference is only indicative, and one day is not "usual intake": reference values apply to intake averaged over time.
- If some ingredients had unknown values for a nutrient, show that list next to the percentage.
- Upper limits apply to total intake, including supplements, which are not modelled. Display them separately from recommended intakes.
- Do not treat an intake percentage as a diagnosis or as proof of a deficiency or excess.

## 11. Missing and incomplete data

A nutrient value can be:

- Known and positive.
- Known to be zero.
- Trace.
- Below a detection or reporting threshold.
- Unknown or not reported.

The data model preserves these states (see [DATA_MODEL.md](DATA_MODEL.md) §4).

Initial phase: zero, trace, below-detection-limit, and unknown values all contribute 0 (see section 3). The result still lists the items with an unknown value, for each nutrient.

Later phases: a total with incomplete ingredient data should be labelled as partial, with a completeness indicator (for example, the share of recipe weight with a known value). Do not infer that unreported nutrients are absent.

## 12. Precision and rounding

- Store and aggregate amounts as integers (fixed-point, 3 decimals); see section 2.
- Use floats only for equations, PAL, and percentages.
- Round for display only, not after each ingredient.
- Display precision depends on the unit (D16): g and mg 1 decimal, µg 2 decimals, kcal and kJ 0 decimals.
- Every rounding, in calculation and display, is half away from zero (D11).
- Use explicit tolerances in tests that involve floats.
- Validate inputs before calculation.

## 13. Validation

Test:

- Scaling by reference quantity.
- Summation of multiple ingredients, including the single final division.
- Per-serving and per-portion calculations.
- Each value state alone, and combinations of two states.
- The list of ingredients with unknown values.
- Raw/cooked selection and the shared-composition fallback.
- Zero and negative quantities.
- Rounding behavior.
- Reference-value selection.
- Boundaries and unsupported profiles.

Expected values come from *golden* fixtures: a small set of CIQUAL foods and a few recipes calculated by hand, with their expected results checked against the source tables. Phase 1 uses synthetic foods instead; CIQUAL fixtures come with phase 2 (D13).

## 14. Scientific references

Each implemented energy equation and nutritional reference must cite its original source or authoritative guideline, and be verified by a human against that source.

The software records which method and version were used for an estimate, so that future revisions do not silently change the meaning of historical results.
