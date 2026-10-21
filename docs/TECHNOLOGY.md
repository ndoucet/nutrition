# Technology Choices

## 1. Design requirements

The technology stack should support:

- Reliable numerical calculations.
- A small local database.
- Offline operation.
- Automated testing.
- Data import and export.
- Multiple possible frontends.
- Long-term maintainability without unnecessary complexity.

The initial implementation should minimize dependencies and avoid introducing a network service before one is needed.

## 2. Rust backend

Rust is responsible for domain logic, validation, persistence integration, and nutritional calculations.

The code is organized as a Cargo workspace (`nutrition-core`, `nutrition-store`, `nutrition-cli`; see [ARCHITECTURE.md](ARCHITECTURE.md) §4).

Recommended initial dependencies:

| Purpose | Candidate |
|---------|-----------|
| Serialization | `serde` |
| JSON import/export | `serde_json` |
| SQLite access | `rusqlite`, with the `bundled` feature |
| Structured errors | `thiserror` |
| Database migrations | `rusqlite_migration`, or numbered SQL files with `PRAGMA user_version` |
| CSV import (CIQUAL) | `csv`, if needed |
| Date handling | `chrono`, `time`, or `jiff`, from phase 5 |

`nutrition-core` should have as few dependencies as possible. Use the Rust standard library wherever it is sufficient. Select and pin compatible dependency versions when implementation begins.

## 3. SQLite

SQLite is suitable for a small local-first application because it requires no separate database server.

Advantages:

- A single database file.
- Transactions and relational constraints.
- Efficient queries and indexes.
- Straightforward backups and migrations.
- Good support for food-to-nutrient relationships.

Notes:

- Use foreign keys, schema migrations, and database constraints.
- Use the `bundled` feature of `rusqlite` so the SQLite version is the same on every platform, Android included.
- Food search: use FTS5 with diacritic-insensitive tokenization (`unicode61 remove_diacritics 2`), so that "creme" finds "crème". Plan for food names in French and English.
- Do not store calculated recipe totals as authoritative data unless there is a specific performance or historical-record requirement.

## 4. Frontend options

### Option A: Rust desktop GUI

Possible toolkit: `egui`.

Advantages:

- One main implementation language.
- Direct use of the Rust library.
- Simple packaging for a desktop prototype.

Disadvantages:

- Platform-specific packaging and interface considerations.
- May not be the best choice for a native mobile experience.

### Option B: Native Android

Suggested stack:

- Kotlin.
- Jetpack Compose.
- The Rust core exposed through UniFFI, which generates the Kotlin bindings (preferred over hand-written JNI).

Advantages:

- Native Android interface and platform integration.
- Reuse of the same Rust calculation engine.
- Good access to mobile storage and lifecycle facilities.

Disadvantages:

- Requires an integration boundary between Kotlin and Rust.
- Android-specific build and packaging work.

### Option C: Web frontend

Suggested stack:

- TypeScript.
- A lightweight web UI framework, if needed.
- A Rust backend exposed through a local or remote HTTP API.

Advantages:

- Broad frontend flexibility.
- Easy separation between presentation and domain logic.

Disadvantages:

- A local web server or another communication mechanism is required for a desktop application.
- A remote service introduces additional deployment and privacy concerns.

## 5. Recommended initial approach

Start with a Rust library and SQLite.

Do not build a network API or commit to a GUI toolkit before the core model and calculations are stable.

Initially, a small command-line interface and Rust integration tests validate the domain logic. Choose the final frontend based on the intended primary platform.

If Android is the main target, Kotlin with Jetpack Compose is the recommended frontend candidate.

## 6. Architectural constraints

- The domain model must not depend on the UI framework.
- Nutritional calculations must not depend directly on SQL queries.
- Storage must be replaceable without rewriting the calculation engine.
- Frontends must not duplicate nutritional formulas.
- The core must be testable without a graphical environment or network access.

## 7. Numerical representation

Integers are used as much as possible; floats only where required (see [DECISIONS.md](DECISIONS.md), D1).

- Quantities: integer grams.
- Nutrient amounts: integer fixed-point, 3 decimals of the canonical unit, stored as SQLite `INTEGER`.
- Aggregation: `i64`, with a single division by the reference quantity at the end (see [CALCULATION.md](CALCULATION.md) §2). Overflow is not a concern at realistic quantities.
- Floats (`f64`): energy equations, PAL, percentages of reference values, display conversions.

Round for display only. Tests involving floats use explicit tolerances.
