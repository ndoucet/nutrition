# Data Model

## 1. Overview

The data model separates food composition from nutritional recommendations.

- Food composition answers: "What nutrients are present in this food?"
- Nutritional references answer: "What intake is recommended or considered adequate for this population?"

These datasets have different sources, update cycles, and meanings.

All amounts are stored as integers (see [DECISIONS.md](DECISIONS.md), D1): quantities in grams, nutrient amounts as fixed-point values with 3 decimals of the canonical unit. Timestamps are UTC text in the form `YYYY-MM-DDTHH:MM:SSZ`.

## 2. Nutrient

Suggested fields:

- `id`
- `code`: stable identifier including the definition, e.g. `carbohydrate_available`
- `name`
- `category`
- `canonical_unit`
- `definition`: the convention used (source and method)
- `description`, optional

Schema version 1 stores `id`, `code`, `name`, and `canonical_unit` (stored as `g`, `mg`, `ug`, `kJ`, or `kcal`). `category`, `definition`, and `description` are added once their text is verified (D30). The initial set is inserted by the first migration (D29).

Initial set (D7; codes and canonical units in D14):

- Energy, as two nutrients: kJ and kcal.
- Protein.
- Available carbohydrate.
- Sugars.
- Fibre.
- Fat.
- Saturated fat.
- Salt.
- Calcium.
- Iron.
- Vitamin C.
- Vitamin D.
- Vitamin B12.

A nutrient is identified by its substance **and** its definition. Definitions follow French and European conventions (CIQUAL, EU Regulation 1169/2011). Two values with different definitions (for example, available carbohydrate and carbohydrate by difference) are different nutrients and are never summed together.

## 3. Food

A food represents one ingredient, which can be used raw or cooked.

Suggested fields:

- `id`
- `name`
- `category`
- `source_id`
- `description`, optional
- `created_at`: UTC timestamp, set on insertion (D25)

### Food composition

Each food has one composition per preparation state, or one composition shared by both (D28).

Fields:

- `id`
- `food_id`
- `source_id`: the food's source, repeated so that `(source_id, source_record_id)` can be unique (D2)
- `state`: `raw`, `cooked`, or `shared` when the source does not distinguish raw and cooked, so the same composition is used for both (D28)
- `source_record_id`: the source's identifier for this composition; absent for user-created foods
- `reference_quantity_g`: 100 by default

Rules:

- When the source provides both states (for example, two CIQUAL records for dry and cooked pasta), each state links to its own record. The pairing is done during import and documented in a mapping file.
- A food without any composition, or with only one of `raw` and `cooked`, is invalid stored data (D28).
- When the source provides only one, the food has a single `shared` composition used for both states (`Compositions::Shared` in Rust). A `shared` composition cannot coexist with another composition of the same food. The calculation result lists these ingredients, whichever state was selected.
- Values are per 100 g of edible portion. The weight entered by the user must be the edible weight (no peel, bones, etc.).
- Other materially different forms (dried, fried, etc.) are separate foods.

## 4. Food nutrient values

Each record links one food composition to one nutrient.

Fields:

- `composition_id`
- `nutrient_id`
- `value_state`: `known`, `zero`, `trace`, `below_limit`, or `unknown`
- `amount`: integer fixed-point; set for `known`. For `below_limit`, holds the limit.

Rules:

- The amount is expressed per the reference quantity, in the nutrient's canonical unit.
- A missing record is equivalent to `unknown`. An explicit `unknown` record keeps the source's marker (CIQUAL `-`).
- `amount` is positive for `known`, non-negative for `below_limit`, and absent for the other states.
- A known value of 0 is stored as `zero`; negative amounts are rejected (D18).
- CIQUAL markers map directly to these states: a number → `known` (or `zero`), `traces` → `trace`, `< x` → `below_limit`, `-` → `unknown`.

In Rust, the state is a type rather than a convention:

```rust
enum NutrientValue {
    Known(MeasuredAmount),    // fixed-point, 3 decimals, always > 0
    Zero,
    Trace,
    BelowLimit { limit: MeasuredAmount },
    Unknown,
}
```

`MeasuredAmount` is a non-negative fixed-point amount that can only be built through validating constructors, so D18 holds by construction.

## 5. Data sources

Fields:

- `id`
- `name`
- `version`
- `source_url`
- `licence`
- `retrieved_at`, if applicable

Retain the source record identifier and original data where needed for traceability and re-imports.

Foods created by the user belong to a source named "user" (D25).

## 6. Recipe

A reusable collection of ingredients.

Fields:

- `id`
- `name`
- `notes`
- `servings`: positive integer
- `cooked_weight_g`, optional: total weight of the finished dish, used to convert a portion in grams into a share of the recipe
- `created_at`
- `updated_at`: set by the store whenever the recipe or its items change

A recipe contains one or more recipe items. Recipes do not contain other recipes in the initial phases.

## 7. Recipe item

Fields:

- `recipe_id`
- `position`: 0-based position in the recipe; `(recipe_id, position)` is unique
- `food_id`
- `state`: `raw` or `cooked`
- `quantity_g`: positive integer, the weight in the selected state

Items are identified by their position in the recipe, and the same food may appear more than once (D17).

If household units such as cups or tablespoons are added later, convert them using a food-specific conversion where possible. Do not assume that a volume unit has the same mass for every food.

## 8. Log entry (phase 5)

Records what was eaten on a given day.

Suggested fields:

- `id`
- `date`
- `time`, optional
- either `food_id` + `state` + `quantity_g`, or `recipe_id` + a portion (number of servings, or grams of the finished dish when `cooked_weight_g` is known)

Daily totals are the sum of the log entries for a date.

## 9. Profile inputs

The inputs needed to estimate requirements:

- `age_years`
- `sex`: used to select the equation and reference values
- `height_cm`
- `weight_kg`
- `activity_level`: a PAL category

These values are passed to the calculation and are **not stored** by default. They are stored locally (one record) only if the user enables the option to remember them. The user can edit or delete the stored record at any time.

Activity is not tracked: there are no activity records.

## 10. Nutritional reference

Suggested fields:

- `id`
- `nutrient_id`
- `age_range`
- `sex_criteria`, if applicable
- `reference_type`: `AR`, `PRI`, `AI`, `RI`, or `UL`
- `amount`
- `unit`
- `source_id`
- `version`

Reference types are not interchangeable and must be labelled accordingly. Only adult, non-pregnant, non-lactating references are needed in the first release.

## 11. Database constraints

- Enforce foreign keys.
- Food quantities, reference quantities, cooked weights, and serving counts are integers from 1 to 4294967295 (the range of `Grams` and `Servings` in `nutrition-core`).
- Validate units at import time; everything stored is in canonical units.
- Use unique identifiers for foods and nutrients, and a unique `(source_id, source_record_id)` for imported records.
- Apply schema changes through versioned migrations (D23). The schema is in `crates/nutrition-store/migrations/`; tables are `STRICT`.
- A food used by a recipe item, a nutrient used by a value, and a data source used by a food cannot be deleted. A food's source cannot change while it has compositions (D2). Deleting a food deletes its compositions and values; deleting a recipe deletes its items.
- Keep food provenance when importing or updating records.

## 12. Derived values

Recipe nutrient totals are calculated from recipe items and food composition records. They are not stored as authoritative data.

If historical calculations must remain reproducible after food data changes, preserve a calculation snapshot or record the exact food-data version used.
