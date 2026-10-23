use std::path::Path;

use nutrition_core::{Nutrient, Unit};
use rusqlite::Connection;

use crate::error::{sqlite, Error};
use crate::migrations::{migrate, user_version, MIGRATIONS};

/// A local SQLite database at the latest schema version, with foreign keys enforced (D23).
pub struct Database {
    pub(crate) conn: Connection,
}

impl Database {
    /// Opens the database file, creating it if missing, and applies the missing migrations.
    ///
    /// A database with a newer schema than this build supports is refused with
    /// [`Error::SchemaTooNew`] before anything is written to it.
    ///
    /// # Arguments
    ///
    /// * `path` - path of the SQLite database file.
    pub fn open(path: impl AsRef<Path>) -> Result<Database, Error> {
        let conn = Connection::open(path).map_err(sqlite("open the database file"))?;
        Database::init(conn)
    }

    /// Creates a new in-memory database at the latest schema version.
    pub fn open_in_memory() -> Result<Database, Error> {
        let conn = Connection::open_in_memory().map_err(sqlite("open an in-memory database"))?;
        Database::init(conn)
    }

    /// Migrates the connection, then enables foreign keys and checks they are on.
    ///
    /// # Arguments
    ///
    /// * `conn` - newly opened connection.
    fn init(mut conn: Connection) -> Result<Database, Error> {
        migrate(&mut conn, MIGRATIONS)?;
        enable_foreign_keys(&conn)?;
        Ok(Database { conn })
    }

    /// The schema version recorded in the database (`PRAGMA user_version`).
    pub fn schema_version(&self) -> Result<i64, Error> {
        user_version(&self.conn)
    }

    /// The nutrients in the database, in id order.
    pub fn nutrients(&self) -> Result<Vec<Nutrient>, Error> {
        let mut stmt = self
            .conn
            .prepare("SELECT code, name, canonical_unit FROM nutrient ORDER BY id")
            .map_err(sqlite("read the nutrients"))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(sqlite("read the nutrients"))?;
        rows.map(|row| {
            let (code, name, unit_code) = row.map_err(sqlite("read the nutrients"))?;
            let unit = unit_from_code(&unit_code).ok_or_else(|| Error::InvalidStoredData {
                table: "nutrient",
                detail: format!("unknown canonical unit {unit_code:?} for nutrient {code}"),
            })?;
            Ok(Nutrient::new(code, name, unit))
        })
        .collect()
    }
}

/// Turns on foreign key enforcement and checks that SQLite accepted it.
///
/// # Arguments
///
/// * `conn` - connection outside any transaction, where the pragma takes effect.
fn enable_foreign_keys(conn: &Connection) -> Result<(), Error> {
    conn.pragma_update(None, "foreign_keys", true)
        .map_err(sqlite("enable foreign keys"))?;
    let enabled: bool = conn
        .pragma_query_value(None, "foreign_keys", |row| row.get(0))
        .map_err(sqlite("check foreign keys"))?;
    if enabled {
        Ok(())
    } else {
        Err(Error::ForeignKeysDisabled)
    }
}

/// The canonical unit stored under `code` in the `nutrient` table (D14).
///
/// # Arguments
///
/// * `code` - unit code as stored: `g`, `mg`, `ug`, `kJ`, or `kcal`.
fn unit_from_code(code: &str) -> Option<Unit> {
    match code {
        "g" => Some(Unit::Gram),
        "mg" => Some(Unit::Milligram),
        "ug" => Some(Unit::Microgram),
        "kJ" => Some(Unit::Kilojoule),
        "kcal" => Some(Unit::Kilocalorie),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stored unit outside the known codes is reported as invalid data, not a panic.
    #[test]
    fn unknown_stored_unit_is_invalid_data() {
        let db = Database::open_in_memory().unwrap();
        // Bypass the CHECK constraint to simulate a corrupted row.
        db.conn
            .execute_batch(
                "PRAGMA ignore_check_constraints = ON;
                 UPDATE nutrient SET canonical_unit = 'oz' WHERE code = 'protein';",
            )
            .unwrap();
        let err = db.nutrients().unwrap_err();
        assert!(matches!(
            err,
            Error::InvalidStoredData { table: "nutrient", ref detail } if detail.contains("oz")
        ));
    }

    /// Every stored unit code maps to its canonical unit (D14).
    #[test]
    fn maps_unit_codes() {
        assert_eq!(unit_from_code("g"), Some(Unit::Gram));
        assert_eq!(unit_from_code("mg"), Some(Unit::Milligram));
        assert_eq!(unit_from_code("ug"), Some(Unit::Microgram));
        assert_eq!(unit_from_code("kJ"), Some(Unit::Kilojoule));
        assert_eq!(unit_from_code("kcal"), Some(Unit::Kilocalorie));
        assert_eq!(unit_from_code("µg"), None);
    }

    /// Foreign key enforcement is on after opening.
    #[test]
    fn foreign_keys_are_enabled() {
        let db = Database::open_in_memory().unwrap();
        let enabled: bool = db
            .conn
            .pragma_query_value(None, "foreign_keys", |row| row.get(0))
            .unwrap();
        assert!(enabled);
    }
}
