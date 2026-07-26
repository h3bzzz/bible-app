-- user.db, migration 001.
--
-- Ships with full sync identity before a single note exists. Risk #10 in the
-- register: retrofitting stable ids, logical clocks and tombstones onto rows
-- that already hold someone's study notes means a migration that can lose
-- them, and there is no server to repair the damage. The cost of doing it now
-- is four columns per table.
--
-- ADR-0006 records the conflict model: last-write-wins per field resolved by
-- hybrid logical clock, with a losing note body preserved as a conflicted copy
-- rather than discarded.

CREATE TABLE device (
    id         BLOB PRIMARY KEY,   -- UUIDv4, generated on first run
    label      TEXT NOT NULL,
    created_at TEXT NOT NULL
) STRICT;

-- Every user-data table carries this identity block:
--   id         BLOB  UUIDv7 -- sortable, collision-free, generable offline
--   device_id  BLOB         -- origin device
--   hlc        TEXT         -- hybrid logical clock, lexicographically sortable
--   created_at / updated_at TEXT -- wall clock, for humans, never for merge
--   deleted_at TEXT NULL    -- tombstone; rows are never hard-deleted

CREATE TABLE note (
    id            BLOB PRIMARY KEY,
    device_id     BLOB NOT NULL REFERENCES device(id),
    hlc           TEXT NOT NULL,

    -- Anchor. Canonical scheme always (ADR-0004), so that changing the
    -- displayed translation never moves a note.
    ref_scheme    TEXT    NOT NULL DEFAULT 'org' CHECK (ref_scheme = 'org'),
    start_book    TEXT    NOT NULL,
    start_chapter INTEGER NOT NULL,
    start_verse   INTEGER NOT NULL,
    end_book      TEXT    NOT NULL,
    end_chapter   INTEGER NOT NULL,
    end_verse     INTEGER NOT NULL,
    -- Provenance only. May name a translation that is no longer installed.
    authored_in   TEXT,

    -- Body. LWW plain text now; the discriminator and the spare column mean a
    -- CRDT upgrade later is a feature, not a migration of row identity.
    body_encoding TEXT NOT NULL DEFAULT 'text' CHECK (body_encoding IN ('text', 'crdt')),
    body_text     TEXT,
    body_crdt     BLOB,

    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL,
    deleted_at    TEXT
) STRICT;

CREATE INDEX ix_note_anchor
    ON note (start_book, start_chapter, start_verse) WHERE deleted_at IS NULL;

CREATE TABLE highlight (
    id            BLOB PRIMARY KEY,
    device_id     BLOB NOT NULL REFERENCES device(id),
    hlc           TEXT NOT NULL,
    ref_scheme    TEXT    NOT NULL DEFAULT 'org' CHECK (ref_scheme = 'org'),
    start_book    TEXT    NOT NULL,
    start_chapter INTEGER NOT NULL,
    start_verse   INTEGER NOT NULL,
    end_book      TEXT    NOT NULL,
    end_chapter   INTEGER NOT NULL,
    end_verse     INTEGER NOT NULL,
    -- Character offsets into the flattened canonical verse text, not paths
    -- into the document tree: tree paths break on any re-import, offsets into
    -- normalised text survive one.
    char_start    INTEGER,
    char_end      INTEGER,
    colour        TEXT NOT NULL,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL,
    deleted_at    TEXT
) STRICT;

CREATE INDEX ix_highlight_anchor
    ON highlight (start_book, start_chapter, start_verse) WHERE deleted_at IS NULL;

CREATE TABLE bookmark (
    id            BLOB PRIMARY KEY,
    device_id     BLOB NOT NULL REFERENCES device(id),
    hlc           TEXT NOT NULL,
    ref_scheme    TEXT    NOT NULL DEFAULT 'org' CHECK (ref_scheme = 'org'),
    start_book    TEXT    NOT NULL,
    start_chapter INTEGER NOT NULL,
    start_verse   INTEGER NOT NULL,
    label         TEXT,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL,
    deleted_at    TEXT
) STRICT;

-- Which annotation layers the user has enabled (ADR-0009). Interpretive layers
-- default off; this table records the user overriding that.
CREATE TABLE layer_preference (
    layer_id   TEXT PRIMARY KEY,
    enabled    INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    hlc        TEXT NOT NULL,
    updated_at TEXT NOT NULL
) STRICT;

-- The sync substrate. Written in the same transaction as the row it describes.
-- Nothing reads it in v1; it exists so that enabling sync later is a feature
-- rather than a migration.
CREATE TABLE oplog (
    seq       INTEGER PRIMARY KEY AUTOINCREMENT,
    entity    TEXT NOT NULL CHECK (entity IN ('note', 'highlight', 'bookmark', 'layer_preference')),
    entity_id BLOB NOT NULL,
    hlc       TEXT NOT NULL,
    device_id BLOB NOT NULL,
    op        TEXT NOT NULL CHECK (op IN ('upsert', 'delete')),
    payload   BLOB NOT NULL,
    UNIQUE (entity, entity_id, hlc)
) STRICT;

CREATE INDEX ix_oplog_hlc ON oplog (hlc);

CREATE TABLE sync_peer (
    device_id     BLOB PRIMARY KEY,
    last_seen_hlc TEXT NOT NULL
) STRICT;
