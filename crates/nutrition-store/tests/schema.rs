//! Schema version 1 tests: migrations, seed data, and every constraint and
//! trigger of `migrations/0001_initial.sql`.
//!
//! Expected behaviour comes from docs/DATA_MODEL.md §2–7 and §11 and
//! docs/DECISIONS.md (D1, D2, D8, D9, D14, D17, D18, D23, D25, D28, D29), not
//! from the implementation. Constraint tests build the schema with
//! `Database::open` and probe it through a raw connection (see `common`).

mod common;

use common::*;
use nutrition_core::initial_nutrients;
use nutrition_store::{Database, Error, LATEST_SCHEMA_VERSION};
use rusqlite::Connection;

// ---------------------------------------------------------------------------
// Migrations (D23)
// ---------------------------------------------------------------------------

/// Schema version 1 is the latest one for now.
#[test]
fn latest_schema_version_is_one() {
    assert_eq!(LATEST_SCHEMA_VERSION, 1);
}

/// An in-memory database is migrated to the latest version on open.
#[test]
fn open_in_memory_reaches_latest_schema_version() {
    let db = Database::open_in_memory().unwrap();
    assert_eq!(db.schema_version().unwrap(), LATEST_SCHEMA_VERSION);
}

/// A new file is migrated to the latest version and `user_version` is stored in it.
#[test]
fn open_new_file_stores_latest_user_version() {
    let file = TempDbFile::new("open_new_file_stores_latest_user_version");
    let db = Database::open(file.path()).unwrap();
    assert_eq!(db.schema_version().unwrap(), LATEST_SCHEMA_VERSION);
    drop(db);
    let conn = raw_connection(file.path());
    assert_eq!(
        query_i64(&conn, "PRAGMA user_version"),
        LATEST_SCHEMA_VERSION
    );
}

/// Reopening a migrated file applies nothing, keeps user data, and does not
/// insert the seed rows twice.
#[test]
fn reopening_migrated_file_is_a_no_op_and_keeps_data() {
    let (file, conn) = migrated_db("reopening_migrated_file_is_a_no_op_and_keeps_data");
    let food = insert_food(&conn, user_source(&conn), "Synthetic lentil");
    drop(conn);

    let db = Database::open(file.path()).unwrap();
    assert_eq!(db.schema_version().unwrap(), LATEST_SCHEMA_VERSION);
    assert_eq!(db.nutrients().unwrap(), initial_nutrients());
    drop(db);

    let conn = raw_connection(file.path());
    assert_eq!(
        query_i64(
            &conn,
            &format!("SELECT count(*) FROM food WHERE id = {food} AND name = 'Synthetic lentil'")
        ),
        1
    );
    // Seed data inserted once (D29): one 'user' source, 14 nutrients (D7).
    assert_eq!(
        query_i64(
            &conn,
            "SELECT count(*) FROM data_source WHERE name = 'user'"
        ),
        1
    );
    assert_eq!(query_i64(&conn, "SELECT count(*) FROM nutrient"), 14);
}

/// A file from a newer application (user_version = LATEST + 1) is refused with
/// `SchemaTooNew` and its `user_version` is left unchanged.
#[test]
fn open_refuses_newer_schema_and_leaves_version_unchanged() {
    let file = TempDbFile::new("open_refuses_newer_schema_and_leaves_version_unchanged");
    {
        let conn = Connection::open(file.path()).unwrap();
        conn.pragma_update(None, "user_version", LATEST_SCHEMA_VERSION + 1)
            .unwrap();
    }

    match Database::open(file.path()) {
        Err(Error::SchemaTooNew { found, supported }) => {
            assert_eq!(found, LATEST_SCHEMA_VERSION + 1);
            assert_eq!(supported, LATEST_SCHEMA_VERSION);
        }
        Err(other) => panic!("expected SchemaTooNew, got {other:?}"),
        Ok(_) => panic!("expected SchemaTooNew, got a database"),
    }

    let conn = Connection::open(file.path()).unwrap();
    assert_eq!(
        query_i64(&conn, "PRAGMA user_version"),
        LATEST_SCHEMA_VERSION + 1
    );
    // No migration ran: the schema is still empty.
    assert_eq!(query_i64(&conn, "SELECT count(*) FROM sqlite_schema"), 0);
}

/// After open, `PRAGMA integrity_check` reports "ok" and
/// `PRAGMA foreign_key_check` reports no violation.
#[test]
fn migrated_database_passes_integrity_and_foreign_key_checks() {
    let (_file, conn) = migrated_db("migrated_database_passes_integrity_and_foreign_key_checks");
    let integrity: String = conn
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
    let mut stmt = conn.prepare("PRAGMA foreign_key_check").unwrap();
    let violations = stmt.query_map([], |_| Ok(())).unwrap().count();
    assert_eq!(violations, 0);
}

// ---------------------------------------------------------------------------
// Seed data (D7, D14, D25, D29)
// ---------------------------------------------------------------------------

/// `nutrients()` returns exactly `initial_nutrients()`: same order, codes,
/// names, and units.
#[test]
fn nutrients_equal_initial_nutrients() {
    let db = Database::open_in_memory().unwrap();
    assert_eq!(db.nutrients().unwrap(), initial_nutrients());
}

/// The seed stores units under the codes g, mg, ug, kJ, kcal (D14).
#[test]
fn seeded_nutrients_use_canonical_unit_codes() {
    let (_file, conn) = migrated_db("seeded_nutrients_use_canonical_unit_codes");
    let mut stmt = conn
        .prepare("SELECT code, canonical_unit FROM nutrient WHERE code IN ('energy_kj', 'energy_kcal', 'protein', 'calcium', 'vitamin_d') ORDER BY id")
        .unwrap();
    let rows: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let expected = [
        ("energy_kj", "kJ"),
        ("energy_kcal", "kcal"),
        ("protein", "g"),
        ("calcium", "mg"),
        ("vitamin_d", "ug"),
    ];
    let expected: Vec<(String, String)> = expected
        .iter()
        .map(|(c, u)| (c.to_string(), u.to_string()))
        .collect();
    assert_eq!(rows, expected);
}

/// The data source of user-created foods, named "user", exists (D25).
#[test]
fn user_data_source_exists() {
    let (_file, conn) = migrated_db("user_data_source_exists");
    assert_eq!(
        query_i64(
            &conn,
            "SELECT count(*) FROM data_source WHERE name = 'user'"
        ),
        1
    );
}

// ---------------------------------------------------------------------------
// Names and codes
// ---------------------------------------------------------------------------

/// Empty names and codes are rejected in every table that has one.
#[test]
fn empty_names_are_rejected() {
    let (_file, conn) = migrated_db("empty_names_are_rejected");
    let user = user_source(&conn);
    assert_rejected(&conn, "INSERT INTO data_source (name) VALUES ('')");
    assert_accepted(
        &conn,
        "INSERT INTO data_source (name) VALUES ('Synthetic source')",
    );
    assert_rejected(
        &conn,
        "INSERT INTO nutrient (code, name, canonical_unit) VALUES ('', 'X', 'g')",
    );
    assert_rejected(
        &conn,
        "INSERT INTO nutrient (code, name, canonical_unit) VALUES ('x_code', '', 'g')",
    );
    assert_accepted(
        &conn,
        "INSERT INTO nutrient (code, name, canonical_unit) VALUES ('x_code', 'X', 'g')",
    );
    assert_rejected(
        &conn,
        &format!("INSERT INTO food (source_id, name) VALUES ({user}, '')"),
    );
    assert_accepted(
        &conn,
        &format!("INSERT INTO food (source_id, name) VALUES ({user}, 'Synthetic food')"),
    );
    assert_rejected(&conn, "INSERT INTO recipe (name, servings) VALUES ('', 1)");
    assert_accepted(
        &conn,
        "INSERT INTO recipe (name, servings) VALUES ('Synthetic recipe', 1)",
    );
}

/// Missing names (NULL) are rejected.
#[test]
fn null_names_are_rejected() {
    let (_file, conn) = migrated_db("null_names_are_rejected");
    let user = user_source(&conn);
    assert_rejected(&conn, "INSERT INTO data_source (name) VALUES (NULL)");
    assert_rejected(
        &conn,
        "INSERT INTO nutrient (code, name, canonical_unit) VALUES (NULL, 'X', 'g')",
    );
    assert_rejected(
        &conn,
        &format!("INSERT INTO food (source_id, name) VALUES ({user}, NULL)"),
    );
    assert_rejected(
        &conn,
        "INSERT INTO recipe (name, servings) VALUES (NULL, 1)",
    );
}

/// Two data sources cannot share a name.
#[test]
fn duplicate_data_source_name_is_rejected() {
    let (_file, conn) = migrated_db("duplicate_data_source_name_is_rejected");
    assert_rejected(&conn, "INSERT INTO data_source (name) VALUES ('user')");
    assert_accepted(
        &conn,
        "INSERT INTO data_source (name) VALUES ('Synthetic source')",
    );
}

/// Two nutrients cannot share a code (DATA_MODEL §11).
#[test]
fn duplicate_nutrient_code_is_rejected() {
    let (_file, conn) = migrated_db("duplicate_nutrient_code_is_rejected");
    assert_rejected(&conn, "INSERT INTO nutrient (code, name, canonical_unit) VALUES ('protein', 'Protein again', 'g')");
    // Same name, different code is a different nutrient (DATA_MODEL §2).
    assert_accepted(&conn, "INSERT INTO nutrient (code, name, canonical_unit) VALUES ('protein_other', 'Protein', 'g')");
}

/// A canonical unit outside g, mg, ug, kJ, kcal is rejected (D14), including
/// the display symbol µg and a different case.
#[test]
fn nutrient_unit_outside_the_set_is_rejected() {
    let (_file, conn) = migrated_db("nutrient_unit_outside_the_set_is_rejected");
    for unit in ["kg", "µg", "mcg", "KJ", "G", "", "IU"] {
        assert_rejected(
            &conn,
            &format!("INSERT INTO nutrient (code, name, canonical_unit) VALUES ('bad_{unit}', 'Bad', '{unit}')"),
        );
    }
    for unit in ["g", "mg", "ug", "kJ", "kcal"] {
        assert_accepted(
            &conn,
            &format!("INSERT INTO nutrient (code, name, canonical_unit) VALUES ('good_{unit}', 'Good', '{unit}')"),
        );
    }
}

// ---------------------------------------------------------------------------
// Foreign keys
// ---------------------------------------------------------------------------

/// A food must reference an existing data source.
#[test]
fn food_with_unknown_source_is_rejected() {
    let (_file, conn) = migrated_db("food_with_unknown_source_is_rejected");
    assert_rejected(
        &conn,
        "INSERT INTO food (source_id, name) VALUES (9999, 'Orphan')",
    );
    let user = user_source(&conn);
    assert_accepted(
        &conn,
        &format!("INSERT INTO food (source_id, name) VALUES ({user}, 'Synthetic food')"),
    );
}

/// A composition must repeat its food's source id (D2): a different existing
/// source is rejected, the food's own source is accepted.
#[test]
fn composition_with_source_other_than_its_food_is_rejected() {
    let (_file, conn) = migrated_db("composition_with_source_other_than_its_food_is_rejected");
    let user = user_source(&conn);
    let other = insert_source(&conn, "Synthetic source");
    let food = insert_food(&conn, user, "Synthetic food");
    assert_rejected(
        &conn,
        &format!("INSERT INTO food_composition (food_id, source_id, state) VALUES ({food}, {other}, 'raw')"),
    );
    assert_accepted(
        &conn,
        &format!("INSERT INTO food_composition (food_id, source_id, state) VALUES ({food}, {user}, 'raw')"),
    );
}

/// A composition must reference an existing food.
#[test]
fn composition_with_unknown_food_is_rejected() {
    let (_file, conn) = migrated_db("composition_with_unknown_food_is_rejected");
    let user = user_source(&conn);
    assert_rejected(
        &conn,
        &format!(
            "INSERT INTO food_composition (food_id, source_id, state) VALUES (9999, {user}, 'raw')"
        ),
    );
}

/// A nutrient value must reference an existing nutrient and composition.
#[test]
fn value_with_unknown_nutrient_or_composition_is_rejected() {
    let (_file, conn) = migrated_db("value_with_unknown_nutrient_or_composition_is_rejected");
    let user = user_source(&conn);
    let food = insert_food(&conn, user, "Synthetic food");
    let comp = insert_composition(&conn, food, user, "raw", None);
    let protein = nutrient_id(&conn, "protein");
    assert_rejected(
        &conn,
        &format!("INSERT INTO food_nutrient_value VALUES ({comp}, 9999, 'known', 1000)"),
    );
    assert_rejected(
        &conn,
        &format!("INSERT INTO food_nutrient_value VALUES (9999, {protein}, 'known', 1000)"),
    );
    assert_accepted(
        &conn,
        &format!("INSERT INTO food_nutrient_value VALUES ({comp}, {protein}, 'known', 1000)"),
    );
}

/// A recipe item must reference an existing recipe and food.
#[test]
fn recipe_item_with_unknown_recipe_or_food_is_rejected() {
    let (_file, conn) = migrated_db("recipe_item_with_unknown_recipe_or_food_is_rejected");
    let food = insert_food(&conn, user_source(&conn), "Synthetic food");
    let recipe = insert_recipe(&conn);
    assert_rejected(
        &conn,
        &format!("INSERT INTO recipe_item VALUES ({recipe}, 0, 9999, 'raw', 100)"),
    );
    assert_rejected(
        &conn,
        &format!("INSERT INTO recipe_item VALUES (9999, 0, {food}, 'raw', 100)"),
    );
    assert_accepted(
        &conn,
        &format!("INSERT INTO recipe_item VALUES ({recipe}, 0, {food}, 'raw', 100)"),
    );
}

// ---------------------------------------------------------------------------
// Food composition (D2, D9, D28)
// ---------------------------------------------------------------------------

/// A state outside raw, cooked, shared is rejected.
#[test]
fn composition_state_outside_the_set_is_rejected() {
    let (_file, conn) = migrated_db("composition_state_outside_the_set_is_rejected");
    let user = user_source(&conn);
    let food = insert_food(&conn, user, "Synthetic food");
    for state in ["fried", "RAW", "", "both"] {
        assert_rejected(
            &conn,
            &format!("INSERT INTO food_composition (food_id, source_id, state) VALUES ({food}, {user}, '{state}')"),
        );
    }
    assert_rejected(
        &conn,
        &format!("INSERT INTO food_composition (food_id, source_id, state) VALUES ({food}, {user}, NULL)"),
    );
    assert_accepted(
        &conn,
        &format!("INSERT INTO food_composition (food_id, source_id, state) VALUES ({food}, {user}, 'raw')"),
    );
}

/// A food has at most one composition per state; raw and cooked together are accepted.
#[test]
fn duplicate_food_state_is_rejected() {
    let (_file, conn) = migrated_db("duplicate_food_state_is_rejected");
    let user = user_source(&conn);
    let food = insert_food(&conn, user, "Synthetic food");
    insert_composition(&conn, food, user, "raw", None);
    assert_rejected(
        &conn,
        &format!("INSERT INTO food_composition (food_id, source_id, state) VALUES ({food}, {user}, 'raw')"),
    );
    assert_accepted(
        &conn,
        &format!("INSERT INTO food_composition (food_id, source_id, state) VALUES ({food}, {user}, 'cooked')"),
    );
}

/// A 'shared' composition is rejected when the food already has a raw one.
#[test]
fn shared_after_raw_is_rejected() {
    let (_file, conn) = migrated_db("shared_after_raw_is_rejected");
    let user = user_source(&conn);
    let food = insert_food(&conn, user, "Synthetic food");
    insert_composition(&conn, food, user, "raw", None);
    assert_rejected(
        &conn,
        &format!("INSERT INTO food_composition (food_id, source_id, state) VALUES ({food}, {user}, 'shared')"),
    );
    // Good row: a shared composition alone on another food.
    let alone = insert_food(&conn, user, "Synthetic shared food");
    assert_accepted(
        &conn,
        &format!("INSERT INTO food_composition (food_id, source_id, state) VALUES ({alone}, {user}, 'shared')"),
    );
}

/// A raw or cooked composition is rejected when the food already has a shared one.
#[test]
fn raw_or_cooked_after_shared_is_rejected() {
    let (_file, conn) = migrated_db("raw_or_cooked_after_shared_is_rejected");
    let user = user_source(&conn);
    let food = insert_food(&conn, user, "Synthetic food");
    insert_composition(&conn, food, user, "shared", None);
    for state in ["raw", "cooked"] {
        assert_rejected(
            &conn,
            &format!("INSERT INTO food_composition (food_id, source_id, state) VALUES ({food}, {user}, '{state}')"),
        );
    }
    assert_eq!(
        query_i64(
            &conn,
            &format!("SELECT count(*) FROM food_composition WHERE food_id = {food}")
        ),
        1
    );
}

/// Re-importing a food's lone 'shared' composition with an upsert on
/// `(food_id, state)` is accepted: the shared-state trigger ignores the row
/// being replaced, which the UNIQUE constraint handles (D2, D28).
#[test]
fn upsert_of_existing_shared_composition_is_accepted() {
    let (_file, conn) = migrated_db("upsert_of_existing_shared_composition_is_accepted");
    let user = user_source(&conn);
    let food = insert_food(&conn, user, "Synthetic food");
    insert_composition(&conn, food, user, "shared", None);
    assert_accepted(
        &conn,
        &format!(
            "INSERT INTO food_composition (food_id, source_id, state, reference_quantity_g) \
             VALUES ({food}, {user}, 'shared', 100) \
             ON CONFLICT (food_id, state) DO UPDATE SET reference_quantity_g = excluded.reference_quantity_g"
        ),
    );
    assert_eq!(
        query_i64(
            &conn,
            &format!("SELECT count(*) FROM food_composition WHERE food_id = {food}")
        ),
        1
    );
}

/// Changing a composition's state to 'shared' is rejected when the food has
/// another composition, and accepted when it is the only one.
#[test]
fn update_to_shared_beside_another_composition_is_rejected() {
    let (_file, conn) = migrated_db("update_to_shared_beside_another_composition_is_rejected");
    let user = user_source(&conn);
    let both = insert_food(&conn, user, "Synthetic two-state food");
    insert_composition(&conn, both, user, "raw", None);
    let cooked = insert_composition(&conn, both, user, "cooked", None);
    assert_rejected(
        &conn,
        &format!("UPDATE food_composition SET state = 'shared' WHERE id = {cooked}"),
    );

    let single = insert_food(&conn, user, "Synthetic one-state food");
    let raw = insert_composition(&conn, single, user, "raw", None);
    assert_accepted(
        &conn,
        &format!("UPDATE food_composition SET state = 'shared' WHERE id = {raw}"),
    );
}

/// Updating the only composition of a food, which is shared, is not blocked
/// by the trigger: the row does not conflict with itself.
#[test]
fn updating_the_lone_shared_composition_is_accepted() {
    let (_file, conn) = migrated_db("updating_the_lone_shared_composition_is_accepted");
    let user = user_source(&conn);
    let food = insert_food(&conn, user, "Synthetic food");
    let shared = insert_composition(&conn, food, user, "shared", None);
    assert_accepted(
        &conn,
        &format!("UPDATE food_composition SET state = 'shared' WHERE id = {shared}"),
    );
    assert_accepted(
        &conn,
        &format!("UPDATE food_composition SET state = 'raw' WHERE id = {shared}"),
    );
}

/// Moving a composition to a food that has a shared composition is rejected.
#[test]
fn update_food_id_onto_food_with_shared_is_rejected() {
    let (_file, conn) = migrated_db("update_food_id_onto_food_with_shared_is_rejected");
    let user = user_source(&conn);
    let a = insert_food(&conn, user, "Synthetic food A");
    let b = insert_food(&conn, user, "Synthetic food B");
    let c = insert_food(&conn, user, "Synthetic food C");
    let a_raw = insert_composition(&conn, a, user, "raw", None);
    insert_composition(&conn, b, user, "shared", None);
    assert_rejected(
        &conn,
        &format!("UPDATE food_composition SET food_id = {b} WHERE id = {a_raw}"),
    );
    // Good row: moving onto a food without compositions.
    assert_accepted(
        &conn,
        &format!("UPDATE food_composition SET food_id = {c} WHERE id = {a_raw}"),
    );
}

/// A source record id is unique within a source (D2), but several
/// compositions may have none (user foods, D25).
#[test]
fn duplicate_source_record_id_is_rejected_but_null_is_repeatable() {
    let (_file, conn) =
        migrated_db("duplicate_source_record_id_is_rejected_but_null_is_repeatable");
    let src = insert_source(&conn, "Synthetic source");
    let other = insert_source(&conn, "Other synthetic source");
    let f1 = insert_food(&conn, src, "Synthetic food 1");
    let f2 = insert_food(&conn, src, "Synthetic food 2");
    let f3 = insert_food(&conn, other, "Synthetic food 3");
    insert_composition(&conn, f1, src, "raw", Some("R1"));
    assert_rejected(
        &conn,
        &format!("INSERT INTO food_composition (food_id, source_id, state, source_record_id) VALUES ({f2}, {src}, 'raw', 'R1')"),
    );
    // Same record id in another source is a different natural key.
    assert_accepted(
        &conn,
        &format!("INSERT INTO food_composition (food_id, source_id, state, source_record_id) VALUES ({f3}, {other}, 'raw', 'R1')"),
    );
    // Two NULL record ids in the same source.
    let user = user_source(&conn);
    let u1 = insert_food(&conn, user, "Synthetic user food 1");
    let u2 = insert_food(&conn, user, "Synthetic user food 2");
    insert_composition(&conn, u1, user, "raw", None);
    assert_accepted(
        &conn,
        &format!("INSERT INTO food_composition (food_id, source_id, state, source_record_id) VALUES ({u2}, {user}, 'raw', NULL)"),
    );
}

/// An empty source record id is rejected (absent must be NULL).
#[test]
fn empty_source_record_id_is_rejected() {
    let (_file, conn) = migrated_db("empty_source_record_id_is_rejected");
    let user = user_source(&conn);
    let food = insert_food(&conn, user, "Synthetic food");
    assert_rejected(
        &conn,
        &format!("INSERT INTO food_composition (food_id, source_id, state, source_record_id) VALUES ({food}, {user}, 'raw', '')"),
    );
}

/// The reference quantity defaults to 100 g (DATA_MODEL §3).
#[test]
fn reference_quantity_defaults_to_100_g() {
    let (_file, conn) = migrated_db("reference_quantity_defaults_to_100_g");
    let user = user_source(&conn);
    let food = insert_food(&conn, user, "Synthetic food");
    let comp = insert_composition(&conn, food, user, "raw", None);
    assert_eq!(
        query_i64(
            &conn,
            &format!("SELECT reference_quantity_g FROM food_composition WHERE id = {comp}")
        ),
        100
    );
}

/// The reference quantity is a u32 gram count of at least 1 (core `Grams`, DATA_MODEL §11): 0 and
/// 4294967296 (u32::MAX + 1) are rejected; 1 and 4294967295 are accepted.
#[test]
fn reference_quantity_outside_1_to_u32_max_is_rejected() {
    let (_file, conn) = migrated_db("reference_quantity_outside_1_to_u32_max_is_rejected");
    let user = user_source(&conn);
    for (i, q) in ["0", "-1", "4294967296"].iter().enumerate() {
        let food = insert_food(&conn, user, &format!("Synthetic bad {i}"));
        assert_rejected(
            &conn,
            &format!("INSERT INTO food_composition (food_id, source_id, state, reference_quantity_g) VALUES ({food}, {user}, 'raw', {q})"),
        );
    }
    for (i, q) in ["1", "4294967295"].iter().enumerate() {
        let food = insert_food(&conn, user, &format!("Synthetic good {i}"));
        assert_accepted(
            &conn,
            &format!("INSERT INTO food_composition (food_id, source_id, state, reference_quantity_g) VALUES ({food}, {user}, 'raw', {q})"),
        );
    }
}

// ---------------------------------------------------------------------------
// Food nutrient values (D1, D8, D18)
// ---------------------------------------------------------------------------

/// Fixture for value tests: a connection, one composition id, and the ids of
/// distinct nutrients so that each accepted row has its own primary key.
///
/// # Arguments
///
/// * `name` - the test name, unique within the test binary.
fn value_fixture(name: &str) -> (TempDbFile, Connection, i64, Vec<i64>) {
    let (file, conn) = migrated_db(name);
    let user = user_source(&conn);
    let food = insert_food(&conn, user, "Synthetic food");
    let comp = insert_composition(&conn, food, user, "raw", None);
    let mut stmt = conn.prepare("SELECT id FROM nutrient ORDER BY id").unwrap();
    let ids: Vec<i64> = stmt
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    drop(stmt);
    (file, conn, comp, ids)
}

/// A value state outside the five states is rejected.
#[test]
fn value_state_outside_the_set_is_rejected() {
    let (_file, conn, comp, n) = value_fixture("value_state_outside_the_set_is_rejected");
    for state in ["missing", "KNOWN", "", "traces"] {
        assert_rejected(
            &conn,
            &format!(
                "INSERT INTO food_nutrient_value VALUES ({comp}, {}, '{state}', NULL)",
                n[0]
            ),
        );
    }
    assert_rejected(
        &conn,
        &format!(
            "INSERT INTO food_nutrient_value VALUES ({comp}, {}, NULL, NULL)",
            n[0]
        ),
    );
}

/// A known value must be positive (D18): 0 and negative amounts are rejected,
/// a known value without amount is rejected, and 1 (0.001 unit) is accepted.
#[test]
fn known_value_requires_a_positive_amount() {
    let (_file, conn, comp, n) = value_fixture("known_value_requires_a_positive_amount");
    assert_rejected(
        &conn,
        &format!(
            "INSERT INTO food_nutrient_value VALUES ({comp}, {}, 'known', 0)",
            n[0]
        ),
    );
    assert_rejected(
        &conn,
        &format!(
            "INSERT INTO food_nutrient_value VALUES ({comp}, {}, 'known', -1)",
            n[0]
        ),
    );
    assert_rejected(
        &conn,
        &format!(
            "INSERT INTO food_nutrient_value VALUES ({comp}, {}, 'known', NULL)",
            n[0]
        ),
    );
    assert_accepted(
        &conn,
        &format!(
            "INSERT INTO food_nutrient_value VALUES ({comp}, {}, 'known', 1)",
            n[0]
        ),
    );
}

/// Amounts are integer fixed-point (D1): a real number or text is rejected by STRICT.
#[test]
fn non_integer_amount_is_rejected() {
    let (_file, conn, comp, n) = value_fixture("non_integer_amount_is_rejected");
    assert_rejected(
        &conn,
        &format!(
            "INSERT INTO food_nutrient_value VALUES ({comp}, {}, 'known', 12.345)",
            n[0]
        ),
    );
    assert_rejected(
        &conn,
        &format!(
            "INSERT INTO food_nutrient_value VALUES ({comp}, {}, 'known', '12 g')",
            n[0]
        ),
    );
    // 12.345 g stored as 12345 (D1).
    assert_accepted(
        &conn,
        &format!(
            "INSERT INTO food_nutrient_value VALUES ({comp}, {}, 'known', 12345)",
            n[0]
        ),
    );
}

/// A below-limit value holds a non-negative limit: 0 and positive accepted,
/// negative and missing rejected.
#[test]
fn below_limit_requires_a_non_negative_limit() {
    let (_file, conn, comp, n) = value_fixture("below_limit_requires_a_non_negative_limit");
    assert_rejected(
        &conn,
        &format!(
            "INSERT INTO food_nutrient_value VALUES ({comp}, {}, 'below_limit', -1)",
            n[0]
        ),
    );
    assert_rejected(
        &conn,
        &format!(
            "INSERT INTO food_nutrient_value VALUES ({comp}, {}, 'below_limit', NULL)",
            n[0]
        ),
    );
    assert_accepted(
        &conn,
        &format!(
            "INSERT INTO food_nutrient_value VALUES ({comp}, {}, 'below_limit', 0)",
            n[0]
        ),
    );
    assert_accepted(
        &conn,
        &format!(
            "INSERT INTO food_nutrient_value VALUES ({comp}, {}, 'below_limit', 500)",
            n[1]
        ),
    );
}

/// Zero, trace, and unknown values carry no amount (D8): any amount,
/// including 0, is rejected; NULL is accepted.
#[test]
fn zero_trace_and_unknown_reject_an_amount() {
    let (_file, conn, comp, n) = value_fixture("zero_trace_and_unknown_reject_an_amount");
    for (i, state) in ["zero", "trace", "unknown"].iter().enumerate() {
        for amount in ["0", "1"] {
            assert_rejected(
                &conn,
                &format!(
                    "INSERT INTO food_nutrient_value VALUES ({comp}, {}, '{state}', {amount})",
                    n[i]
                ),
            );
        }
        assert_accepted(
            &conn,
            &format!(
                "INSERT INTO food_nutrient_value VALUES ({comp}, {}, '{state}', NULL)",
                n[i]
            ),
        );
    }
}

/// A composition has at most one value per nutrient.
#[test]
fn duplicate_value_for_composition_and_nutrient_is_rejected() {
    let (_file, conn, comp, n) =
        value_fixture("duplicate_value_for_composition_and_nutrient_is_rejected");
    assert_accepted(
        &conn,
        &format!(
            "INSERT INTO food_nutrient_value VALUES ({comp}, {}, 'known', 100)",
            n[0]
        ),
    );
    assert_rejected(
        &conn,
        &format!(
            "INSERT INTO food_nutrient_value VALUES ({comp}, {}, 'zero', NULL)",
            n[0]
        ),
    );
}

/// Updating a stored value into an inconsistent state is rejected too.
#[test]
fn update_to_inconsistent_value_is_rejected() {
    let (_file, conn, comp, n) = value_fixture("update_to_inconsistent_value_is_rejected");
    assert_accepted(
        &conn,
        &format!(
            "INSERT INTO food_nutrient_value VALUES ({comp}, {}, 'known', 100)",
            n[0]
        ),
    );
    assert_rejected(
        &conn,
        &format!("UPDATE food_nutrient_value SET value_state = 'zero' WHERE composition_id = {comp} AND nutrient_id = {}", n[0]),
    );
    assert_accepted(
        &conn,
        &format!("UPDATE food_nutrient_value SET value_state = 'zero', amount = NULL WHERE composition_id = {comp} AND nutrient_id = {}", n[0]),
    );
}

// ---------------------------------------------------------------------------
// Recipes and items (D6, D17)
// ---------------------------------------------------------------------------

/// Servings must be at least 1 and fit in a u32 (DATA_MODEL §11).
#[test]
fn servings_outside_1_to_u32_max_are_rejected() {
    let (_file, conn) = migrated_db("servings_outside_1_to_u32_max_are_rejected");
    for s in ["0", "-1", "4294967296", "NULL"] {
        assert_rejected(
            &conn,
            &format!("INSERT INTO recipe (name, servings) VALUES ('R', {s})"),
        );
    }
    for s in ["1", "4294967295"] {
        assert_accepted(
            &conn,
            &format!("INSERT INTO recipe (name, servings) VALUES ('R', {s})"),
        );
    }
}

/// The cooked weight is optional but, when present, at least 1 g.
#[test]
fn cooked_weight_zero_is_rejected_and_null_accepted() {
    let (_file, conn) = migrated_db("cooked_weight_zero_is_rejected_and_null_accepted");
    for w in ["0", "-1", "4294967296"] {
        assert_rejected(
            &conn,
            &format!("INSERT INTO recipe (name, servings, cooked_weight_g) VALUES ('R', 1, {w})"),
        );
    }
    for w in ["NULL", "1", "4294967295"] {
        assert_accepted(
            &conn,
            &format!("INSERT INTO recipe (name, servings, cooked_weight_g) VALUES ('R', 1, {w})"),
        );
    }
}

/// Recipe item quantities are whole grams from 1 to u32::MAX (D6, DATA_MODEL §11).
#[test]
fn item_quantity_outside_1_to_u32_max_is_rejected() {
    let (_file, conn) = migrated_db("item_quantity_outside_1_to_u32_max_is_rejected");
    let food = insert_food(&conn, user_source(&conn), "Synthetic food");
    let recipe = insert_recipe(&conn);
    for q in ["0", "-5", "4294967296", "12.5", "'100g'"] {
        assert_rejected(
            &conn,
            &format!("INSERT INTO recipe_item VALUES ({recipe}, 0, {food}, 'raw', {q})"),
        );
    }
    assert_accepted(
        &conn,
        &format!("INSERT INTO recipe_item VALUES ({recipe}, 0, {food}, 'raw', 1)"),
    );
    assert_accepted(
        &conn,
        &format!("INSERT INTO recipe_item VALUES ({recipe}, 1, {food}, 'raw', 4294967295)"),
    );
}

/// Item positions are 0-based: -1 is rejected, 0 is accepted (D17).
#[test]
fn negative_item_position_is_rejected() {
    let (_file, conn) = migrated_db("negative_item_position_is_rejected");
    let food = insert_food(&conn, user_source(&conn), "Synthetic food");
    let recipe = insert_recipe(&conn);
    assert_rejected(
        &conn,
        &format!("INSERT INTO recipe_item VALUES ({recipe}, -1, {food}, 'raw', 100)"),
    );
    assert_accepted(
        &conn,
        &format!("INSERT INTO recipe_item VALUES ({recipe}, 0, {food}, 'raw', 100)"),
    );
}

/// A recipe item is raw or cooked; 'shared' is a composition state, not a
/// user choice (D9, D28).
#[test]
fn item_state_shared_is_rejected() {
    let (_file, conn) = migrated_db("item_state_shared_is_rejected");
    let food = insert_food(&conn, user_source(&conn), "Synthetic food");
    let recipe = insert_recipe(&conn);
    for state in ["shared", "fried", ""] {
        assert_rejected(
            &conn,
            &format!("INSERT INTO recipe_item VALUES ({recipe}, 0, {food}, '{state}', 100)"),
        );
    }
    assert_accepted(
        &conn,
        &format!("INSERT INTO recipe_item VALUES ({recipe}, 0, {food}, 'raw', 100)"),
    );
    assert_accepted(
        &conn,
        &format!("INSERT INTO recipe_item VALUES ({recipe}, 1, {food}, 'cooked', 100)"),
    );
}

/// Two items cannot share a position, but the same food may appear twice at
/// two positions (D17).
#[test]
fn duplicate_position_is_rejected_but_same_food_twice_is_accepted() {
    let (_file, conn) =
        migrated_db("duplicate_position_is_rejected_but_same_food_twice_is_accepted");
    let user = user_source(&conn);
    let food = insert_food(&conn, user, "Synthetic food");
    let other = insert_food(&conn, user, "Other synthetic food");
    let recipe = insert_recipe(&conn);
    assert_accepted(
        &conn,
        &format!("INSERT INTO recipe_item VALUES ({recipe}, 0, {food}, 'raw', 100)"),
    );
    assert_rejected(
        &conn,
        &format!("INSERT INTO recipe_item VALUES ({recipe}, 0, {other}, 'raw', 50)"),
    );
    assert_accepted(
        &conn,
        &format!("INSERT INTO recipe_item VALUES ({recipe}, 1, {food}, 'raw', 100)"),
    );
    // Same position in another recipe is fine.
    let recipe2 = insert_recipe(&conn);
    assert_accepted(
        &conn,
        &format!("INSERT INTO recipe_item VALUES ({recipe2}, 0, {food}, 'raw', 100)"),
    );
}

// ---------------------------------------------------------------------------
// Delete behaviour
// ---------------------------------------------------------------------------

/// Deleting a food deletes its compositions and their nutrient values.
#[test]
fn deleting_food_cascades_to_compositions_and_values() {
    let (_file, conn) = migrated_db("deleting_food_cascades_to_compositions_and_values");
    let user = user_source(&conn);
    let food = insert_food(&conn, user, "Synthetic food");
    let keep = insert_food(&conn, user, "Kept synthetic food");
    let raw = insert_composition(&conn, food, user, "raw", None);
    let cooked = insert_composition(&conn, food, user, "cooked", None);
    let kept = insert_composition(&conn, keep, user, "shared", None);
    let protein = nutrient_id(&conn, "protein");
    for comp in [raw, cooked, kept] {
        assert_accepted(
            &conn,
            &format!("INSERT INTO food_nutrient_value VALUES ({comp}, {protein}, 'known', 1000)"),
        );
    }

    assert_accepted(&conn, &format!("DELETE FROM food WHERE id = {food}"));

    assert_eq!(
        query_i64(
            &conn,
            &format!("SELECT count(*) FROM food_composition WHERE food_id = {food}")
        ),
        0
    );
    assert_eq!(
        query_i64(&conn, &format!("SELECT count(*) FROM food_nutrient_value WHERE composition_id IN ({raw}, {cooked})")),
        0
    );
    // The other food is untouched.
    assert_eq!(query_i64(&conn, "SELECT count(*) FROM food_composition"), 1);
    assert_eq!(
        query_i64(
            &conn,
            &format!("SELECT count(*) FROM food_nutrient_value WHERE composition_id = {kept}")
        ),
        1
    );
}

/// Deleting a recipe deletes its items.
#[test]
fn deleting_recipe_cascades_to_items() {
    let (_file, conn) = migrated_db("deleting_recipe_cascades_to_items");
    let food = insert_food(&conn, user_source(&conn), "Synthetic food");
    let recipe = insert_recipe(&conn);
    let kept = insert_recipe(&conn);
    assert_accepted(
        &conn,
        &format!("INSERT INTO recipe_item VALUES ({recipe}, 0, {food}, 'raw', 100)"),
    );
    assert_accepted(
        &conn,
        &format!("INSERT INTO recipe_item VALUES ({recipe}, 1, {food}, 'cooked', 50)"),
    );
    assert_accepted(
        &conn,
        &format!("INSERT INTO recipe_item VALUES ({kept}, 0, {food}, 'raw', 10)"),
    );

    assert_accepted(&conn, &format!("DELETE FROM recipe WHERE id = {recipe}"));

    assert_eq!(
        query_i64(
            &conn,
            &format!("SELECT count(*) FROM recipe_item WHERE recipe_id = {recipe}")
        ),
        0
    );
    assert_eq!(query_i64(&conn, "SELECT count(*) FROM recipe_item"), 1);
}

/// A food used by a recipe item cannot be deleted; once unused it can.
#[test]
fn deleting_food_used_by_recipe_item_is_rejected() {
    let (_file, conn) = migrated_db("deleting_food_used_by_recipe_item_is_rejected");
    let food = insert_food(&conn, user_source(&conn), "Synthetic food");
    let recipe = insert_recipe(&conn);
    assert_accepted(
        &conn,
        &format!("INSERT INTO recipe_item VALUES ({recipe}, 0, {food}, 'raw', 100)"),
    );
    assert_rejected(&conn, &format!("DELETE FROM food WHERE id = {food}"));
    assert_eq!(
        query_i64(
            &conn,
            &format!("SELECT count(*) FROM food WHERE id = {food}")
        ),
        1
    );
    assert_accepted(&conn, &format!("DELETE FROM recipe WHERE id = {recipe}"));
    assert_accepted(&conn, &format!("DELETE FROM food WHERE id = {food}"));
}

/// A nutrient used by a stored value cannot be deleted; an unused one can.
#[test]
fn deleting_nutrient_used_by_value_is_rejected() {
    let (_file, conn) = migrated_db("deleting_nutrient_used_by_value_is_rejected");
    let user = user_source(&conn);
    let food = insert_food(&conn, user, "Synthetic food");
    let comp = insert_composition(&conn, food, user, "raw", None);
    let protein = nutrient_id(&conn, "protein");
    assert_accepted(
        &conn,
        &format!("INSERT INTO food_nutrient_value VALUES ({comp}, {protein}, 'trace', NULL)"),
    );
    assert_rejected(&conn, &format!("DELETE FROM nutrient WHERE id = {protein}"));
    let iron = nutrient_id(&conn, "iron");
    assert_accepted(&conn, &format!("DELETE FROM nutrient WHERE id = {iron}"));
}

/// A data source used by a food cannot be deleted.
#[test]
fn deleting_data_source_used_by_food_is_rejected() {
    let (_file, conn) = migrated_db("deleting_data_source_used_by_food_is_rejected");
    let src = insert_source(&conn, "Synthetic source");
    let unused = insert_source(&conn, "Unused synthetic source");
    insert_food(&conn, src, "Synthetic food");
    assert_rejected(&conn, &format!("DELETE FROM data_source WHERE id = {src}"));
    assert_accepted(
        &conn,
        &format!("DELETE FROM data_source WHERE id = {unused}"),
    );
}
