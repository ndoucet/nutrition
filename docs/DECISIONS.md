# Decisions

Design decisions taken so far. Each entry gives the decision, its date and, where useful, the reason. The IDs refer to [INIT_REVIEW.md](INIT_REVIEW.md) (Cn = contradiction, Dn = decision).

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
| — | Roadmap contradictions (C2, C3, C4) postponed | 2026-10-21 |

## D1 / C7: numeric representation

Use integers as much as possible, and floats only when an operation requires them.

- Food quantities are integer grams.
- Nutrient amounts are stored as integer fixed-point values with 3 decimals of the nutrient's canonical unit (for example, 12.345 g of protein is stored as `12345`).
- Meal aggregation is done in integers (`i64`). The division by the reference quantity happens once, on the total, not per ingredient (see [CALCULATION.md](CALCULATION.md) §3).
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

## Postponed

The roadmap contradictions (C2: UI before frontend choice, C3: v0.1 scope, C4: phase-1 data) will be resolved later.
