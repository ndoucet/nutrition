/// Errors of the persistence layer (ARCHITECTURE.md §5: database failures).
#[derive(Debug, thiserror::Error)]
pub enum Error {
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
