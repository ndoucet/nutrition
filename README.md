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

Initial design and specification.

The first milestone is a tested Rust library that calculates the nutrient composition of a recipe using a small local food dataset.

## Documentation

- [Architecture](docs/ARCHITECTURE.md): application components and responsibilities.
- [Technology Choices](docs/TECHNOLOGY.md): languages, libraries, and frontend options.
- [Data Model](docs/DATA_MODEL.md): food, nutrient, recipe, and profile entities.
- [Calculations](docs/CALCULATION.md): nutrient aggregation, units, energy, and requirements.
- [Data Sources](docs/DATA_SOURCE.md): food composition databases, references, and licensing.
- [Decisions](docs/DECISIONS.md): design decisions taken so far.
- [Glossary](docs/GLOSSARY.md): nutrition terms and abbreviations.
- [Roadmap](docs/ROAD_MAP.md): development phases and initial acceptance criteria.
- [AI Agents](docs/AI_AGENTS.md): roles for AI-assisted development.
- [Initial Review](docs/INIT_REVIEW.md): review of the first draft.
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
