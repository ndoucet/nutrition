use std::collections::{BTreeSet, HashMap};

use nutrition_core::{Composition, Compositions, Food, FoodCatalog, FoodId, Grams, NutrientValue};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

use crate::error::{sqlite, Error};
use crate::Database;

/// Name of the data source that holds the foods created by the user (D25).
pub const USER_SOURCE: &str = "user";

/// Integer identifier of a data source (D2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceId(pub i64);

/// A data source to store, with its provenance fields (DATA_MODEL.md §5).
///
/// Optional fields are absent when not applicable; when present they must not be blank.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NewDataSource {
    pub name: String,
    pub version: Option<String>,
    pub source_url: Option<String>,
    pub licence: Option<String>,
    pub retrieved_at: Option<String>,
}

/// A composition to store, with the source's identifier of the record it comes
/// from; user-created foods have none (D2, D25).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewComposition {
    pub composition: Composition,
    pub source_record_id: Option<String>,
}

/// The compositions of a food to store: one shared by both states, or one per
/// state (D9, D28). Mirrors [`Compositions`] with provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewCompositions {
    Shared(NewComposition),
    Distinct {
        raw: NewComposition,
        cooked: NewComposition,
    },
}

/// A food to store. At least one of the names is required (D31); optional
/// text fields, when present, must not be blank.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewFood {
    pub source: SourceId,
    pub name_fr: Option<String>,
    pub name_en: Option<String>,
    pub category: Option<String>,
    pub description: Option<String>,
    pub compositions: NewCompositions,
}

/// A food's identifier and names, as listed by [`Database::foods`] and
/// [`Database::search_foods`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoodSummary {
    pub id: FoodId,
    pub name_fr: Option<String>,
    pub name_en: Option<String>,
}

impl Database {
    /// The identifier of the data source named `name`.
    ///
    /// # Arguments
    ///
    /// * `name` - exact name of the data source, e.g. [`USER_SOURCE`].
    pub fn data_source_id(&self, name: &str) -> Result<SourceId, Error> {
        self.conn
            .query_row(
                "SELECT id FROM data_source WHERE name = ?1",
                [name],
                |row| row.get(0),
            )
            .optional()
            .map_err(sqlite("read a data source"))?
            .map(SourceId)
            .ok_or_else(|| Error::UnknownDataSource {
                name: name.to_owned(),
            })
    }

    /// Stores a new data source and returns its identifier.
    ///
    /// # Arguments
    ///
    /// * `source` - the data source; its name must be unique and not blank.
    pub fn insert_data_source(&mut self, source: &NewDataSource) -> Result<SourceId, Error> {
        require_text("data source name", &source.name)?;
        for (field, value) in [
            ("data source version", &source.version),
            ("data source URL", &source.source_url),
            ("data source licence", &source.licence),
            ("data source retrieval date", &source.retrieved_at),
        ] {
            require_optional_text(field, value)?;
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite("start inserting a data source"))?;
        let exists: bool = tx
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM data_source WHERE name = ?1)",
                [&source.name],
                |row| row.get(0),
            )
            .map_err(sqlite("check a data source name"))?;
        if exists {
            return Err(Error::DuplicateDataSource {
                name: source.name.clone(),
            });
        }
        tx.execute(
            "INSERT INTO data_source (name, version, source_url, licence, retrieved_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                source.name,
                source.version,
                source.source_url,
                source.licence,
                source.retrieved_at
            ],
        )
        .map_err(sqlite("insert a data source"))?;
        let id = tx.last_insert_rowid();
        tx.commit().map_err(sqlite("commit a data source"))?;
        Ok(SourceId(id))
    }

    /// Stores a food with its compositions and nutrient values in one
    /// transaction, and returns its identifier. Nothing is written on error.
    ///
    /// A food of the [`USER_SOURCE`] has no source record identifiers and its
    /// compositions are per 100 g (D25).
    ///
    /// # Arguments
    ///
    /// * `food` - the food; amounts are in thousandths of each nutrient's canonical unit per reference quantity.
    pub fn insert_food(&mut self, food: &NewFood) -> Result<FoodId, Error> {
        validate_new_food(food)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite("start inserting a food"))?;
        let source_name: Option<String> = tx
            .query_row(
                "SELECT name FROM data_source WHERE id = ?1",
                [food.source.0],
                |row| row.get(0),
            )
            .optional()
            .map_err(sqlite("check a data source"))?;
        match source_name.as_deref() {
            None => {
                return Err(Error::UnknownDataSourceId {
                    source_id: food.source.0,
                })
            }
            Some(USER_SOURCE) => validate_user_food(food)?,
            Some(_) => {}
        }
        let nutrient_ids = nutrient_ids(&tx)?;
        tx.execute(
            "INSERT INTO food (source_id, name_fr, name_en, category, description)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                food.source.0,
                food.name_fr,
                food.name_en,
                food.category,
                food.description
            ],
        )
        .map_err(sqlite("insert a food"))?;
        let food_id = tx.last_insert_rowid();
        let compositions: Vec<(&str, &NewComposition)> = match &food.compositions {
            NewCompositions::Shared(shared) => vec![("shared", shared)],
            NewCompositions::Distinct { raw, cooked } => vec![("raw", raw), ("cooked", cooked)],
        };
        for (state, composition) in compositions {
            insert_composition(&tx, food_id, food.source, state, composition, &nutrient_ids)?;
        }
        tx.commit().map_err(sqlite("commit a food"))?;
        Ok(FoodId(food_id))
    }

    /// The food with identifier `id`, with its compositions.
    ///
    /// The core [`Food`]'s name is the French name, or the English name when
    /// there is no French one (D31). A food with no composition, or with only
    /// one of raw and cooked, is invalid stored data (D28).
    ///
    /// # Arguments
    ///
    /// * `id` - identifier of the food.
    pub fn food(&self, id: FoodId) -> Result<Food, Error> {
        // One read transaction, so a concurrent change cannot be seen half done.
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(sqlite("start reading a food"))?;
        let food = load_food(&tx, id)?;
        tx.commit().map_err(sqlite("finish reading a food"))?;
        Ok(food)
    }

    /// The foods with the given identifiers, for a calculation. Repeated
    /// identifiers are loaded once.
    ///
    /// # Arguments
    ///
    /// * `ids` - identifiers of the foods; each must exist.
    pub fn catalog(&self, ids: &[FoodId]) -> Result<FoodCatalog, Error> {
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(sqlite("start reading foods"))?;
        let mut catalog = FoodCatalog::new();
        for id in ids.iter().copied().collect::<BTreeSet<_>>() {
            catalog
                .insert(load_food(&tx, id)?)
                .map_err(|err| Error::InvalidStoredData {
                    table: "food",
                    detail: err.to_string(),
                })?;
        }
        tx.commit().map_err(sqlite("finish reading foods"))?;
        Ok(catalog)
    }

    /// All foods, ordered by display name ignoring case and accents, then by
    /// identifier (D32).
    pub fn foods(&self) -> Result<Vec<FoodSummary>, Error> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT id, name_fr, name_en FROM food")
            .map_err(sqlite("list the foods"))?;
        let mut foods = stmt
            .query_map([], summary_from_row)
            .map_err(sqlite("list the foods"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sqlite("list the foods"))?;
        foods.sort_by_cached_key(|food| (sort_key(display_name(food)), food.id));
        Ok(foods)
    }

    /// Foods whose names match every word of `text` as a word prefix, ignoring
    /// accents and case, best matches first, then in the order of [`Database::foods`] (D32).
    ///
    /// Search syntax in `text` has no effect: each word is matched literally.
    /// Blank text matches nothing.
    ///
    /// # Arguments
    ///
    /// * `text` - words typed by the user, in French or English.
    /// * `limit` - maximum number of results.
    pub fn search_foods(&self, text: &str, limit: usize) -> Result<Vec<FoodSummary>, Error> {
        let Some(query) = fts_query(text) else {
            return Ok(Vec::new());
        };
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT food.id, food.name_fr, food.name_en, bm25(food_fts)
                 FROM food_fts JOIN food ON food.id = food_fts.rowid
                 WHERE food_fts MATCH ?1",
            )
            .map_err(sqlite("search the foods"))?;
        let mut matches = stmt
            .query_map([query], |row| {
                Ok((summary_from_row(row)?, row.get::<_, f64>(3)?))
            })
            .map_err(sqlite("search the foods"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sqlite("search the foods"))?;
        // Sorted here rather than in SQL, so ties use the same key as `foods`.
        // A lower bm25 score is a better match.
        let mut keyed: Vec<_> = matches
            .drain(..)
            .map(|(food, rank)| (rank, sort_key(display_name(&food)), food))
            .collect();
        keyed.sort_by(|a, b| {
            a.0.total_cmp(&b.0)
                .then_with(|| a.1.cmp(&b.1))
                .then_with(|| a.2.id.cmp(&b.2.id))
        });
        Ok(keyed
            .into_iter()
            .take(limit)
            .map(|(_, _, food)| food)
            .collect())
    }

    /// Deletes a food with its compositions and values. A food used by a
    /// recipe cannot be deleted.
    ///
    /// # Arguments
    ///
    /// * `id` - identifier of the food.
    pub fn delete_food(&mut self, id: FoodId) -> Result<(), Error> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite("start deleting a food"))?;
        let (exists, in_use): (bool, bool) = tx
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM food WHERE id = ?1),
                        EXISTS (SELECT 1 FROM recipe_item WHERE food_id = ?1)",
                [id.0],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(sqlite("check a food before deleting it"))?;
        if !exists {
            return Err(Error::FoodNotFound { food_id: id });
        }
        if in_use {
            return Err(Error::FoodInUse { food_id: id });
        }
        tx.execute("DELETE FROM food WHERE id = ?1", [id.0])
            .map_err(sqlite("delete a food"))?;
        tx.commit().map_err(sqlite("commit deleting a food"))
    }
}

/// The food with identifier `id`, read with `conn` (D28, D31).
///
/// # Arguments
///
/// * `conn` - connection or open transaction to read from.
/// * `id` - identifier of the food.
fn load_food(conn: &Connection, id: FoodId) -> Result<Food, Error> {
    let names: Option<(Option<String>, Option<String>)> = conn
        .prepare_cached("SELECT name_fr, name_en FROM food WHERE id = ?1")
        .and_then(|mut stmt| {
            stmt.query_row([id.0], |row| Ok((row.get(0)?, row.get(1)?)))
                .optional()
        })
        .map_err(sqlite("read a food"))?;
    let (name_fr, name_en) = names.ok_or(Error::FoodNotFound { food_id: id })?;
    let name = preferred_name(name_fr.as_deref(), name_en.as_deref())
        .ok_or_else(|| Error::InvalidStoredData {
            table: "food",
            detail: format!("food {id} has no name"),
        })?
        .to_owned();
    Ok(Food::new(id, name, load_compositions(conn, id)?))
}

/// The compositions of food `id`, checked against the shapes of D28.
///
/// # Arguments
///
/// * `conn` - connection or open transaction to read from.
/// * `id` - identifier of an existing food.
fn load_compositions(conn: &Connection, id: FoodId) -> Result<Compositions, Error> {
    let mut stmt = conn
        .prepare_cached(
            "SELECT id, state, reference_quantity_g FROM food_composition
             WHERE food_id = ?1 ORDER BY state",
        )
        .map_err(sqlite("read the compositions of a food"))?;
    let rows = stmt
        .query_map([id.0], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .map_err(sqlite("read the compositions of a food"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sqlite("read the compositions of a food"))?;
    let states: Vec<String> = rows.iter().map(|(_, state, _)| state.clone()).collect();
    let mut by_state = HashMap::new();
    for (composition_id, state, reference_quantity) in rows {
        let reference_quantity =
            Grams::new(reference_quantity).map_err(|err| Error::InvalidStoredData {
                table: "food_composition",
                detail: err.to_string(),
            })?;
        by_state.insert(
            state,
            load_composition(conn, composition_id, reference_quantity)?,
        );
    }
    let shared = by_state.remove("shared");
    let raw = by_state.remove("raw");
    let cooked = by_state.remove("cooked");
    match (shared, raw, cooked) {
        (Some(shared), None, None) if by_state.is_empty() => Ok(Compositions::Shared(shared)),
        (None, Some(raw), Some(cooked)) if by_state.is_empty() => {
            Ok(Compositions::Distinct { raw, cooked })
        }
        _ => Err(Error::InvalidStoredData {
            table: "food_composition",
            detail: format!(
                "food {id} has compositions {states:?}; expected one shared, or raw and cooked"
            ),
        }),
    }
}

/// The nutrient values of one stored composition.
///
/// # Arguments
///
/// * `conn` - connection or open transaction to read from.
/// * `composition_id` - row identifier in `food_composition`.
/// * `reference_quantity` - quantity of edible portion the values refer to, in grams.
fn load_composition(
    conn: &Connection,
    composition_id: i64,
    reference_quantity: Grams,
) -> Result<Composition, Error> {
    let mut stmt = conn
        .prepare_cached(
            "SELECT nutrient.code, value.value_state, value.amount
             FROM food_nutrient_value AS value
             JOIN nutrient ON nutrient.id = value.nutrient_id
             WHERE value.composition_id = ?1",
        )
        .map_err(sqlite("read nutrient values"))?;
    let rows = stmt
        .query_map([composition_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<i64>>(2)?,
            ))
        })
        .map_err(sqlite("read nutrient values"))?;
    let mut composition = Composition::new(reference_quantity);
    for row in rows {
        let (code, state, amount) = row.map_err(sqlite("read nutrient values"))?;
        composition.set(code, value_from_row(&state, amount)?);
    }
    Ok(composition)
}

/// Checks the input rules of [`NewFood`] that do not need the database.
///
/// # Arguments
///
/// * `food` - the food to check.
fn validate_new_food(food: &NewFood) -> Result<(), Error> {
    if food.name_fr.is_none() && food.name_en.is_none() {
        return Err(Error::InvalidInput {
            detail: "a food needs a French or an English name".to_owned(),
        });
    }
    require_optional_text("French name", &food.name_fr)?;
    require_optional_text("English name", &food.name_en)?;
    require_optional_text("category", &food.category)?;
    require_optional_text("description", &food.description)?;
    match &food.compositions {
        NewCompositions::Shared(shared) => {
            require_optional_text("source record id", &shared.source_record_id)?;
        }
        NewCompositions::Distinct { raw, cooked } => {
            require_optional_text("source record id", &raw.source_record_id)?;
            require_optional_text("source record id", &cooked.source_record_id)?;
            if let (Some(record), Some(other)) = (&raw.source_record_id, &cooked.source_record_id) {
                if record == other {
                    return Err(Error::DuplicateSourceRecord {
                        source_id: food.source.0,
                        source_record_id: record.clone(),
                    });
                }
            }
        }
    }
    Ok(())
}

/// Checks the rules of D25 for a food of the [`USER_SOURCE`]: no source record
/// identifiers, and compositions per 100 g.
///
/// # Arguments
///
/// * `food` - the food to check.
fn validate_user_food(food: &NewFood) -> Result<(), Error> {
    let compositions = match &food.compositions {
        NewCompositions::Shared(shared) => vec![shared],
        NewCompositions::Distinct { raw, cooked } => vec![raw, cooked],
    };
    for new in compositions {
        if new.source_record_id.is_some() {
            return Err(Error::InvalidInput {
                detail: "a user food has no source record id".to_owned(),
            });
        }
        if new.composition.reference_quantity() != Grams::HUNDRED {
            return Err(Error::InvalidInput {
                detail: format!(
                    "a user food is entered per 100 g, not per {}",
                    new.composition.reference_quantity()
                ),
            });
        }
    }
    Ok(())
}

/// Rejects a blank text field.
///
/// # Arguments
///
/// * `field` - name of the field, for the error message.
/// * `value` - the text to check.
fn require_text(field: &str, value: &str) -> Result<(), Error> {
    if value.trim().is_empty() {
        Err(Error::InvalidInput {
            detail: format!("the {field} must not be blank"),
        })
    } else {
        Ok(())
    }
}

/// Rejects an optional text field that is present but blank.
///
/// # Arguments
///
/// * `field` - name of the field, for the error message.
/// * `value` - the optional text to check.
fn require_optional_text(field: &str, value: &Option<String>) -> Result<(), Error> {
    value
        .as_deref()
        .map_or(Ok(()), |text| require_text(field, text))
}

/// The nutrient identifiers by code.
///
/// # Arguments
///
/// * `tx` - the open transaction.
fn nutrient_ids(tx: &Transaction<'_>) -> Result<HashMap<String, i64>, Error> {
    let mut stmt = tx
        .prepare_cached("SELECT code, id FROM nutrient")
        .map_err(sqlite("read the nutrients"))?;
    let ids = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(sqlite("read the nutrients"))?
        .collect::<Result<HashMap<_, _>, _>>()
        .map_err(sqlite("read the nutrients"))?;
    Ok(ids)
}

/// Inserts one composition and its values.
///
/// # Arguments
///
/// * `tx` - the open transaction.
/// * `food_id` - row identifier of the food just inserted.
/// * `source` - the food's data source.
/// * `state` - `shared`, `raw`, or `cooked` (D28).
/// * `new` - the composition and its source record identifier.
/// * `nutrient_ids` - nutrient identifiers by code.
fn insert_composition(
    tx: &Transaction<'_>,
    food_id: i64,
    source: SourceId,
    state: &str,
    new: &NewComposition,
    nutrient_ids: &HashMap<String, i64>,
) -> Result<(), Error> {
    if let Some(record) = &new.source_record_id {
        let taken: bool = tx
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM food_composition
                                WHERE source_id = ?1 AND source_record_id = ?2)",
                params![source.0, record],
                |row| row.get(0),
            )
            .map_err(sqlite("check a source record id"))?;
        if taken {
            return Err(Error::DuplicateSourceRecord {
                source_id: source.0,
                source_record_id: record.clone(),
            });
        }
    }
    tx.execute(
        "INSERT INTO food_composition (food_id, source_id, state, source_record_id, reference_quantity_g)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            food_id,
            source.0,
            state,
            new.source_record_id,
            i64::from(new.composition.reference_quantity().get())
        ],
    )
    .map_err(sqlite("insert a composition"))?;
    let composition_id = tx.last_insert_rowid();
    // Sorted so that the first unknown nutrient reported does not depend on hash order.
    let mut values: Vec<_> = new.composition.values().collect();
    values.sort_unstable_by_key(|(code, _)| *code);
    for (code, value) in values {
        let nutrient_id = nutrient_ids
            .get(code)
            .ok_or_else(|| Error::UnknownNutrient {
                code: code.to_owned(),
            })?;
        let (state, amount) = value_to_row(value);
        tx.execute(
            "INSERT INTO food_nutrient_value (composition_id, nutrient_id, value_state, amount)
             VALUES (?1, ?2, ?3, ?4)",
            params![composition_id, nutrient_id, state, amount],
        )
        .map_err(sqlite("insert a nutrient value"))?;
    }
    Ok(())
}

/// The stored `value_state` and `amount` of a value (DATA_MODEL.md §4).
///
/// # Arguments
///
/// * `value` - the value; amounts are in thousandths of the canonical unit.
fn value_to_row(value: NutrientValue) -> (&'static str, Option<i64>) {
    match value {
        NutrientValue::Known(amount) => ("known", Some(amount.milli())),
        NutrientValue::Zero => ("zero", None),
        NutrientValue::Trace => ("trace", None),
        NutrientValue::BelowLimit { limit } => ("below_limit", Some(limit.milli())),
        NutrientValue::Unknown => ("unknown", None),
    }
}

/// The value stored as `state` and `amount`, through the validating constructors.
///
/// # Arguments
///
/// * `state` - stored `value_state`.
/// * `amount` - stored amount, in thousandths of the canonical unit.
fn value_from_row(state: &str, amount: Option<i64>) -> Result<NutrientValue, Error> {
    let invalid = |detail: String| Error::InvalidStoredData {
        table: "food_nutrient_value",
        detail,
    };
    match (state, amount) {
        ("known", Some(milli)) => {
            NutrientValue::known(milli).map_err(|err| invalid(err.to_string()))
        }
        ("below_limit", Some(limit)) => {
            NutrientValue::below_limit(limit).map_err(|err| invalid(err.to_string()))
        }
        ("zero", None) => Ok(NutrientValue::Zero),
        ("trace", None) => Ok(NutrientValue::Trace),
        ("unknown", None) => Ok(NutrientValue::Unknown),
        _ => Err(invalid(format!(
            "value state {state:?} with amount {amount:?}"
        ))),
    }
}

/// A [`FoodSummary`] from a row of `id, name_fr, name_en`.
///
/// # Arguments
///
/// * `row` - the result row.
fn summary_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<FoodSummary> {
    Ok(FoodSummary {
        id: FoodId(row.get(0)?),
        name_fr: row.get(1)?,
        name_en: row.get(2)?,
    })
}

/// The name shown for a food: French, else English (D31).
///
/// # Arguments
///
/// * `name_fr` - French name, if any.
/// * `name_en` - English name, if any.
fn preferred_name<'a>(name_fr: Option<&'a str>, name_en: Option<&'a str>) -> Option<&'a str> {
    name_fr.or(name_en)
}

/// The display name of a listed food (D31); empty only if the stored row has no name.
///
/// # Arguments
///
/// * `food` - the food summary.
fn display_name(food: &FoodSummary) -> &str {
    preferred_name(food.name_fr.as_deref(), food.name_en.as_deref()).unwrap_or_default()
}

/// The key that orders names ignoring case and the accents of French and
/// other Latin-script names (D32): `Épinard` sorts with `epinard`, `Œuf` as `oeuf`.
///
/// # Arguments
///
/// * `name` - the display name.
fn sort_key(name: &str) -> String {
    let mut key = String::with_capacity(name.len());
    for c in name.chars().flat_map(char::to_lowercase) {
        match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => key.push('a'),
            'æ' => key.push_str("ae"),
            'ç' => key.push('c'),
            'è' | 'é' | 'ê' | 'ë' => key.push('e'),
            'ì' | 'í' | 'î' | 'ï' => key.push('i'),
            'ñ' => key.push('n'),
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' => key.push('o'),
            'œ' => key.push_str("oe"),
            'ù' | 'ú' | 'û' | 'ü' => key.push('u'),
            'ý' | 'ÿ' => key.push('y'),
            other => key.push(other),
        }
    }
    key
}

/// The FTS5 query for the user's text (D32): words are split on every character
/// that is not a letter or digit, as the `unicode61` tokenizer does, so
/// `pomm-de` is two words. Each word is quoted, so search syntax has no effect,
/// and matched as a prefix; all words must match. `None` when there is no word.
///
/// # Arguments
///
/// * `text` - words typed by the user.
fn fts_query(text: &str) -> Option<String> {
    let terms: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(|word| format!("\"{word}\"*"))
        .collect();
    (!terms.is_empty()).then(|| terms.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Words are split on every non-alphanumeric character, quoted, and marked
    /// as prefixes; input without a letter or digit gives no query.
    #[test]
    fn builds_literal_prefix_queries() {
        assert_eq!(fts_query("pat cuit"), Some("\"pat\"* \"cuit\"*".to_owned()));
        assert_eq!(fts_query("a\"b"), Some("\"a\"* \"b\"*".to_owned()));
        assert_eq!(
            fts_query("pomm-de l'huile"),
            Some("\"pomm\"* \"de\"* \"l\"* \"huile\"*".to_owned())
        );
        assert_eq!(fts_query("- * ( )"), None);
        assert_eq!(fts_query("   "), None);
        assert_eq!(fts_query("NEAR"), Some("\"NEAR\"*".to_owned()));
    }

    /// Every value state maps to its stored form and back (DATA_MODEL.md §4).
    #[test]
    fn value_states_round_trip() {
        for value in [
            NutrientValue::known(1_500).unwrap(),
            NutrientValue::Zero,
            NutrientValue::Trace,
            NutrientValue::below_limit(0).unwrap(),
            NutrientValue::below_limit(500).unwrap(),
            NutrientValue::Unknown,
        ] {
            let (state, amount) = value_to_row(value);
            assert_eq!(value_from_row(state, amount).unwrap(), value);
        }
    }

    /// A stored state with an amount that does not fit it is invalid data.
    #[test]
    fn inconsistent_stored_value_is_invalid_data() {
        assert!(matches!(
            value_from_row("zero", Some(5)),
            Err(Error::InvalidStoredData { .. })
        ));
        assert!(matches!(
            value_from_row("known", None),
            Err(Error::InvalidStoredData { .. })
        ));
        assert!(matches!(
            value_from_row("known", Some(-1)),
            Err(Error::InvalidStoredData { .. })
        ));
    }

    /// Names sort ignoring case and accents: accented capitals no longer come
    /// after every ASCII name.
    #[test]
    fn sort_key_ignores_case_and_accents() {
        assert_eq!(sort_key("Épinard"), "epinard");
        assert_eq!(sort_key("Œuf"), "oeuf");
        assert_eq!(sort_key("Crème fraîche"), "creme fraiche");
        let mut names = vec!["Zeste", "Œuf", "Épinard", "endive"];
        names.sort_by_key(|name| sort_key(name));
        assert_eq!(names, vec!["endive", "Épinard", "Œuf", "Zeste"]);
    }
}
