use rusqlite::{Connection, OptionalExtension, TransactionBehavior};

use crate::error::{sqlite, Error};

/// One schema migration: the schema version it produces and its SQL (D23).
#[derive(Clone)]
pub(crate) struct Migration {
    pub(crate) version: i64,
    pub(crate) sql: &'static str,
}

/// All migrations in order, embedded in the binary. Version `n` is at index `n - 1`.
pub(crate) const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        sql: include_str!("../migrations/0001_initial.sql"),
    },
    Migration {
        version: 2,
        sql: include_str!("../migrations/0002_food_names_and_search.sql"),
    },
];

/// Schema version produced by the last migration; the version this build supports.
pub const LATEST_SCHEMA_VERSION: i64 = MIGRATIONS[MIGRATIONS.len() - 1].version;

/// The schema version recorded in the database (`PRAGMA user_version`); 0 when empty.
///
/// # Arguments
///
/// * `conn` - open connection to the database.
pub(crate) fn user_version(conn: &Connection) -> Result<i64, Error> {
    conn.pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(sqlite("read the schema version"))
}

/// Applies the migrations newer than the database's version, each in its own
/// transaction together with its `user_version` update.
///
/// A database newer than the last migration is refused before anything is written.
/// Foreign keys are off while migrations run, so that a migration can rebuild a
/// table, and are checked before each commit; they are on when this returns.
///
/// # Arguments
///
/// * `conn` - open connection to the database, outside any transaction.
/// * `migrations` - migrations in increasing version order, numbered from 1 without gaps.
pub(crate) fn migrate(conn: &mut Connection, migrations: &[Migration]) -> Result<(), Error> {
    let supported = migrations.last().map_or(0, |m| m.version);
    // Read-only check first, so an up-to-date or newer database is never locked for writing.
    let found = user_version(conn)?;
    if found > supported {
        return Err(Error::SchemaTooNew { found, supported });
    }
    if found == supported {
        return Ok(());
    }
    // The pragma has no effect inside a transaction, so it is set around them.
    conn.pragma_update(None, "foreign_keys", false)
        .map_err(sqlite("disable foreign keys for the migrations"))?;
    let applied = apply_pending(conn, migrations, supported);
    let restored = conn
        .pragma_update(None, "foreign_keys", true)
        .map_err(sqlite("enable foreign keys after the migrations"));
    applied.and(restored)
}

/// Applies the pending migrations one transaction at a time, re-reading the
/// version inside each transaction.
///
/// The transactions are `IMMEDIATE`: the write lock is taken before the version
/// is read, so when two processes open a new database at the same time each
/// migration runs once.
///
/// # Arguments
///
/// * `conn` - open connection with foreign keys off, outside any transaction.
/// * `migrations` - migrations in increasing version order, numbered from 1 without gaps.
/// * `supported` - version of the last migration.
fn apply_pending(
    conn: &mut Connection,
    migrations: &[Migration],
    supported: i64,
) -> Result<(), Error> {
    loop {
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite("start a migration"))?;
        let found = user_version(&tx)?;
        if found > supported {
            return Err(Error::SchemaTooNew { found, supported });
        }
        let Some(migration) = migrations.iter().find(|m| m.version == found + 1) else {
            return Ok(());
        };
        let version = migration.version;
        let failed = |source| Error::Migration { version, source };
        tx.execute_batch(migration.sql).map_err(failed)?;
        let violation: Option<String> = tx
            .query_row("PRAGMA foreign_key_check", [], |row| row.get(0))
            .optional()
            .map_err(failed)?;
        if let Some(table) = violation {
            return Err(Error::MigrationBrokeForeignKeys { version, table });
        }
        tx.pragma_update(None, "user_version", version)
            .map_err(failed)?;
        tx.commit().map_err(failed)?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Migrations are numbered 1..=N without gaps, and the latest version is N.
    #[test]
    fn migrations_are_numbered_without_gaps() {
        for (index, migration) in MIGRATIONS.iter().enumerate() {
            assert_eq!(migration.version, index as i64 + 1);
        }
        assert_eq!(LATEST_SCHEMA_VERSION, MIGRATIONS.len() as i64);
    }

    /// Only the migrations newer than the database's version are applied.
    #[test]
    fn applies_only_missing_migrations() {
        let mut conn = Connection::open_in_memory().unwrap();
        let first = [Migration {
            version: 1,
            sql: "CREATE TABLE t (x INTEGER) STRICT; INSERT INTO t VALUES (1);",
        }];
        migrate(&mut conn, &first).unwrap();
        let both = [
            first[0].clone(),
            Migration {
                version: 2,
                sql: "INSERT INTO t VALUES (2);",
            },
        ];
        migrate(&mut conn, &both).unwrap();
        // Re-running version 1 would have inserted a second row with x = 1.
        let rows: i64 = conn
            .query_row("SELECT count(*) FROM t", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 2);
        assert_eq!(user_version(&conn).unwrap(), 2);
    }

    /// A migration that fails part way leaves the previous version and data untouched.
    #[test]
    fn failed_migration_is_rolled_back() {
        let mut conn = Connection::open_in_memory().unwrap();
        let migrations = [
            Migration {
                version: 1,
                sql: "CREATE TABLE t (x INTEGER) STRICT; INSERT INTO t VALUES (1);",
            },
            Migration {
                version: 2,
                sql: "INSERT INTO t VALUES (2); CREATE TABLE u (y); NOT VALID SQL;",
            },
        ];
        let err = migrate(&mut conn, &migrations).unwrap_err();
        assert!(matches!(err, Error::Migration { version: 2, .. }));
        assert_eq!(user_version(&conn).unwrap(), 1);
        let rows: i64 = conn
            .query_row("SELECT count(*) FROM t", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 1);
        let u_exists: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE name = 'u'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(u_exists, 0);
    }

    /// A database newer than the last migration is refused and left unchanged.
    #[test]
    fn refuses_newer_schema() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", 3).unwrap();
        let migrations = [Migration {
            version: 1,
            sql: "CREATE TABLE t (x INTEGER) STRICT;",
        }];
        let err = migrate(&mut conn, &migrations).unwrap_err();
        assert!(matches!(
            err,
            Error::SchemaTooNew {
                found: 3,
                supported: 1
            }
        ));
        assert_eq!(user_version(&conn).unwrap(), 3);
    }

    /// A migration can rebuild a parent table without its child rows being
    /// cascade-deleted, because foreign keys are off while migrating.
    #[test]
    fn table_rebuild_keeps_child_rows() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", true).unwrap();
        let first = Migration {
            version: 1,
            sql: "CREATE TABLE parent (id INTEGER PRIMARY KEY) STRICT;
                  CREATE TABLE child (parent_id INTEGER NOT NULL
                      REFERENCES parent (id) ON DELETE CASCADE) STRICT;
                  INSERT INTO parent VALUES (1);
                  INSERT INTO child VALUES (1);",
        };
        migrate(&mut conn, &[first.clone()]).unwrap();
        let rebuild = Migration {
            version: 2,
            sql: "CREATE TABLE parent_new (id INTEGER PRIMARY KEY, name TEXT) STRICT;
                  INSERT INTO parent_new (id) SELECT id FROM parent;
                  DROP TABLE parent;
                  ALTER TABLE parent_new RENAME TO parent;",
        };
        migrate(&mut conn, &[first, rebuild]).unwrap();
        let children: i64 = conn
            .query_row("SELECT count(*) FROM child", [], |r| r.get(0))
            .unwrap();
        assert_eq!(children, 1);
        let enabled: bool = conn
            .pragma_query_value(None, "foreign_keys", |r| r.get(0))
            .unwrap();
        assert!(enabled);
    }

    /// A migration that leaves a dangling foreign key is rolled back with the
    /// table named.
    #[test]
    fn migration_breaking_foreign_keys_is_rolled_back() {
        let mut conn = Connection::open_in_memory().unwrap();
        let first = Migration {
            version: 1,
            sql: "CREATE TABLE parent (id INTEGER PRIMARY KEY) STRICT;
                  CREATE TABLE child (parent_id INTEGER NOT NULL REFERENCES parent (id)) STRICT;",
        };
        let dangling = Migration {
            version: 2,
            sql: "INSERT INTO child VALUES (99);",
        };
        let err = migrate(&mut conn, &[first, dangling]).unwrap_err();
        assert!(matches!(
            err,
            Error::MigrationBrokeForeignKeys { version: 2, ref table } if table == "child"
        ));
        assert_eq!(user_version(&conn).unwrap(), 1);
        let children: i64 = conn
            .query_row("SELECT count(*) FROM child", [], |r| r.get(0))
            .unwrap();
        assert_eq!(children, 0);
    }
}
