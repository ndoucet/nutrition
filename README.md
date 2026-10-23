# Meal Nutrient Estimator

A small, extensible application to estimate the nutritional composition of user-defined meals and compare nutrient intake with estimated daily nutritional requirements.

The project uses a Rust calculation engine and a local food database. The frontend is intentionally decoupled from the core so that desktop, web, or Android interfaces can be developed independently.

## Goals

- Maintain a database of foods and their nutrient composition.
- Define recipes as collections of foods with quantities.
- Calculate energy, macronutrients, vitamins, minerals, and other available nutrients.
- Estimate daily energy expenditure and nutritional requirements from user-provided information.
- Compare meal or daily intake with nutritional reference values.
- Keep calculations reproducible, transparent, and testable.
- Prefer local storage and offline operation.

## Core concepts

- **Food**: a food item with a known or partially known nutritional composition, available in a raw and a cooked state.
- **Nutrient**: a measurable component such as protein, vitamin C, iron, or energy.
- **Recipe**: a reusable collection of foods and their quantities.
- **Log entry**: what was eaten on a given day (a food or a recipe portion). Planned for phase 5.
- **Profile inputs**: age, sex, height, weight, and activity level used to estimate requirements. Stored only if the user asks.
- **Nutritional reference**: a documented recommendation or reference value applicable to a population.

Food composition data and nutritional recommendations are maintained separately.

## Technology

- Backend: Rust (Cargo workspace)
- Database: SQLite
- Serialization and import/export: JSON
- Frontend: to be selected; candidates include a Rust desktop GUI, a web frontend, or native Android using Kotlin.

See [Technology Choices](docs/TECHNOLOGY.md) and [Architecture](docs/ARCHITECTURE.md).

## Project status

Phase 1 done: `nutrition-core` is a tested Rust library that calculates the nutrient composition of a recipe, tested with synthetic foods (see [Decisions](docs/DECISIONS.md), D13 and D15). Try it with `cargo run -p nutrition-core --example print_recipe`.

Phase 2 in progress: `nutrition-store` creates and migrates the SQLite database (schema version 1, with the initial nutrients). Next: food and recipe persistence, JSON import and export, a small verified CIQUAL dataset, and a minimal CLI.

## Getting started

### Install

Requirements: a stable Rust toolchain, 1.85 or later (developed with Rust 1.87), installed for example with [rustup](https://rustup.rs). SQLite is bundled; there is no database server or other service to set up.

```sh
git clone <repository-url> nutrition
cd nutrition
cargo build
```

### Run the example

```sh
cargo run -p nutrition-core --example print_recipe
```

It builds a recipe from synthetic foods (not real composition data), removes an ingredient, and prints the ingredients, the total and per-serving value of each nutrient, and which ingredients had unknown values. The source is in [`crates/nutrition-core/examples/print_recipe.rs`](crates/nutrition-core/examples/print_recipe.rs).

### Use the library

```rust
use nutrition_core::{
    calculate_recipe, initial_nutrients, Composition, Compositions, Error, Food, FoodCatalog,
    FoodId, Grams, NutrientValue, Recipe, Servings, State,
};

fn main() -> Result<(), Error> {
    // Amounts are thousandths of the canonical unit per 100 g: 31 g of protein → 31_000.
    let mut foods = FoodCatalog::new();
    foods.insert(Food::new(
        FoodId(1),
        "Lentils",
        Compositions::Shared(
            Composition::per_100g().with("protein", NutrientValue::known(31_000)?),
        ),
    ))?;

    let mut recipe = Recipe::new("Lentil soup", Servings::new(2)?);
    recipe.add_ingredient(FoodId(1), State::Cooked, Grams::new(150)?);

    let result = calculate_recipe(&recipe, &foods, &initial_nutrients())?;
    print!("{result}"); // Protein  46.5 g / 23.3 g
    Ok(())
}
```

Nutrients without a value are not an error: they count as 0 and the result lists the ingredients concerned.

### Tests and checks

```sh
cargo test --workspace                                  # unit and integration tests
cargo clippy --workspace --all-targets -- -D warnings   # lints
cargo fmt --all                                         # formatting
cargo xtask uml                                         # regenerate the class diagram
```

Integration tests are in [`crates/nutrition-core/tests/`](crates/nutrition-core/tests/). Expected values are calculated by hand from [Calculations](docs/CALCULATION.md), with the arithmetic in comments.

Every struct, enum, and function has a doc comment that states units and, for functions, documents each argument (see [Decisions](docs/DECISIONS.md), D21). The [class diagram](docs/CLASS_DIAGRAM.md) is generated from the source; the test suite fails when it is out of date (D22).

## Documentation

- [Architecture](docs/ARCHITECTURE.md): application components and responsibilities.
- [Technology Choices](docs/TECHNOLOGY.md): languages, libraries, and frontend options.
- [Data Model](docs/DATA_MODEL.md): food, nutrient, recipe, and profile entities.
- [Class Diagram](docs/CLASS_DIAGRAM.md): UML class diagram of `nutrition-core`, generated from the source.
- [Calculations](docs/CALCULATION.md): nutrient aggregation, units, energy, and requirements.
- [Data Sources](docs/DATA_SOURCE.md): food composition databases, references, and licensing.
- [Decisions](docs/DECISIONS.md): design decisions taken so far.
- [Glossary](docs/GLOSSARY.md): nutrition terms and abbreviations.
- [Roadmap](docs/ROAD_MAP.md): development phases and initial acceptance criteria.
- [Completed roadmap](docs/ROAD_MAP_DONE.md): roadmap items already done.
- [AI Agents](docs/AI_AGENTS.md): roles for AI-assisted development.
- Contributing: development and testing workflow (to be written).

## Design principles

- Correctness before features.
- Explicit and consistent units.
- Unknown nutrient values are kept distinct from zero in the data. During the initial phase they contribute 0 to totals, and the result lists the ingredients concerned (see [Decisions](docs/DECISIONS.md), D8).
- Calculations must be independent of the frontend.
- Food data and reference values must retain their provenance.
- Estimates must expose their assumptions and limitations.
- No personal information is collected or stored unless the user asks for it.

## Disclaimer

The application provides nutritional estimates for informational and educational purposes. Estimates are not medical advice, a diagnosis, or a substitute for individualized professional guidance.
