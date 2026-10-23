-- Schema version 1: sources, nutrients, foods with their compositions, recipes (D23).
-- Amounts are integer fixed-point, thousandths of the canonical unit (D1).
-- Quantities and servings are 1..=4294967295, the range of core Grams and Servings (u32).
-- Timestamps are UTC text, YYYY-MM-DDTHH:MM:SSZ.

CREATE TABLE data_source (
    id           INTEGER PRIMARY KEY,
    name         TEXT NOT NULL UNIQUE CHECK (name <> ''),
    version      TEXT,
    source_url   TEXT,
    licence      TEXT,
    retrieved_at TEXT
) STRICT;

-- Canonical unit codes (D14): g, mg, ug (µg), kJ, kcal.
CREATE TABLE nutrient (
    id             INTEGER PRIMARY KEY,
    code           TEXT NOT NULL UNIQUE CHECK (code <> ''),
    name           TEXT NOT NULL CHECK (name <> ''),
    canonical_unit TEXT NOT NULL CHECK (canonical_unit IN ('g', 'mg', 'ug', 'kJ', 'kcal'))
) STRICT;

CREATE TABLE food (
    id          INTEGER PRIMARY KEY,
    source_id   INTEGER NOT NULL REFERENCES data_source (id) ON DELETE RESTRICT,
    name        TEXT NOT NULL CHECK (name <> ''),
    category    TEXT,
    description TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    -- Target of the composite foreign key from food_composition.
    UNIQUE (id, source_id)
) STRICT;

CREATE INDEX food_source_id ON food (source_id);

-- One row per state, or a single 'shared' row when the source does not
-- distinguish raw and cooked (D9, Compositions::Shared).
-- (source_id, source_record_id) is the natural key of imported records (D2);
-- user foods have no source record.
CREATE TABLE food_composition (
    id                   INTEGER PRIMARY KEY,
    food_id              INTEGER NOT NULL,
    source_id            INTEGER NOT NULL,
    state                TEXT NOT NULL CHECK (state IN ('raw', 'cooked', 'shared')),
    source_record_id     TEXT CHECK (source_record_id <> ''),
    reference_quantity_g INTEGER NOT NULL DEFAULT 100
        CHECK (reference_quantity_g BETWEEN 1 AND 4294967295),
    UNIQUE (food_id, state),
    UNIQUE (source_id, source_record_id),
    FOREIGN KEY (food_id, source_id) REFERENCES food (id, source_id) ON DELETE CASCADE
) STRICT;

-- The triggers ignore rows with the same state: UNIQUE (food_id, state) handles
-- those, so an upsert of an existing composition is not blocked.
CREATE TRIGGER food_composition_shared_is_alone_on_insert
BEFORE INSERT ON food_composition
WHEN EXISTS (
    SELECT 1 FROM food_composition
    WHERE food_id = NEW.food_id AND state <> NEW.state
      AND (state = 'shared' OR NEW.state = 'shared')
)
BEGIN
    SELECT RAISE(ABORT, 'a shared composition cannot coexist with another composition of the same food');
END;

CREATE TRIGGER food_composition_shared_is_alone_on_update
BEFORE UPDATE OF food_id, state ON food_composition
WHEN EXISTS (
    SELECT 1 FROM food_composition
    WHERE food_id = NEW.food_id AND id <> OLD.id AND state <> NEW.state
      AND (state = 'shared' OR NEW.state = 'shared')
)
BEGIN
    SELECT RAISE(ABORT, 'a shared composition cannot coexist with another composition of the same food');
END;

-- A missing row means unknown; an explicit 'unknown' row keeps the source's marker (D8).
-- 'known' amounts are positive (a known 0 is 'zero', D18); 'below_limit' holds the limit.
CREATE TABLE food_nutrient_value (
    composition_id INTEGER NOT NULL REFERENCES food_composition (id) ON DELETE CASCADE,
    nutrient_id    INTEGER NOT NULL REFERENCES nutrient (id) ON DELETE RESTRICT,
    value_state    TEXT NOT NULL
        CHECK (value_state IN ('known', 'zero', 'trace', 'below_limit', 'unknown')),
    amount         INTEGER,
    PRIMARY KEY (composition_id, nutrient_id),
    CHECK (
        -- IS NOT NULL is needed: a comparison with NULL is NULL, which CHECK accepts.
        (value_state = 'known' AND amount IS NOT NULL AND amount > 0)
        OR (value_state = 'below_limit' AND amount IS NOT NULL AND amount >= 0)
        OR (value_state IN ('zero', 'trace', 'unknown') AND amount IS NULL)
    )
) STRICT, WITHOUT ROWID;

CREATE INDEX food_nutrient_value_nutrient_id ON food_nutrient_value (nutrient_id);

CREATE TABLE recipe (
    id              INTEGER PRIMARY KEY,
    name            TEXT NOT NULL CHECK (name <> ''),
    notes           TEXT,
    servings        INTEGER NOT NULL CHECK (servings BETWEEN 1 AND 4294967295),
    cooked_weight_g INTEGER CHECK (cooked_weight_g BETWEEN 1 AND 4294967295),
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
) STRICT;

-- Items are identified by their 0-based position; a food may appear twice (D17).
CREATE TABLE recipe_item (
    recipe_id  INTEGER NOT NULL REFERENCES recipe (id) ON DELETE CASCADE,
    position   INTEGER NOT NULL CHECK (position >= 0),
    food_id    INTEGER NOT NULL REFERENCES food (id) ON DELETE RESTRICT,
    state      TEXT NOT NULL CHECK (state IN ('raw', 'cooked')),
    quantity_g INTEGER NOT NULL CHECK (quantity_g BETWEEN 1 AND 4294967295),
    PRIMARY KEY (recipe_id, position)
) STRICT, WITHOUT ROWID;

CREATE INDEX recipe_item_food_id ON recipe_item (food_id);

-- Seed data: the data source of user-created foods (D25) and the initial
-- nutrient set (D7, D14), in the order of nutrition_core::initial_nutrients().
INSERT INTO data_source (name) VALUES ('user');

INSERT INTO nutrient (code, name, canonical_unit) VALUES
    ('energy_kj', 'Energy', 'kJ'),
    ('energy_kcal', 'Energy', 'kcal'),
    ('protein', 'Protein', 'g'),
    ('carbohydrate_available', 'Available carbohydrate', 'g'),
    ('sugars', 'Sugars', 'g'),
    ('fibre', 'Fibre', 'g'),
    ('fat', 'Fat', 'g'),
    ('fat_saturated', 'Saturated fat', 'g'),
    ('salt', 'Salt', 'g'),
    ('calcium', 'Calcium', 'mg'),
    ('iron', 'Iron', 'mg'),
    ('vitamin_c', 'Vitamin C', 'mg'),
    ('vitamin_d', 'Vitamin D', 'ug'),
    ('vitamin_b12', 'Vitamin B12', 'ug');
