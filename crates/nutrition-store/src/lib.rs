//! SQLite persistence for the Meal Nutrient Estimator.
//!
//! Translates between database rows and the `nutrition-core` domain types; no
//! calculation happens here. The schema is created and upgraded by numbered SQL
//! migrations tracked with `PRAGMA user_version` (DECISIONS.md, D23).

mod database;
mod error;
mod migrations;

pub use database::Database;
pub use error::Error;
pub use migrations::LATEST_SCHEMA_VERSION;
