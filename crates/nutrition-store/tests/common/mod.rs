//! Shared fixtures for the `nutrition-store` schema tests.
//!
//! Each constraint test creates its own database file through
//! `Database::open`, so that the migrations under test build the schema, then
//! inspects it through a raw `rusqlite::Connection` with foreign keys enabled.
//! All rows are synthetic (D13).

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use nutrition_store::Database;
use rusqlite::{params, Connection, ErrorCode};

/// A database file in the system temporary directory, removed on drop
/// together with its journal files.
pub struct TempDbFile {
    path: PathBuf,
}

impl TempDbFile {
    /// A unique, not yet existing path for the test `name`; any leftover file
    /// from a previous run is removed first.
    ///
    /// # Arguments
    ///
    /// * `name` - the test name, unique within the test binary.
    pub fn new(name: &str) -> TempDbFile {
        let path = std::env::temp_dir().join(format!(
            "nutrition-store-test-{}-{}.sqlite3",
            std::process::id(),
            name
        ));
        let file = TempDbFile { path };
        file.remove();
        file
    }

    /// The path of the database file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Removes the database file and its journal files, ignoring missing ones.
    fn remove(&self) {
        for suffix in ["", "-journal", "-wal", "-shm"] {
            let mut os = self.path.clone().into_os_string();
            os.push(suffix);
            let _ = std::fs::remove_file(PathBuf::from(os));
        }
    }
}

impl Drop for TempDbFile {
    /// Removes the file when the test ends, whether it passed or not.
    fn drop(&mut self) {
        self.remove();
    }
}

/// Creates a migrated database file with `Database::open`, closes it, and
/// returns the file with a raw connection to it that enforces foreign keys.
///
/// # Arguments
///
/// * `name` - the test name, unique within the test binary.
pub fn migrated_db(name: &str) -> (TempDbFile, Connection) {
    let file = TempDbFile::new(name);
    let db = Database::open(file.path()).expect("Database::open on a new file");
    drop(db);
    let conn = raw_connection(file.path());
    (file, conn)
}

/// A raw connection to `path` with `PRAGMA foreign_keys = ON`.
///
/// # Arguments
///
/// * `path` - the database file.
pub fn raw_connection(path: &Path) -> Connection {
    let conn = Connection::open(path).expect("raw open");
    conn.pragma_update(None, "foreign_keys", true)
        .expect("enable foreign keys");
    let on: i64 = conn
        .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
        .expect("read foreign_keys");
    assert_eq!(on, 1, "foreign keys must be enforced on the raw connection");
    conn
}

/// Asserts that `sql` fails with an SQLite constraint violation (CHECK,
/// NOT NULL, UNIQUE, FOREIGN KEY, STRICT type, or trigger RAISE).
///
/// # Arguments
///
/// * `conn` - the connection to run the statement on.
/// * `sql` - one SQL statement expected to break a constraint.
pub fn assert_rejected(conn: &Connection, sql: &str) {
    match conn.execute_batch(sql) {
        Ok(()) => panic!("statement was accepted but should be rejected: {sql}"),
        Err(rusqlite::Error::SqliteFailure(e, msg)) => assert_eq!(
            e.code,
            ErrorCode::ConstraintViolation,
            "expected a constraint violation for {sql}, got {e:?} {msg:?}"
        ),
        Err(other) => panic!("expected a constraint violation for {sql}, got {other:?}"),
    }
}

/// Asserts that `sql` succeeds.
///
/// # Arguments
///
/// * `conn` - the connection to run the statement on.
/// * `sql` - one or more SQL statements expected to be valid.
pub fn assert_accepted(conn: &Connection, sql: &str) {
    if let Err(e) = conn.execute_batch(sql) {
        panic!("statement was rejected but should be accepted: {sql}: {e:?}");
    }
}

/// Runs a single-integer query, e.g. `SELECT count(*) ...`.
///
/// # Arguments
///
/// * `conn` - the connection to query.
/// * `sql` - a query returning one row with one integer column.
pub fn query_i64(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |r| r.get(0))
        .unwrap_or_else(|e| panic!("query failed: {sql}: {e:?}"))
}

/// The id of the seeded `user` data source (D25).
///
/// # Arguments
///
/// * `conn` - the connection to query.
pub fn user_source(conn: &Connection) -> i64 {
    query_i64(conn, "SELECT id FROM data_source WHERE name = 'user'")
}

/// Inserts a data source and returns its id.
///
/// # Arguments
///
/// * `conn` - the connection to write to.
/// * `name` - the source name, non-empty and unique.
pub fn insert_source(conn: &Connection, name: &str) -> i64 {
    conn.execute("INSERT INTO data_source (name) VALUES (?1)", params![name])
        .expect("insert data_source");
    conn.last_insert_rowid()
}

/// Inserts a food and returns its id.
///
/// # Arguments
///
/// * `conn` - the connection to write to.
/// * `source_id` - id of an existing data source.
/// * `name` - the food name, non-empty.
pub fn insert_food(conn: &Connection, source_id: i64, name: &str) -> i64 {
    conn.execute(
        "INSERT INTO food (source_id, name) VALUES (?1, ?2)",
        params![source_id, name],
    )
    .expect("insert food");
    conn.last_insert_rowid()
}

/// Inserts a composition with the default reference quantity and returns its id.
///
/// # Arguments
///
/// * `conn` - the connection to write to.
/// * `food_id` - id of an existing food.
/// * `source_id` - the food's data source id.
/// * `state` - `raw`, `cooked`, or `shared` (D28).
/// * `record` - the source record id, `None` for user foods (D2, D25).
pub fn insert_composition(
    conn: &Connection,
    food_id: i64,
    source_id: i64,
    state: &str,
    record: Option<&str>,
) -> i64 {
    conn.execute(
        "INSERT INTO food_composition (food_id, source_id, state, source_record_id) \
         VALUES (?1, ?2, ?3, ?4)",
        params![food_id, source_id, state, record],
    )
    .expect("insert food_composition");
    conn.last_insert_rowid()
}

/// The id of the nutrient with `code`.
///
/// # Arguments
///
/// * `conn` - the connection to query.
/// * `code` - a nutrient code from the seed set (D14).
pub fn nutrient_id(conn: &Connection, code: &str) -> i64 {
    conn.query_row(
        "SELECT id FROM nutrient WHERE code = ?1",
        params![code],
        |r| r.get(0),
    )
    .expect("nutrient id")
}

/// Inserts a recipe with one serving and returns its id.
///
/// # Arguments
///
/// * `conn` - the connection to write to.
pub fn insert_recipe(conn: &Connection) -> i64 {
    conn.execute(
        "INSERT INTO recipe (name, servings) VALUES ('Synthetic recipe', 1)",
        [],
    )
    .expect("insert recipe");
    conn.last_insert_rowid()
}
