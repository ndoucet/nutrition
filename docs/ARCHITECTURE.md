# Architecture

## 1. Overview

The application is organized into independent layers:

- Domain model.
- Calculation engine.
- Persistence layer.
- Application services.
- Frontend.

The Rust core must be usable without a graphical interface.

## 2. Component responsibilities

### Domain model

Defines the core concepts:

- Food, with its raw and cooked composition.
- Nutrient.
- Food composition record and nutrient value state (known, zero, trace, below detection limit, unknown).
- Recipe and recipe item.
- Log entry (phase 5).
- Profile inputs (age, sex, height, weight, activity level).
- Nutritional reference value.

Domain types should enforce basic invariants wherever practical (for example, a quantity is a positive integer number of grams).

### Calculation engine

Implements:

- Nutrient scaling by food quantity.
- Recipe nutrient aggregation.
- Per-serving calculations.
- Daily intake aggregation (phase 5).
- Energy expenditure estimates (Henry BMR × PAL).
- Macronutrient targets.
- Selection and application of nutritional reference values.

The engine receives structured inputs and returns structured results. It does not read from SQLite or render UI components.

Reference selection is a pure function: it receives the profile inputs and a list of candidate references, and returns the applicable ones. The persistence layer loads the candidates.

### Persistence layer

Responsible for:

- Opening and initializing SQLite.
- Applying database migrations.
- Creating, reading, updating, and deleting records.
- Food searches.
- Recipe persistence.
- Profile persistence, only when the user enables it.
- Import and export.

### Application services

Coordinate operations across domain logic and persistence.

Examples:

- Create a recipe and validate its ingredients.
- Load a recipe and calculate its composition.
- Estimate requirements from profile inputs (entered or stored).
- Import a food dataset and validate its contents.

### Frontend

Responsible for:

- Input forms.
- Food search and selection.
- Recipe editing, including the raw/cooked tickbox for each ingredient.
- Nutrient summaries.
- Profile input, with an explicit option to remember it.
- Comparison and visualization.

The frontend calls application services and does not implement its own nutritional formulas.

## 3. Data flow

For recipe calculation:

1. Load the recipe and its ingredients.
2. Load the composition records matching each ingredient's selected state (raw or cooked).
3. Validate quantities.
4. Calculate each ingredient's nutrient contribution.
5. Aggregate contributions by nutrient.
6. Record ingredients with unknown values and ingredients that used the raw/cooked fallback.
7. Return totals, per-serving values, and these data-quality lists.
8. Present the results in the frontend.

For nutritional requirement estimation:

1. Receive the profile inputs (entered by the user, or loaded if the user chose to store them).
2. Check that the profile is supported (adults 18+, not pregnant or lactating).
3. Calculate BMR with the Henry equations and multiply by the selected PAL.
4. Select the relevant EFSA reference values.
5. Return values together with units, sources, method version, and assumptions.

## 4. Code layout

The project is a Cargo workspace. Splitting crates lets the compiler enforce the layer boundaries: the core crate cannot depend on SQLite or a UI.

```
crates/
  nutrition-core/       # domain + calculations; no I/O
    src/
      lib.rs
      domain/
        food.rs
        nutrient.rs
        recipe.rs
        profile.rs
        reference.rs
      calculations/
        recipe_nutrients.rs
        energy.rs
        requirements.rs
  nutrition-store/      # SQLite, migrations, import/export
    src/
      lib.rs
      database.rs
      foods.rs
      recipes.rs
      profiles.rs
      import/
        ciqual.rs
        json.rs
  nutrition-cli/        # command-line interface
    src/
      main.rs
```

The layout may be simplified during the initial prototype, but `nutrition-core` stays free of I/O.

## 5. Error handling

Errors stop a calculation. They include:

- Invalid input (zero or negative quantity, zero servings).
- A food that does not exist in the local database.
- An unsupported profile or calculation method.
- Database failures.
- Failed data imports (including unit conversion failures at import time).

Incomplete nutrient data is **not** an error. Missing values use the unknown state and are reported in the result alongside the totals. In the user interface, ingredients can only be selected from the local database.

## 6. API boundary

An HTTP API is optional.

If a separate frontend process requires one, expose a small API for food search, recipe management, profile inputs, and calculation requests.

Keep calculation logic in the shared Rust core regardless of the chosen communication mechanism.

## 7. Privacy

- The requirement calculation needs age, sex, height, weight, and activity level. These are inputs to the calculation and are not stored by default.
- They are stored, locally, only if the user enables an explicit option to remember them. The user can change or delete them at any time.
- No information is collected unless the user requests it.
- Profile information and meal history are never transmitted to a remote service without an explicit product requirement and the user's consent.
