# Completed Roadmap

Items moved from [ROAD_MAP.md](ROAD_MAP.md) when done, grouped by phase, with the commit that completed them.

## Phase 2: Persistence

- Before starting: create the `sqlite-migrations` skill in `base-agents` (Database Engineer role, see [AI_AGENTS.md](AI_AGENTS.md)). Completed 2026-10-23 in base-agents 6e0bb7f.
- Add SQLite. Completed 2026-10-23 in 935635f.
- Create versioned schema migrations (D23), with the initial nutrients and the "user" data source (D29). Completed 2026-10-23 in 935635f.

## Phase 1: Core calculation engine

Completed 2026-10-21 in a5a784f.

- Define food, nutrient, recipe, and recipe-item types.
- Define canonical units.
- Implement nutrient scaling.
- Implement recipe aggregation.
- Implement per-serving calculations.
- Handle incomplete nutrient data.
- Add automated unit tests.

Scope: the `nutrition-core` crate only, tested with synthetic foods (D13, D15).

Deliverable: a tested Rust library that calculates recipe composition without a frontend:

- add and remove ingredients through library functions;
- print the content line by line.
