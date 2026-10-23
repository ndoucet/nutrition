# Development Roadmap

> The inconsistencies between phases noted in the initial review (C2, C3) are known and postponed. C4 is resolved by D13.

Completed items are moved to [ROAD_MAP_DONE.md](ROAD_MAP_DONE.md).

## Phase 2: Persistence

- Food editing (update a stored food; re-import updates come with JSON import, D2).
- Implement recipe persistence.
- Add basic validation.
- Add JSON import and export (D26).
- Populate a small verified dataset, written by hand in JSON (D24), after the CIQUAL licence check.
- Add a minimal command-line interface (D27).

Migrations follow D23. The CIQUAL CSV importer is postponed (D24).

Deliverable: a local database-backed meal calculator.

## Phase 3: Basic user interface

- Search for foods.
- Add and remove ingredients.
- Edit ingredient quantities.
- Create and save recipes.
- Display nutrient totals.
- Display per-serving values.
- Indicate incomplete nutrient data.

Deliverable: a usable meal calculator.

## Phase 4: User profiles and nutritional requirements

- Implement profile input validation.
- Add documented energy estimation equations.
- Define physical activity levels (PAL categories).
- Add macronutrient targets.
- Import or implement age- and population-appropriate reference values.
- Display assumptions and sources.

Deliverable: a meal and daily intake comparison tool.

## Phase 5: Daily food logging

- Add a daily log.
- Aggregate multiple meals.
- Compare daily intake with selected references.
- Add date-based summaries.
- Preserve historical calculation context where necessary.

Deliverable: a basic daily nutrition tracker.

## Phase 6: Frontend and distribution

- Select the primary frontend.
- Package the application for the target platform.
- Improve accessibility.
- Improve database import workflows.
- Add backup and restore.
- Evaluate additional platforms.

Deliverable: a distributable application with a stable Rust core.

## Out of scope for the initial release

- Cloud synchronization.
- Social features.
- Barcode scanning.
- Automatic food recognition from photographs.
- Activity and exercise tracking.
- Medical or disease-specific dietary prescriptions.

These features should be considered only after the core data model and calculations are reliable.

## Version 0.1 acceptance criteria

- The database can be initialized locally.
- Foods can be created, queried, and retrieved.
- Food records contain validated units and provenance.
- Recipes can contain multiple ingredients.
- Recipe totals and per-serving values are calculated correctly.
- Missing data is distinguished from zero.
- Core calculations have automated tests.
- The project documents its data sources and limitations.
