# Decisions

Design decisions taken so far. Each entry gives the decision, its date and, where useful, the reason. D1–D10 and the Cn IDs come from the initial review of the first draft (Cn = contradiction, Dn = decision); D11 onwards were recorded during implementation.

When a decision changes, update its entry and keep a short note of the previous choice.

| ID | Decision | Date |
|----|----------|------|
| D1 / C7 | Integers wherever possible; floats only where required | 2026-10-21 |
| D2 | Integer identifiers | 2026-10-21 |
| D3 | CIQUAL as the single initial food source | 2026-10-21 |
| D3b | European nutrient definitions as the base | 2026-10-21 |
| D4 | EFSA dietary reference values, adults only | 2026-10-21 |
| D5 | Henry (2005) BMR × PAL for energy requirements | 2026-10-21 |
| D6 | Quantities in grams only | 2026-10-21 |
| D7 | Short initial nutrient set | 2026-10-21 |
| D8 | Missing-data aggregation rule | 2026-10-21 |
| D9 | Raw and cooked states for each food | 2026-10-21 |
| D10 | Recipe and log entry are separate entities | 2026-10-21 |
| C1 | Profile data stored only on user request | 2026-10-21 |
| C5 | Incomplete data is not an error | 2026-10-21 |
| C6 | No activity tracking; activity level is a PAL input | 2026-10-21 |
| D11 | Integer division rounds half away from zero | 2026-10-21 |
| D12 | Per-serving and per-portion values use a single division | 2026-10-21 |
| D13 / C4 | Phase 1 uses synthetic test foods; CIQUAL fixtures from phase 2 | 2026-10-21 |
| D14 | Canonical units; energy stored as two nutrients (kJ and kcal) | 2026-10-21 |
| D15 | Phase 1 scope: `nutrition-core` only, printable result | 2026-10-21 |
| D16 | Display precision per unit | 2026-10-21 |
| D17 | Recipe items are identified by position; a food may appear twice | 2026-10-21 |
| D18 | A known value of 0 is normalized to the zero state | 2026-10-21 |
| D19 | Phase 1 supports only a 100 g reference quantity | 2026-10-21 |
| D20 | Nutrients are data, not a Rust enum | 2026-10-21 |
| D21 | Every struct, enum, and function has a doc comment in a fixed format | 2026-10-22 |
| — | Roadmap contradictions (C2, C3) postponed | 2026-10-21 |

## D1 / C7: numeric representation

Use integers as much as possible, and floats only when an operation requires them.

- Food quantities are integer grams.
- Nutrient amounts are stored as integer fixed-point values with 3 decimals of the nutrient's canonical unit (for example, 12.345 g of protein is stored as `12345`).
- Meal aggregation is done in integers: products and sums in `i128`, results in `i64` (an overflow on narrowing is an error). The division by the reference quantity happens once, on the total, not per ingredient (see [CALCULATION.md](CALCULATION.md) §3). Previously (2026-10-21): aggregation in `i64`; changed with D11 so that intermediate products cannot overflow.
- Floats (`f64`) are used for energy equations, PAL multiplication, percentages of reference values, and display conversions.

**Why:** integers make stored values and aggregation exact and reproducible. The equations involve non-integer coefficients and need floats anyway.

## D2: identifiers

Internal `INTEGER` primary keys. Imported foods also keep a natural key `(source_id, source_record_id)` so that re-imports update existing records instead of duplicating them.

## D3: initial food source

CIQUAL (ANSES) is the only food source for the initial phases, so that a meal never mixes nutrient definitions from different sources. Its licence must be checked and recorded before bundling (see [DATA_SOURCE.md](DATA_SOURCE.md)).

## D3b: nutrient definitions

French and European conventions are the base: the CIQUAL nutrient definitions and EU Regulation 1169/2011 (for example, available carbohydrate excluding fibre, and salt = sodium × 2.5).

If USDA data is added later, a documented conversion process must map USDA nutrients to these definitions before the data enters the application database.

## D4: reference values

EFSA Dietary Reference Values, using the EFSA vocabulary (AR, PRI, AI, UL, RI). For the first release, the supported population is healthy adults aged 18 and over who are not pregnant or lactating. Other profiles return an "unsupported profile" result.

## D5: energy equation

Resting energy is estimated with the Henry (2005) equations, as used by EFSA. Total energy is BMR × PAL. The coefficients must be taken from the primary source and verified by a human before use.

## D6: quantity units

Grams only for the initial phases. Household measures (cups, spoons, pieces) come later, with per-food conversion factors.

## D7: initial nutrient set

Energy, protein, available carbohydrate, sugars, fibre, fat, saturated fat, salt, plus calcium, iron, vitamin C, vitamin D, and vitamin B12. This list can be extended once import and validation are reliable.

## D8: missing-data aggregation

| Value state | Contribution to the total |
|-------------|---------------------------|
| Known | scaled amount |
| Zero | 0 |
| Trace | 0 |
| Below detection limit | 0 |
| Unknown | 0 (initial phase) |

The unknown state is kept as a separate state in the data and in the code. The calculation result lists, for each nutrient, the ingredients whose value was unknown. That way, when a completeness indicator is added later, the API does not change.

## D9: raw and cooked states

Each food can be used in a raw or a cooked state, selected with a flag in the API and a tickbox in the UI. The quantity entered is the weight in the selected state.

If the source does not distinguish the two states for a food, the same composition is used for both. The calculation result lists the ingredients for which this fallback was used.

## D10: recipe and log entry

A **recipe** is a reusable list of ingredients with quantities and a number of servings. A **log entry** records what was eaten on a given day (a food or a portion of a recipe). Log entries are planned for phase 5, but the two concepts stay separate from the start. Recipes do not contain other recipes in the initial phases.

## C1: profile data

The requirement calculation takes age, sex, height, weight, and activity level (PAL) as inputs. These values are not stored by default. They are saved locally only if the user enables an explicit "remember my information" option, as a convenience to avoid entering them again, and the user can change or delete them at any time.

No information is collected unless the user requests it.

## C5: incomplete data

Incomplete nutrient data is never an error in the initial phases: missing values use the unknown state. Ingredients can only be selected from the local database.

## C6: activity

Activity is not tracked: there are no activity records or exercise logs. The user selects a general activity level, which maps to a PAL value used in the energy calculation.

## D11: rounding of integer division

Every integer division in the calculation engine (fixed-point amount × grams ÷ reference quantity, and the divisions in D12) rounds half away from zero. Intermediate products use `i128`, so they cannot overflow. Display rounding (D16) uses the same rule.

Example: `12345 × 7 / 100 = 864.15` → `864`; `12345 × 10 / 100 = 1234.5` → `1235`.

**Why:** the result must be deterministic and documented; truncation would bias every total downwards.

## D12: per-serving and per-portion division

Per-serving and per-portion values are computed from the unrounded sum of products, with one division:

```
per_serving = sum(N_i × Q_i) / (R × S)
portion     = sum(N_i × Q_i) × P / (R × W)
```

They are not derived from the rounded recipe total, so they are rounded once.

## D13 / C4: phase 1 data

Phase 1 tests use synthetic foods with round, hand-checkable values, clearly marked as not real composition data. CIQUAL golden fixtures are added in phase 2, once the CIQUAL licence has been checked (D3).

**Why:** phase 1 has no database, and bundling CIQUAL values requires the licence check first.

## D14: canonical units

| Nutrient | Code | Canonical unit |
|----------|------|----------------|
| Energy | `energy_kj` | kJ |
| Energy | `energy_kcal` | kcal |
| Protein | `protein` | g |
| Available carbohydrate | `carbohydrate_available` | g |
| Sugars | `sugars` | g |
| Fibre | `fibre` | g |
| Fat | `fat` | g |
| Saturated fat | `fat_saturated` | g |
| Salt | `salt` | g |
| Calcium | `calcium` | mg |
| Iron | `iron` | mg |
| Vitamin C | `vitamin_c` | mg |
| Vitamin D | `vitamin_d` | µg |
| Vitamin B12 | `vitamin_b12` | µg |

Energy is stored as two nutrients, both taken from the source, and never converted from one to the other.

**Why:** CIQUAL publishes both values under EU Regulation 1169/2011; they are not exactly related by 4.184 after the source's own rounding, and recomputing one from the other would lose provenance.

## D15: phase 1 scope

Phase 1 delivers the `nutrition-core` crate only. The `nutrition-store` and `nutrition-cli` crates are created when they are needed. "Print the content line by line" is a `Display` implementation on the calculation result.

## D16: display precision

| Unit | Decimals |
|------|----------|
| g | 1 |
| mg | 1 |
| µg | 2 |
| kcal, kJ | 0 |

Rounding for display only, half away from zero (D11).

## D17: recipe items

A recipe item is identified by its position in the recipe. The same food may appear more than once (for example, raw and cooked). Ingredients are removed by position, and the result's data-quality lists (unknown values, raw/cooked fallback) refer to positions.

## D18: known zero

A known value of 0 is normalized to the `Zero` state when the value is created. Negative amounts are rejected.

**Why:** one representation per meaning keeps comparisons and tests simple.

## D19: reference quantity

Phase 1 accepts only compositions with a reference quantity of 100 g; any other value is an error. The common-denominator logic of [CALCULATION.md](CALCULATION.md) §2 is added if a source needs it.

## D20: nutrients as data

A nutrient is a value (code, name, canonical unit), not a variant of a Rust enum. The initial set (D7, D14) is provided as a constant list. Calculations take the list of nutrients to compute.

**Why:** the set will grow and will come from the database (DATA_MODEL §2); an enum would need a code change for every new nutrient.

## D21: code documentation

Every struct, enum, and function in every `.rs` file has a `///` doc comment, and the comments follow one format. This applies to public and private items, trait impls, test helpers, and `#[test]` functions. Documentation is written with the code, in the same change, never added afterwards.

- **Structs and enums:** a summary of what the type represents, with its unit or scale and the decisions it implements (for example, "A nutrient amount in thousandths of its canonical unit (D1)").
- **Functions:** a one-line summary, then details if needed. A function with arguments other than `self` ends with an `# Arguments` section, one line per argument in the form `` * `name` - meaning ``, giving what the argument represents and its unit or scale (grams, thousandths of the canonical unit, position from 0).
- **Tests:** the summary states the behaviour checked; the hand calculation of the expected values stays in comments in the body.

**Why:** the code mixes several units and scales (grams, thousandths of different canonical units, 0-based positions shown from 1). Units that are written next to every value and argument prevent scaling errors, and a single format keeps the documentation easy to read and to review.

## Postponed

The roadmap contradictions C2 (UI before frontend choice) and C3 (v0.1 scope) will be resolved later. C4 (phase-1 data) is resolved by D13.
