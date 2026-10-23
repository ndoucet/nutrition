-- Schema version 2: French and English food names, and food search (FTS5).
--
-- `food.name` becomes `name_fr`, and `name_en` is added. Either may be absent,
-- but not both, so the table is rebuilt to drop the NOT NULL on the name.
-- The migration runner turns foreign keys off around this transaction (D23),
-- so dropping the old table does not cascade to compositions.

CREATE TABLE food_new (
    id          INTEGER PRIMARY KEY,
    source_id   INTEGER NOT NULL REFERENCES data_source (id) ON DELETE RESTRICT,
    -- trim() removes spaces only; the store also rejects other blank text.
    name_fr     TEXT CHECK (trim(name_fr) <> ''),
    name_en     TEXT CHECK (trim(name_en) <> ''),
    category    TEXT,
    description TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    CHECK (name_fr IS NOT NULL OR name_en IS NOT NULL),
    -- Target of the composite foreign key from food_composition.
    UNIQUE (id, source_id)
) STRICT;

INSERT INTO food_new (id, source_id, name_fr, category, description, created_at)
SELECT id, source_id, name, category, description, created_at FROM food;

DROP TABLE food;
ALTER TABLE food_new RENAME TO food;

CREATE INDEX food_source_id ON food (source_id);

-- Search index over both names, kept in sync by triggers. Accents and case are
-- ignored, so "creme" finds "Crème" (TECHNOLOGY.md §3).
CREATE VIRTUAL TABLE food_fts USING fts5 (
    name_fr,
    name_en,
    content = 'food',
    content_rowid = 'id',
    tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TRIGGER food_fts_after_insert AFTER INSERT ON food BEGIN
    INSERT INTO food_fts (rowid, name_fr, name_en) VALUES (NEW.id, NEW.name_fr, NEW.name_en);
END;

CREATE TRIGGER food_fts_after_delete AFTER DELETE ON food BEGIN
    INSERT INTO food_fts (food_fts, rowid, name_fr, name_en)
    VALUES ('delete', OLD.id, OLD.name_fr, OLD.name_en);
END;

-- The store never uses INSERT OR REPLACE on food: REPLACE does not fire the
-- delete trigger, which would leave stale rows in the index.
CREATE TRIGGER food_fts_after_update AFTER UPDATE OF id, name_fr, name_en ON food BEGIN
    INSERT INTO food_fts (food_fts, rowid, name_fr, name_en)
    VALUES ('delete', OLD.id, OLD.name_fr, OLD.name_en);
    INSERT INTO food_fts (rowid, name_fr, name_en) VALUES (NEW.id, NEW.name_fr, NEW.name_en);
END;

INSERT INTO food_fts (food_fts) VALUES ('rebuild');
