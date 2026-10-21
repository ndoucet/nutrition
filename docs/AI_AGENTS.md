AI Agents — Meal Nutrient Estimator

1. Project Architect
Defines the application architecture and module boundaries.
Maintains technical decisions and project conventions.
Coordinates interfaces and dependencies between agents.
Keeps the design simple and extensible.

2. Rust Backend Developer
Implements the Rust domain model and application logic.
Develops meal composition and nutrient aggregation.
Implements validation, error handling, and calculation services.
Keeps business logic independent of the frontend and database.

3. Database Engineer
Designs and maintains the SQLite schema and migrations.
Implements food, nutrient, meal, and profile persistence.
Ensures data integrity, unit consistency, and efficient queries.
Supports food data import and export.

4. Nutrition Scientist
Defines and validates nutrient calculation methods.
Selects documented equations for energy expenditure and nutritional requirements.
Identifies reliable food composition databases and dietary reference values.
Documents assumptions, units, limitations, and data provenance.

5. Test & Quality Engineer
Develops unit, integration, and regression tests.
Verifies numerical accuracy and edge cases.
Tests missing nutrient data, invalid quantities, and unit conversions.
Ensures new features do not break existing behavior.

6. Code Reviewer
Reviews changes for correctness, readability, and maintainability.
Checks consistency with architecture and coding conventions.
Identifies duplicated logic, unnecessary complexity, and potential bugs.
Verifies that changes include appropriate tests and documentation.
Collaboration Rules
The Project Architect owns architectural decisions.
The Nutrition Scientist validates nutritional methods; the Rust Backend Developer implements them.
The Database Engineer owns schema and persistence changes.
The Test & Quality Engineer independently verifies expected behavior.
The Code Reviewer reviews changes before integration.
Every agent documents assumptions and reports unresolved issues.
Avoid overlapping ownership and unnecessary dependencies between agents.
Shared Priorities
Correct and reproducible calculations.
Explicit units and reliable food data.
Missing values must never silently become zero.
Small, testable, incremental changes.
Local-first operation and minimal complexity.

Implementation
The roles are implemented with generic Claude Code agents and skills from `../base-agents`, symlinked into `.claude/`. Project rules and priorities are in `CLAUDE.md`.

| Role | Implemented as |
|------|----------------|
| Project Architect | Main session, `decision-record` skill |
| Rust Backend Developer | Main session, `rust-conventions` skill |
| Database Engineer | `sqlite-migrations` skill (phase 2) |
| Nutrition Scientist | `domain-verifier` agent |
| Test & Quality Engineer | `test-engineer` agent |
| Code Reviewer | `/code-review` and `docs-conformance-reviewer` agent |

Features are implemented through the `/feature` workflow.
