use nutrition_core::FoodId;

/// Errors of the persistence layer (ARCHITECTURE.md §5): invalid input, an
/// unknown food, data source, or nutrient, a food still in use, and database
/// failures.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid input: {detail}")]
    InvalidInput { detail: String },

    #[error("data source {name:?} does not exist")]
    UnknownDataSource { name: String },

    #[error("data source #{source_id} does not exist")]
    UnknownDataSourceId { source_id: i64 },

    #[error("data source {name:?} already exists")]
    DuplicateDataSource { name: String },

    #[error("nutrient {code} is not in the database")]
    UnknownNutrient { code: String },

    #[error("food {food_id} does not exist")]
    FoodNotFound { food_id: FoodId },

    #[error("food {food_id} is used by a recipe and cannot be deleted")]
    FoodInUse { food_id: FoodId },

    #[error("record {source_record_id:?} of data source #{source_id} is already stored")]
    DuplicateSourceRecord {
        source_id: i64,
        source_record_id: String,
    },

    #[error("database error while trying to {operation}")]
    Sqlite {
        operation: &'static str,
        #[source]
        source: rusqlite::Error,
    },

    #[error("migration to schema version {version} failed")]
    Migration {
        version: i64,
        #[source]
        source: rusqlite::Error,
    },

    #[error(
        "migration to schema version {version} left rows in {table} with a dangling foreign key"
    )]
    MigrationBrokeForeignKeys { version: i64, table: String },

    #[error(
        "the database has schema version {found}, newer than the supported version {supported}"
    )]
    SchemaTooNew { found: i64, supported: i64 },

    #[error("SQLite did not enable foreign key enforcement")]
    ForeignKeysDisabled,

    #[error("invalid data in table {table}: {detail}")]
    InvalidStoredData { table: &'static str, detail: String },
}

/// Wraps a `rusqlite` error with the operation that failed, for use with `map_err`.
///
/// # Arguments
///
/// * `operation` - what was being done, phrased to follow "while trying to", e.g. `read the nutrients`.
pub(crate) fn sqlite(operation: &'static str) -> impl FnOnce(rusqlite::Error) -> Error {
    move |source| Error::Sqlite { operation, source }
}
