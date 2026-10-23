//! Food and data source persistence tests: insert, load, list, search, and
//! delete through the `Database` API.
//!
//! Expected behaviour comes from docs/DATA_MODEL.md §3–5, docs/DECISIONS.md
//! (D2, D8, D9, D18, D19, D25, D28), docs/TECHNOLOGY.md §3 (search), and the
//! store API brief, not from the implementation. Every composition value is
//! synthetic, round, and not real composition data (D13). Amounts are in
//! thousandths of the canonical unit (D1): `known(12_000)` is 12 g.

mod common;

use common::{query_i64, raw_connection, TempDbFile};
use nutrition_core::{
    calculate_recipe, Composition, Compositions, Food, FoodId, Grams, NutrientValue, Recipe,
    Servings, State,
};
use nutrition_store::{
    Database, Error, FoodSummary, NewComposition, NewCompositions, NewDataSource, NewFood,
    SourceId, USER_SOURCE,
};
use rusqlite::Connection;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// A new database file opened with `Database::open`, so that raw connections
/// can inspect the same file.
///
/// # Arguments
///
/// * `name` - the test name, unique within the test binary.
fn open(name: &str) -> (TempDbFile, Database) {
    let file = TempDbFile::new(&format!("foods-{name}"));
    let db = Database::open(file.path()).expect("Database::open on a new file");
    (file, db)
}

/// The id of the seeded "user" data source, resolved through the API (D25).
///
/// # Arguments
///
/// * `db` - the database to query.
fn user(db: &Database) -> SourceId {
    db.data_source_id(USER_SOURCE)
        .expect("the user source is seeded")
}

/// Inserts a data source with only a name and returns its id.
///
/// # Arguments
///
/// * `db` - the database to write to.
/// * `name` - the source name, non-blank and unique.
fn source(db: &mut Database, name: &str) -> SourceId {
    db.insert_data_source(&NewDataSource {
        name: name.to_string(),
        ..Default::default()
    })
    .expect("insert data source")
}

/// A composition to insert, with an optional source record id.
///
/// # Arguments
///
/// * `composition` - the nutrient values per reference quantity.
/// * `record` - the source record id (D2), `None` for user foods (D25).
fn comp(composition: Composition, record: Option<&str>) -> NewComposition {
    NewComposition {
        composition,
        source_record_id: record.map(str::to_string),
    }
}

/// A food to insert, without category or description.
///
/// # Arguments
///
/// * `source` - the food's data source.
/// * `name_fr` - the French name, if any.
/// * `name_en` - the English name, if any.
/// * `compositions` - the compositions to insert with the food.
fn new_food(
    source: SourceId,
    name_fr: Option<&str>,
    name_en: Option<&str>,
    compositions: NewCompositions,
) -> NewFood {
    NewFood {
        source,
        name_fr: name_fr.map(str::to_string),
        name_en: name_en.map(str::to_string),
        category: None,
        description: None,
        compositions,
    }
}

/// A synthetic shared composition per 100 g with 10 g of protein.
fn simple_composition() -> Composition {
    Composition::per_100g().with("protein", NutrientValue::known(10_000).unwrap())
}

/// Inserts a user food with a French name and a simple shared composition.
///
/// # Arguments
///
/// * `db` - the database to write to.
/// * `name_fr` - the French name.
fn insert_named(db: &mut Database, name_fr: &str) -> FoodId {
    let src = user(db);
    db.insert_food(&new_food(
        src,
        Some(name_fr),
        None,
        NewCompositions::Shared(comp(simple_composition(), None)),
    ))
    .expect("insert food")
}

/// The number of rows in `table`, read through a fresh raw connection.
///
/// # Arguments
///
/// * `file` - the database file.
/// * `table` - a table name written by the test.
fn count(file: &TempDbFile, table: &str) -> i64 {
    let conn = raw_connection(file.path());
    query_i64(&conn, &format!("SELECT count(*) FROM {table}"))
}

/// Asserts that a failed insert left no row in `food`, `food_composition`, or
/// `food_nutrient_value` beyond the counts given.
///
/// # Arguments
///
/// * `file` - the database file.
/// * `foods` - the expected number of foods.
/// * `compositions` - the expected number of compositions.
/// * `values` - the expected number of nutrient values.
fn assert_counts(file: &TempDbFile, foods: i64, compositions: i64, values: i64) {
    assert_eq!(count(file, "food"), foods, "food rows");
    assert_eq!(
        count(file, "food_composition"),
        compositions,
        "food_composition rows"
    );
    assert_eq!(
        count(file, "food_nutrient_value"),
        values,
        "food_nutrient_value rows"
    );
}

/// The ids of a list of summaries, in the order returned.
///
/// # Arguments
///
/// * `summaries` - the summaries returned by `foods` or `search_foods`.
fn ids(summaries: &[FoodSummary]) -> Vec<FoodId> {
    summaries.iter().map(|s| s.id).collect()
}

/// The ids found by `search_foods`, panicking on an error.
///
/// # Arguments
///
/// * `db` - the database to search.
/// * `text` - the user's search text.
/// * `limit` - the maximum number of results.
fn search(db: &Database, text: &str, limit: usize) -> Vec<FoodId> {
    match db.search_foods(text, limit) {
        Ok(found) => ids(&found),
        Err(e) => panic!("search_foods({text:?}, {limit}) failed: {e:?}"),
    }
}

/// A composition per 100 g with every value state (D8, D18): known, zero,
/// trace, below a limit (500 and 0), and an explicit unknown. Iron has no
/// record, so it is unknown.
fn all_states_composition() -> Composition {
    Composition::per_100g()
        .with("energy_kj", NutrientValue::known(1_000_000).unwrap()) // 1000 kJ
        .with("protein", NutrientValue::known(10_000).unwrap()) // 10 g
        .with("fat", NutrientValue::Zero)
        .with("sugars", NutrientValue::Trace)
        .with("salt", NutrientValue::below_limit(500).unwrap()) // < 0.5 g
        .with("fibre", NutrientValue::below_limit(0).unwrap()) // < 0 g
        .with("vitamin_d", NutrientValue::Unknown)
}

// ---------------------------------------------------------------------------
// Data sources (DATA_MODEL §5, D25)
// ---------------------------------------------------------------------------

/// `USER_SOURCE` is "user" and resolves to the seeded user source (D25).
#[test]
fn user_source_resolves_to_seeded_row() {
    let (file, db) = open("user_source_resolves_to_seeded_row");
    assert_eq!(USER_SOURCE, "user");
    let conn = raw_connection(file.path());
    let id = query_i64(&conn, "SELECT id FROM data_source WHERE name = 'user'");
    assert_eq!(user(&db), SourceId(id));
}

/// An inserted data source is found by name with the same id, and its
/// optional fields are stored.
#[test]
fn inserted_data_source_round_trips_by_name() {
    let (file, mut db) = open("inserted_data_source_round_trips_by_name");
    let id = db
        .insert_data_source(&NewDataSource {
            name: "Synthetic source".to_string(),
            version: Some("2020".to_string()),
            source_url: Some("https://example.invalid/source".to_string()),
            licence: Some("Synthetic licence".to_string()),
            retrieved_at: Some("2026-01-02T03:04:05Z".to_string()),
        })
        .unwrap();
    assert_eq!(db.data_source_id("Synthetic source").unwrap(), id);
    assert_ne!(id, user(&db));

    let conn = raw_connection(file.path());
    let row: (String, String, String, String) = conn
        .query_row(
            "SELECT version, source_url, licence, retrieved_at FROM data_source WHERE id = ?1",
            [id.0],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(
        row,
        (
            "2020".to_string(),
            "https://example.invalid/source".to_string(),
            "Synthetic licence".to_string(),
            "2026-01-02T03:04:05Z".to_string()
        )
    );
}

/// A data source with a name already used, including "user", is rejected
/// with `DuplicateDataSource`.
#[test]
fn duplicate_data_source_name_is_rejected() {
    let (file, mut db) = open("duplicate_data_source_name_is_rejected");
    source(&mut db, "Synthetic source");
    for name in ["Synthetic source", "user"] {
        match db.insert_data_source(&NewDataSource {
            name: name.to_string(),
            ..Default::default()
        }) {
            Err(Error::DuplicateDataSource { name: n }) => assert_eq!(n, name),
            other => panic!("expected DuplicateDataSource for {name}, got {other:?}"),
        }
    }
    assert_eq!(count(&file, "data_source"), 2);
}

/// A blank data source name ("" or spaces) is rejected with `InvalidInput`.
#[test]
fn blank_data_source_name_is_rejected() {
    let (file, mut db) = open("blank_data_source_name_is_rejected");
    for name in ["", "  "] {
        match db.insert_data_source(&NewDataSource {
            name: name.to_string(),
            ..Default::default()
        }) {
            Err(Error::InvalidInput { .. }) => {}
            other => panic!("expected InvalidInput for {name:?}, got {other:?}"),
        }
    }
    assert_eq!(count(&file, "data_source"), 1);
}

/// Looking up a name that is not a data source gives `UnknownDataSource`.
#[test]
fn unknown_data_source_name_is_an_error() {
    let db = Database::open_in_memory().unwrap();
    match db.data_source_id("No such source") {
        Err(Error::UnknownDataSource { name }) => assert_eq!(name, "No such source"),
        other => panic!("expected UnknownDataSource, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Round trip (D8, D9, D18, D28)
// ---------------------------------------------------------------------------

/// A shared composition with every value state is loaded back equal: known,
/// zero, trace, below-limit (including limit 0), explicit unknown, and a
/// nutrient without a record that stays unknown.
#[test]
fn shared_food_with_every_value_state_round_trips() {
    let (_file, mut db) = open("shared_food_with_every_value_state_round_trips");
    let src = user(&db);
    let id = db
        .insert_food(&new_food(
            src,
            Some("Synthetic food A"),
            None,
            NewCompositions::Shared(comp(all_states_composition(), None)),
        ))
        .unwrap();

    let food = db.food(id).unwrap();
    assert_eq!(
        food,
        Food::new(
            id,
            "Synthetic food A",
            Compositions::Shared(all_states_composition())
        )
    );
    // Explicit checks of the states that are easiest to lose.
    let raw = food.composition(State::Raw);
    assert_eq!(raw.value("fat"), NutrientValue::Zero);
    assert_eq!(raw.value("fibre"), NutrientValue::below_limit(0).unwrap());
    assert_eq!(raw.value("vitamin_d"), NutrientValue::Unknown);
    assert_eq!(raw.value("iron"), NutrientValue::Unknown);
    assert!(raw
        .values()
        .any(|(c, v)| c == "vitamin_d" && v == NutrientValue::Unknown));
    assert!(raw.values().all(|(c, _)| c != "iron"));
    assert!(food.is_shared());
}

/// Distinct raw and cooked compositions with source record ids are loaded
/// back equal, each in its own state (D9, D28).
#[test]
fn distinct_food_round_trips() {
    let (_file, mut db) = open("distinct_food_round_trips");
    let src = source(&mut db, "Synthetic source");
    let raw = all_states_composition();
    let cooked = Composition::per_100g()
        .with("energy_kj", NutrientValue::known(400_000).unwrap()) // 400 kJ
        .with("protein", NutrientValue::known(4_000).unwrap()) // 4 g
        .with("fat", NutrientValue::Trace)
        .with("salt", NutrientValue::Unknown);
    let id = db
        .insert_food(&new_food(
            src,
            Some("Synthetic pâtes"),
            Some("Synthetic pasta"),
            NewCompositions::Distinct {
                raw: comp(raw.clone(), Some("R-RAW")),
                cooked: comp(cooked.clone(), Some("R-COOKED")),
            },
        ))
        .unwrap();

    let food = db.food(id).unwrap();
    assert_eq!(
        food,
        Food::new(
            id,
            "Synthetic pâtes",
            Compositions::Distinct {
                raw: raw.clone(),
                cooked: cooked.clone()
            }
        )
    );
    assert!(!food.is_shared());
    assert_eq!(food.composition(State::Cooked), &cooked);
}

/// The stored food row holds both names, the category, the description, and
/// the source; the source record ids are kept per state (D2).
#[test]
fn inserted_food_row_keeps_all_fields() {
    let (file, mut db) = open("inserted_food_row_keeps_all_fields");
    let src = source(&mut db, "Synthetic source");
    let id = db
        .insert_food(&NewFood {
            source: src,
            name_fr: Some("Synthetic pomme".to_string()),
            name_en: Some("Synthetic apple".to_string()),
            category: Some("Synthetic fruits".to_string()),
            description: Some("Synthetic description".to_string()),
            compositions: NewCompositions::Distinct {
                raw: comp(simple_composition(), Some("R-1")),
                cooked: comp(simple_composition(), Some("R-2")),
            },
        })
        .unwrap();

    let conn = raw_connection(file.path());
    let row: (i64, String, String, String, String) = conn
        .query_row(
            "SELECT source_id, name_fr, name_en, category, description FROM food WHERE id = ?1",
            [id.0],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .unwrap();
    assert_eq!(
        row,
        (
            src.0,
            "Synthetic pomme".to_string(),
            "Synthetic apple".to_string(),
            "Synthetic fruits".to_string(),
            "Synthetic description".to_string()
        )
    );
    let records: Vec<(String, String)> = conn
        .prepare(
            "SELECT state, source_record_id FROM food_composition WHERE food_id = ?1 ORDER BY state",
        )
        .unwrap()
        .query_map([id.0], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(
        records,
        vec![
            ("cooked".to_string(), "R-2".to_string()),
            ("raw".to_string(), "R-1".to_string())
        ]
    );
}

/// The display name of the loaded food is the French name when present.
#[test]
fn display_name_prefers_name_fr() {
    let (_file, mut db) = open("display_name_prefers_name_fr");
    let src = user(&db);
    let id = db
        .insert_food(&new_food(
            src,
            Some("Synthetic pomme"),
            Some("Synthetic apple"),
            NewCompositions::Shared(comp(simple_composition(), None)),
        ))
        .unwrap();
    assert_eq!(db.food(id).unwrap().name(), "Synthetic pomme");
}

/// The display name of the loaded food is the English name when there is no
/// French name.
#[test]
fn display_name_falls_back_to_name_en() {
    let (_file, mut db) = open("display_name_falls_back_to_name_en");
    let src = user(&db);
    let id = db
        .insert_food(&new_food(
            src,
            None,
            Some("Synthetic apple"),
            NewCompositions::Shared(comp(simple_composition(), None)),
        ))
        .unwrap();
    assert_eq!(db.food(id).unwrap().name(), "Synthetic apple");
}

/// A user food (D25) is inserted under `USER_SOURCE` with no source record
/// id, and two such foods can coexist.
#[test]
fn user_food_without_source_record_ids_is_accepted() {
    let (_file, mut db) = open("user_food_without_source_record_ids_is_accepted");
    let src = user(&db);
    let a = db
        .insert_food(&new_food(
            src,
            Some("Synthetic user food"),
            None,
            NewCompositions::Distinct {
                raw: comp(simple_composition(), None),
                cooked: comp(simple_composition(), None),
            },
        ))
        .unwrap();
    let b = insert_named(&mut db, "Other synthetic user food");
    assert_ne!(a, b);
    assert_eq!(db.food(a).unwrap().name(), "Synthetic user food");
    assert_eq!(db.food(b).unwrap().name(), "Other synthetic user food");
}

/// A reference quantity other than 100 g from an imported source is stored and
/// loaded back: the store does not enforce D19, the calculation does.
#[test]
fn non_100_g_reference_quantity_round_trips() {
    let (_file, mut db) = open("non_100_g_reference_quantity_round_trips");
    let src = source(&mut db, "Synthetic source");
    let composition = Composition::new(Grams::new(50).unwrap())
        .with("protein", NutrientValue::known(5_000).unwrap());
    let id = db
        .insert_food(&new_food(
            src,
            Some("Synthetic half portion"),
            None,
            NewCompositions::Shared(comp(composition.clone(), None)),
        ))
        .unwrap();
    let food = db.food(id).unwrap();
    assert_eq!(food.composition(State::Raw).reference_quantity().get(), 50);
    assert_eq!(
        food,
        Food::new(
            id,
            "Synthetic half portion",
            Compositions::Shared(composition)
        )
    );
}

/// A user food is entered per 100 g (D25): another reference quantity is
/// rejected and nothing is written.
#[test]
fn user_food_not_per_100_g_is_rejected() {
    let (file, mut db) = open("user_food_not_per_100_g_is_rejected");
    let src = user(&db);
    let composition = Composition::new(Grams::new(50).unwrap())
        .with("protein", NutrientValue::known(5_000).unwrap());
    let err = db
        .insert_food(&new_food(
            src,
            Some("Synthetic half portion"),
            None,
            NewCompositions::Shared(comp(composition, None)),
        ))
        .unwrap_err();
    assert!(matches!(err, Error::InvalidInput { .. }), "{err:?}");
    assert_counts(&file, 0, 0, 0);
}

/// A user food has no source record id (D25, DATA_MODEL.md §3): one is
/// rejected, in a shared or a distinct composition, and nothing is written.
#[test]
fn user_food_with_source_record_id_is_rejected() {
    let (file, mut db) = open("user_food_with_source_record_id_is_rejected");
    let src = user(&db);
    for compositions in [
        NewCompositions::Shared(comp(simple_composition(), Some("1234"))),
        NewCompositions::Distinct {
            raw: comp(simple_composition(), None),
            cooked: comp(simple_composition(), Some("1234")),
        },
    ] {
        let err = db
            .insert_food(&new_food(
                src,
                Some("Synthetic user food"),
                None,
                compositions,
            ))
            .unwrap_err();
        assert!(matches!(err, Error::InvalidInput { .. }), "{err:?}");
    }
    assert_counts(&file, 0, 0, 0);
}

/// Words joined by a hyphen or an apostrophe are matched as separate prefixes
/// (D32): "pomm-de" finds "Pommes de terre".
#[test]
fn search_splits_words_on_punctuation() {
    let (_file, mut db) = open("search_splits_words_on_punctuation");
    let id = insert_named(&mut db, "Pommes de terre");
    assert_eq!(search(&db, "pomm-de", 10), vec![id]);
    assert_eq!(search(&db, "l'pomm", 10), Vec::<FoodId>::new());
}

/// Foods with accented capitals are listed with their unaccented neighbours,
/// not after every ASCII name (D32).
#[test]
fn foods_ignore_accents_when_ordering() {
    let (_file, mut db) = open("foods_ignore_accents_when_ordering");
    let zeste = insert_named(&mut db, "Zeste");
    let oeuf = insert_named(&mut db, "Œuf");
    let epinard = insert_named(&mut db, "Épinard");
    let endive = insert_named(&mut db, "endive");
    assert_eq!(
        ids(&db.foods().unwrap()),
        vec![endive, epinard, oeuf, zeste]
    );
}

// ---------------------------------------------------------------------------
// insert_food rejections: nothing is written
// ---------------------------------------------------------------------------

/// A food with neither name is rejected with `InvalidInput`, and nothing is written.
#[test]
fn food_without_any_name_is_rejected() {
    let (file, mut db) = open("food_without_any_name_is_rejected");
    let src = user(&db);
    let result = db.insert_food(&new_food(
        src,
        None,
        None,
        NewCompositions::Shared(comp(simple_composition(), None)),
    ));
    assert!(
        matches!(result, Err(Error::InvalidInput { .. })),
        "got {result:?}"
    );
    assert_counts(&file, 0, 0, 0);
}

/// A blank French or English name ("" or spaces) is rejected with
/// `InvalidInput`, even when the other name is valid; nothing is written.
#[test]
fn blank_food_name_is_rejected() {
    let (file, mut db) = open("blank_food_name_is_rejected");
    let src = user(&db);
    for blank in ["", "  "] {
        for (fr, en) in [
            (Some(blank), Some("Synthetic apple")),
            (Some("Synthetic pomme"), Some(blank)),
            (Some(blank), None),
            (None, Some(blank)),
        ] {
            let result = db.insert_food(&new_food(
                src,
                fr,
                en,
                NewCompositions::Shared(comp(simple_composition(), None)),
            ));
            assert!(
                matches!(result, Err(Error::InvalidInput { .. })),
                "fr {fr:?}, en {en:?}: got {result:?}"
            );
        }
    }
    assert_counts(&file, 0, 0, 0);
}

/// A blank category or description ("" or spaces) is rejected with
/// `InvalidInput` (absent must be `None`); nothing is written.
#[test]
fn blank_category_or_description_is_rejected() {
    let (file, mut db) = open("blank_category_or_description_is_rejected");
    let src = user(&db);
    for blank in ["", "  "] {
        for (category, description) in [(Some(blank), None), (None, Some(blank))] {
            let mut food = new_food(
                src,
                Some("Synthetic food"),
                None,
                NewCompositions::Shared(comp(simple_composition(), None)),
            );
            food.category = category.map(str::to_string);
            food.description = description.map(str::to_string);
            let result = db.insert_food(&food);
            assert!(
                matches!(result, Err(Error::InvalidInput { .. })),
                "category {category:?}, description {description:?}: got {result:?}"
            );
        }
    }
    assert_counts(&file, 0, 0, 0);
}

/// A blank source record id ("" or spaces) is rejected with `InvalidInput`
/// in a shared composition and in either distinct one; nothing is written.
#[test]
fn blank_source_record_id_is_rejected() {
    let (file, mut db) = open("blank_source_record_id_is_rejected");
    let src = source(&mut db, "Synthetic source");
    for blank in ["", "  "] {
        let cases = [
            NewCompositions::Shared(comp(simple_composition(), Some(blank))),
            NewCompositions::Distinct {
                raw: comp(simple_composition(), Some(blank)),
                cooked: comp(simple_composition(), Some("R-COOKED")),
            },
            NewCompositions::Distinct {
                raw: comp(simple_composition(), Some("R-RAW")),
                cooked: comp(simple_composition(), Some(blank)),
            },
        ];
        for compositions in cases {
            let result = db.insert_food(&new_food(src, Some("Synthetic food"), None, compositions));
            assert!(
                matches!(result, Err(Error::InvalidInput { .. })),
                "record {blank:?}: got {result:?}"
            );
        }
    }
    assert_counts(&file, 0, 0, 0);
}

/// A food whose source id does not exist is rejected with
/// `UnknownDataSourceId`; nothing is written.
#[test]
fn food_with_unknown_source_id_is_rejected() {
    let (file, mut db) = open("food_with_unknown_source_id_is_rejected");
    match db.insert_food(&new_food(
        SourceId(9999),
        Some("Synthetic food"),
        None,
        NewCompositions::Shared(comp(simple_composition(), None)),
    )) {
        Err(Error::UnknownDataSourceId { source_id }) => assert_eq!(source_id, 9999),
        other => panic!("expected UnknownDataSourceId, got {other:?}"),
    }
    assert_counts(&file, 0, 0, 0);
}

/// A nutrient code missing from the nutrient table is rejected with
/// `UnknownNutrient { code }`, also when it is in the cooked composition after
/// valid values; the food row is rolled back too.
#[test]
fn unknown_nutrient_code_is_rejected_and_rolled_back() {
    let (file, mut db) = open("unknown_nutrient_code_is_rejected_and_rolled_back");
    let src = user(&db);
    let bad = simple_composition().with("synthetic_unobtainium", NutrientValue::Trace);
    let cases = [
        NewCompositions::Shared(comp(bad.clone(), None)),
        NewCompositions::Distinct {
            raw: comp(simple_composition(), None),
            cooked: comp(bad.clone(), None),
        },
    ];
    for compositions in cases {
        match db.insert_food(&new_food(src, Some("Synthetic food"), None, compositions)) {
            Err(Error::UnknownNutrient { code }) => assert_eq!(code, "synthetic_unobtainium"),
            other => panic!("expected UnknownNutrient, got {other:?}"),
        }
    }
    assert_counts(&file, 0, 0, 0);
    assert!(search(&db, "synthetic", 10).is_empty());
}

/// A source record id already used in the same source is rejected with
/// `DuplicateSourceRecord` (D2); the second food is not written.
#[test]
fn duplicate_source_record_in_same_source_is_rejected() {
    let (file, mut db) = open("duplicate_source_record_in_same_source_is_rejected");
    let src = source(&mut db, "Synthetic source");
    db.insert_food(&new_food(
        src,
        Some("Synthetic food 1"),
        None,
        NewCompositions::Shared(comp(simple_composition(), Some("R1"))),
    ))
    .unwrap();
    // 1 food, 1 composition, 1 value (protein).
    assert_counts(&file, 1, 1, 1);

    match db.insert_food(&new_food(
        src,
        Some("Synthetic food 2"),
        None,
        NewCompositions::Distinct {
            raw: comp(simple_composition(), Some("R2")),
            cooked: comp(simple_composition(), Some("R1")),
        },
    )) {
        Err(Error::DuplicateSourceRecord {
            source_id,
            source_record_id,
        }) => {
            assert_eq!(source_id, src.0);
            assert_eq!(source_record_id, "R1");
        }
        other => panic!("expected DuplicateSourceRecord, got {other:?}"),
    }
    assert_counts(&file, 1, 1, 1);
}

/// Raw and cooked compositions of one food with the same record id are
/// rejected with `DuplicateSourceRecord`; nothing is written.
#[test]
fn raw_and_cooked_with_same_record_id_are_rejected() {
    let (file, mut db) = open("raw_and_cooked_with_same_record_id_are_rejected");
    let src = source(&mut db, "Synthetic source");
    match db.insert_food(&new_food(
        src,
        Some("Synthetic food"),
        None,
        NewCompositions::Distinct {
            raw: comp(simple_composition(), Some("R1")),
            cooked: comp(simple_composition(), Some("R1")),
        },
    )) {
        Err(Error::DuplicateSourceRecord {
            source_id,
            source_record_id,
        }) => {
            assert_eq!(source_id, src.0);
            assert_eq!(source_record_id, "R1");
        }
        other => panic!("expected DuplicateSourceRecord, got {other:?}"),
    }
    assert_counts(&file, 0, 0, 0);
}

/// The same source record id under another source is a different natural
/// key and is accepted (D2).
#[test]
fn same_record_id_in_another_source_is_accepted() {
    let (_file, mut db) = open("same_record_id_in_another_source_is_accepted");
    let a = source(&mut db, "Synthetic source A");
    let b = source(&mut db, "Synthetic source B");
    for (src, name) in [(a, "Synthetic food A"), (b, "Synthetic food B")] {
        db.insert_food(&new_food(
            src,
            Some(name),
            None,
            NewCompositions::Shared(comp(simple_composition(), Some("R1"))),
        ))
        .unwrap_or_else(|e| panic!("insert {name}: {e:?}"));
    }
    assert_eq!(db.foods().unwrap().len(), 2);
}

// ---------------------------------------------------------------------------
// food and catalog
// ---------------------------------------------------------------------------

/// Loading a missing food gives `FoodNotFound` with its id.
#[test]
fn food_with_missing_id_is_not_found() {
    let db = Database::open_in_memory().unwrap();
    match db.food(FoodId(9999)) {
        Err(Error::FoodNotFound { food_id }) => assert_eq!(food_id, FoodId(9999)),
        other => panic!("expected FoodNotFound, got {other:?}"),
    }
}

/// A catalog built from repeated ids holds each food once, equal to `food(id)`.
#[test]
fn catalog_with_repeated_ids_holds_each_food_once() {
    let (_file, mut db) = open("catalog_with_repeated_ids_holds_each_food_once");
    let a = insert_named(&mut db, "Synthetic food A");
    let b = insert_named(&mut db, "Synthetic food B");
    insert_named(&mut db, "Synthetic food not requested");
    let catalog = db.catalog(&[a, b, a]).unwrap();
    assert_eq!(catalog.len(), 2);
    assert_eq!(catalog.get(a), Some(&db.food(a).unwrap()));
    assert_eq!(catalog.get(b), Some(&db.food(b).unwrap()));
}

/// A catalog of no ids is empty. Assumption: the brief does not state this
/// case; an empty request is not an error.
#[test]
fn catalog_of_no_ids_is_empty() {
    let db = Database::open_in_memory().unwrap();
    assert!(db.catalog(&[]).unwrap().is_empty());
}

/// A catalog request with a missing id gives `FoodNotFound` for that id.
#[test]
fn catalog_with_missing_id_is_not_found() {
    let (_file, mut db) = open("catalog_with_missing_id_is_not_found");
    let a = insert_named(&mut db, "Synthetic food A");
    match db.catalog(&[a, FoodId(9999)]) {
        Err(Error::FoodNotFound { food_id }) => assert_eq!(food_id, FoodId(9999)),
        other => panic!("expected FoodNotFound, got {other:?}"),
    }
}

/// A catalog loaded from the store feeds `calculate_recipe` with the stored
/// nutrients, and the totals are the hand-checked ones.
#[test]
fn catalog_works_with_calculate_recipe() {
    let (_file, mut db) = open("catalog_works_with_calculate_recipe");
    let src = user(&db);
    let pasta = db
        .insert_food(&new_food(
            src,
            Some("Synthetic pâtes"),
            None,
            NewCompositions::Distinct {
                raw: comp(
                    Composition::per_100g().with("protein", NutrientValue::known(10_000).unwrap()),
                    None,
                ),
                cooked: comp(
                    Composition::per_100g().with("protein", NutrientValue::known(4_000).unwrap()),
                    None,
                ),
            },
        ))
        .unwrap();
    let oil = db
        .insert_food(&new_food(
            src,
            Some("Synthetic huile"),
            None,
            NewCompositions::Shared(comp(
                Composition::per_100g().with("protein", NutrientValue::Zero),
                None,
            )),
        ))
        .unwrap();

    let mut recipe = Recipe::new("Synthetic recipe", Servings::new(2).unwrap());
    recipe.add_ingredient(pasta, State::Raw, Grams::new(200).unwrap());
    recipe.add_ingredient(oil, State::Cooked, Grams::new(10).unwrap());
    recipe.add_ingredient(pasta, State::Cooked, Grams::new(50).unwrap());
    let catalog = db.catalog(&[pasta, oil, pasta]).unwrap();
    let nutrients = db.nutrients().unwrap();
    let result = calculate_recipe(&recipe, &catalog, &nutrients).unwrap();

    let protein = result.nutrient("protein").expect("protein total");
    // 200 g × 10 g/100 g + 10 g × 0 + 50 g × 4 g/100 g = 20 g + 0 + 2 g = 22 g.
    assert_eq!(protein.total().milli(), 22_000);
    // 22 g / 2 servings = 11 g.
    assert_eq!(protein.per_serving().milli(), 11_000);
    assert!(protein.unknown_items().is_empty());
    // Fat is recorded for no food: unknown for all three items (D8).
    assert_eq!(result.nutrient("fat").unwrap().unknown_items(), &[0, 1, 2]);
}

// ---------------------------------------------------------------------------
// Invalid stored data (D28)
// ---------------------------------------------------------------------------

/// Inserts a food by raw SQL with the given composition states and returns its id.
///
/// # Arguments
///
/// * `conn` - a raw connection to the database file.
/// * `name` - the French name of the food.
/// * `states` - the composition states to insert, possibly none.
fn raw_food_with_states(conn: &Connection, name: &str, states: &[&str]) -> FoodId {
    let user = query_i64(conn, "SELECT id FROM data_source WHERE name = 'user'");
    conn.execute(
        "INSERT INTO food (source_id, name_fr) VALUES (?1, ?2)",
        rusqlite::params![user, name],
    )
    .unwrap();
    let food = conn.last_insert_rowid();
    for state in states {
        conn.execute(
            "INSERT INTO food_composition (food_id, source_id, state) VALUES (?1, ?2, ?3)",
            rusqlite::params![food, user, state],
        )
        .unwrap();
    }
    FoodId(food)
}

/// A food with only a raw, or only a cooked, composition is invalid stored
/// data: `food` and `catalog` return `InvalidStoredData` (D28).
#[test]
fn food_with_one_of_raw_and_cooked_is_invalid_stored_data() {
    let (file, db) = open("food_with_one_of_raw_and_cooked_is_invalid_stored_data");
    let conn = raw_connection(file.path());
    let raw_only = raw_food_with_states(&conn, "Synthetic raw only", &["raw"]);
    let cooked_only = raw_food_with_states(&conn, "Synthetic cooked only", &["cooked"]);
    for id in [raw_only, cooked_only] {
        let result = db.food(id);
        assert!(
            matches!(result, Err(Error::InvalidStoredData { .. })),
            "food {id:?}: got {result:?}"
        );
        let result = db.catalog(&[id]);
        assert!(
            matches!(result, Err(Error::InvalidStoredData { .. })),
            "catalog {id:?}: got {result:?}"
        );
    }
}

/// A food without any composition is invalid stored data (D28).
#[test]
fn food_without_composition_is_invalid_stored_data() {
    let (file, db) = open("food_without_composition_is_invalid_stored_data");
    let conn = raw_connection(file.path());
    let id = raw_food_with_states(&conn, "Synthetic empty", &[]);
    let result = db.food(id);
    assert!(
        matches!(result, Err(Error::InvalidStoredData { .. })),
        "got {result:?}"
    );
}

// ---------------------------------------------------------------------------
// foods()
// ---------------------------------------------------------------------------

/// A new database lists no foods (the seed holds none).
#[test]
fn foods_of_new_database_is_empty() {
    let db = Database::open_in_memory().unwrap();
    assert!(db.foods().unwrap().is_empty());
}

/// `foods` lists every food by display name, ignoring case, using the English
/// name when there is no French one, with both names in each summary.
#[test]
fn foods_are_ordered_by_display_name_ignoring_case() {
    let (_file, mut db) = open("foods_are_ordered_by_display_name_ignoring_case");
    let src = user(&db);
    let cerise = insert_named(&mut db, "cerise");
    let abricot = insert_named(&mut db, "Abricot");
    let banane = insert_named(&mut db, "banane");
    let blueberry = db
        .insert_food(&new_food(
            src,
            None,
            Some("Blueberry"),
            NewCompositions::Shared(comp(simple_composition(), None)),
        ))
        .unwrap();
    let foods = db.foods().unwrap();
    // Case-insensitive: abricot < banane < blueberry < cerise. A case-sensitive
    // order would put "Blueberry" before "banane".
    assert_eq!(ids(&foods), vec![abricot, banane, blueberry, cerise]);
    let blue = &foods[2];
    assert_eq!(blue.name_fr, None);
    assert_eq!(blue.name_en.as_deref(), Some("Blueberry"));
    assert_eq!(foods[0].name_fr.as_deref(), Some("Abricot"));
}

/// Foods with the same display name are ordered by id.
#[test]
fn foods_with_equal_names_are_ordered_by_id() {
    let (_file, mut db) = open("foods_with_equal_names_are_ordered_by_id");
    let first = insert_named(&mut db, "Synthetic twin");
    let second = insert_named(&mut db, "SYNTHETIC TWIN");
    let third = insert_named(&mut db, "Synthetic twin");
    assert!(first < second && second < third);
    assert_eq!(ids(&db.foods().unwrap()), vec![first, second, third]);
}

// ---------------------------------------------------------------------------
// search_foods (TECHNOLOGY.md §3)
// ---------------------------------------------------------------------------

/// Ids of the search fixture foods.
struct SearchFixture {
    /// "Crème fraîche".
    creme: FoodId,
    /// "Pâtes cuites".
    pates_cuites: FoodId,
    /// "Pâtes sèches".
    pates_seches: FoodId,
    /// "Pomme" / "Apple".
    pomme: FoodId,
}

/// Inserts the search fixture foods.
///
/// # Arguments
///
/// * `db` - the database to write to.
fn search_fixture(db: &mut Database) -> SearchFixture {
    let creme = insert_named(db, "Crème fraîche");
    let pates_cuites = insert_named(db, "Pâtes cuites");
    let pates_seches = insert_named(db, "Pâtes sèches");
    let src = user(db);
    let pomme = db
        .insert_food(&new_food(
            src,
            Some("Pomme"),
            Some("Apple"),
            NewCompositions::Shared(comp(simple_composition(), None)),
        ))
        .unwrap();
    SearchFixture {
        creme,
        pates_cuites,
        pates_seches,
        pomme,
    }
}

/// Search ignores accents and case: "creme" and "CRÈME" find "Crème fraîche".
#[test]
fn search_ignores_accents_and_case() {
    let (_file, mut db) = open("search_ignores_accents_and_case");
    let f = search_fixture(&mut db);
    assert_eq!(search(&db, "creme", 10), vec![f.creme]);
    assert_eq!(search(&db, "CRÈME", 10), vec![f.creme]);
}

/// A word prefix matches: "fraich" finds "Crème fraîche".
#[test]
fn search_matches_word_prefixes() {
    let (_file, mut db) = open("search_matches_word_prefixes");
    let f = search_fixture(&mut db);
    assert_eq!(search(&db, "fraich", 10), vec![f.creme]);
}

/// Every word must match: "pat cuit" finds "Pâtes cuites" but not "Pâtes
/// sèches", while "pat" alone finds both.
#[test]
fn search_requires_every_word() {
    let (_file, mut db) = open("search_requires_every_word");
    let f = search_fixture(&mut db);
    assert_eq!(search(&db, "pat cuit", 10), vec![f.pates_cuites]);
    let mut both = search(&db, "pat", 10);
    both.sort();
    assert_eq!(both, vec![f.pates_cuites, f.pates_seches]);
}

/// English names are searched, and the summary holds both names.
#[test]
fn search_covers_english_names() {
    let (_file, mut db) = open("search_covers_english_names");
    let f = search_fixture(&mut db);
    let found = db.search_foods("apple", 10).unwrap();
    assert_eq!(ids(&found), vec![f.pomme]);
    assert_eq!(found[0].name_fr.as_deref(), Some("Pomme"));
    assert_eq!(found[0].name_en.as_deref(), Some("Apple"));
}

/// Text that matches no food gives an empty list.
#[test]
fn search_without_match_is_empty() {
    let (_file, mut db) = open("search_without_match_is_empty");
    search_fixture(&mut db);
    assert!(search(&db, "zzzz", 10).is_empty());
}

/// Blank text ("" or spaces) gives an empty list, not every food.
#[test]
fn search_with_blank_text_is_empty() {
    let (_file, mut db) = open("search_with_blank_text_is_empty");
    search_fixture(&mut db);
    assert!(search(&db, "", 10).is_empty());
    assert!(search(&db, "   ", 10).is_empty());
}

/// At most `limit` results are returned. Assumption: limit 0 gives an empty
/// list (the brief only says the limit is applied).
#[test]
fn search_applies_the_limit() {
    let (_file, mut db) = open("search_applies_the_limit");
    for i in 1..=3 {
        insert_named(&mut db, &format!("Synthetic lentille {i}"));
    }
    assert_eq!(search(&db, "lentille", 10).len(), 3);
    assert_eq!(search(&db, "lentille", 2).len(), 2);
    assert_eq!(search(&db, "lentille", 1).len(), 1);
    assert!(search(&db, "lentille", 0).is_empty());
}

/// Text containing FTS5 query syntax never makes the search fail.
#[test]
fn search_with_fts5_syntax_never_fails() {
    let (_file, mut db) = open("search_with_fts5_syntax_never_fails");
    search_fixture(&mut db);
    let inputs = [
        "\"",
        "\"crème",
        "crème\"",
        "\"\"",
        "*",
        "cr*me",
        "creme*",
        "-",
        "-creme",
        "creme -pates",
        ":",
        "name_fr:creme",
        "name_fr:",
        "{name_fr name_en}: creme",
        "(",
        ")",
        "(creme",
        "creme)",
        "()",
        "AND",
        "OR",
        "NOT",
        "NEAR",
        "creme AND",
        "OR creme",
        "creme NOT pates",
        "NEAR(creme pates)",
        "NEAR(creme pates, 2)",
        "^",
        "^creme",
        "+",
        "'",
        "creme'",
        ",",
        ".",
        "\\",
        "%",
        "_",
    ];
    for text in inputs {
        if let Err(e) = db.search_foods(text, 10) {
            panic!("search_foods({text:?}) failed: {e:?}");
        }
    }
}

/// Operator words are searched as plain words: a food named "Synthetic and
/// or near not" is found by "and or near not". Assumption: user input is
/// searched literally, as the brief implies.
#[test]
fn search_treats_operator_words_as_text() {
    let (_file, mut db) = open("search_treats_operator_words_as_text");
    let food = insert_named(&mut db, "Synthetic and or near not");
    insert_named(&mut db, "Synthetic other");
    assert_eq!(search(&db, "AND OR NEAR NOT", 10), vec![food]);
    assert_eq!(search(&db, "-synthetic and", 10), vec![food]);
}

// ---------------------------------------------------------------------------
// delete_food
// ---------------------------------------------------------------------------

/// Deleting a food removes it, its compositions, and their values; it is then
/// not found and no longer searchable, and other foods are untouched.
#[test]
fn delete_food_removes_food_compositions_and_values() {
    let (file, mut db) = open("delete_food_removes_food_compositions_and_values");
    let src = user(&db);
    let id = db
        .insert_food(&new_food(
            src,
            Some("Synthetic quokka"),
            None,
            NewCompositions::Distinct {
                raw: comp(all_states_composition(), None),
                cooked: comp(simple_composition(), None),
            },
        ))
        .unwrap();
    let kept = insert_named(&mut db, "Synthetic kept");
    // all_states has 7 records, simple has 1, kept has 1: 9 values.
    assert_counts(&file, 2, 3, 9);

    db.delete_food(id).unwrap();

    assert_counts(&file, 1, 1, 1);
    match db.food(id) {
        Err(Error::FoodNotFound { food_id }) => assert_eq!(food_id, id),
        other => panic!("expected FoodNotFound, got {other:?}"),
    }
    assert!(search(&db, "quokka", 10).is_empty());
    assert_eq!(ids(&db.foods().unwrap()), vec![kept]);
    assert_eq!(db.food(kept).unwrap().name(), "Synthetic kept");
}

/// A food used by a recipe item cannot be deleted (`FoodInUse`) and stays
/// loadable; once the recipe is gone it can be deleted.
#[test]
fn delete_food_used_by_recipe_is_rejected() {
    let (file, mut db) = open("delete_food_used_by_recipe_is_rejected");
    let id = insert_named(&mut db, "Synthetic used food");
    let conn = raw_connection(file.path());
    conn.execute(
        "INSERT INTO recipe (name, servings) VALUES ('Synthetic recipe', 1)",
        [],
    )
    .unwrap();
    let recipe = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO recipe_item VALUES (?1, 0, ?2, 'raw', 100)",
        [recipe, id.0],
    )
    .unwrap();

    match db.delete_food(id) {
        Err(Error::FoodInUse { food_id }) => assert_eq!(food_id, id),
        other => panic!("expected FoodInUse, got {other:?}"),
    }
    assert_eq!(db.food(id).unwrap().name(), "Synthetic used food");
    assert_counts(&file, 1, 1, 1);

    conn.execute("DELETE FROM recipe WHERE id = ?1", [recipe])
        .unwrap();
    db.delete_food(id).unwrap();
    assert_counts(&file, 0, 0, 0);
}

/// Deleting a missing food gives `FoodNotFound`.
#[test]
fn delete_missing_food_is_not_found() {
    let mut db = Database::open_in_memory().unwrap();
    match db.delete_food(FoodId(9999)) {
        Err(Error::FoodNotFound { food_id }) => assert_eq!(food_id, FoodId(9999)),
        other => panic!("expected FoodNotFound, got {other:?}"),
    }
}
