# Meal Nutrient Estimator

Rust library and local SQLite database that estimate the nutrient composition of recipes and compare intake with EFSA reference values. See [README.md](README.md).

Status: phase 1 (core calculation engine) implemented in `crates/nutrition-core`.

## Docs map

- Purpose: README.md
- Architecture: docs/ARCHITECTURE.md
- Decisions: docs/DECISIONS.md
- Roadmap: docs/ROAD_MAP.md
- Data model: docs/DATA_MODEL.md
- Domain methods: docs/CALCULATION.md
- Domain data and sources: docs/DATA_SOURCE.md
- Glossary: docs/GLOSSARY.md
- Technology: docs/TECHNOLOGY.md
- AI roles: docs/AI_AGENTS.md

## Commands

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p nutrition-core --example print_recipe   # phase 1 demo
```

## Rules

- `nutrition-core` does no I/O: no SQLite, filesystem, network, or UI dependency. Persistence lives in `nutrition-store`.
- Numbers follow D1: integer grams for quantities, integer fixed-point (3 decimals of the canonical unit) for nutrient amounts, aggregation in `i128` with `i64` results and a single division, rounded half away from zero (D11, D12). `f64` only for energy equations, PAL, percentages, and display. Round for display only.
- Missing data follows D8: the unknown state stays distinct from zero in data and code, and results list the ingredients with unknown values. Never use 0 or another sentinel to mean unknown.
- Incomplete data is not an error (C5). Errors are invalid input, unknown food, unsupported profile or method, overflow, database and import failures (ARCHITECTURE.md §5).
- Domain constants (Henry coefficients, EFSA reference values) are implemented only after they are verified against the primary source and confirmed by a human (D5). Use the `domain-verifier` agent.
- Every calculation change has tests with hand-checked expected values; float comparisons use explicit tolerances.
- Profile data is never stored unless the user enables it, and never transmitted (C1).
- A design choice made or changed during work is recorded with the `decision-record` skill.

## Priorities

1. Correct and reproducible calculations.
2. Explicit units and data provenance.
3. Small, testable, incremental changes.
4. Local-first operation and minimal complexity.

## Workflow

Implement roadmap items with `/feature <item>`. Agents and skills come from `../base-agents` (installed as symlinks in `.claude/` by `../base-agents/install.sh`).
