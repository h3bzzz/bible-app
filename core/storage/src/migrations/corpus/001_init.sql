-- corpus.db, migration 001.
--
-- Read-only at runtime, produced by tools/build-corpus, replaceable wholesale.
-- Kept separate from user.db (ADR-0005) so that updating a translation cannot
-- endanger a user's notes and restoring notes cannot corrupt the corpus.

-- ── Versification ────────────────────────────────────────────────────────
CREATE TABLE versification_scheme (
    id           TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    source_sha256 TEXT NOT NULL
) STRICT;

CREATE TABLE versification_last_verse (
    scheme_id  TEXT    NOT NULL REFERENCES versification_scheme(id) ON DELETE CASCADE,
    book_id    TEXT    NOT NULL,
    chapter    INTEGER NOT NULL,
    last_verse INTEGER NOT NULL,
    PRIMARY KEY (scheme_id, book_id, chapter)
) STRICT, WITHOUT ROWID;

-- ── Translations ─────────────────────────────────────────────────────────
CREATE TABLE translation (
    id               TEXT PRIMARY KEY,
    abbrev           TEXT NOT NULL,
    name             TEXT NOT NULL,
    language_tag     TEXT NOT NULL,
    text_direction   TEXT NOT NULL CHECK (text_direction IN ('ltr', 'rtl')),
    scheme_id        TEXT NOT NULL REFERENCES versification_scheme(id),
    canon            TEXT NOT NULL CHECK (canon IN ('protestant', 'catholic', 'eastern_orthodox')),
    license_spdx     TEXT,
    -- The §4.3 bundling gate, recorded in the artefact as well as enforced at
    -- build time, so an audit of a shipped database can answer the question.
    redistributable  INTEGER NOT NULL CHECK (redistributable IN (0, 1)),
    commercial_use   INTEGER NOT NULL CHECK (commercial_use IN (0, 1)),
    attribution      TEXT NOT NULL CHECK (length(trim(attribution)) > 0),
    source_url       TEXT NOT NULL,
    source_sha256    TEXT NOT NULL,
    imported_at      TEXT NOT NULL,
    -- Bumped when the importer's output shape changes, so a stale corpus is
    -- detectable rather than silently mis-rendered.
    importer_version TEXT NOT NULL
) STRICT;

-- ── Text ─────────────────────────────────────────────────────────────────
-- One row per chapter (ADR-0003). `doc` is a CBOR-encoded ChapterDoc.
CREATE TABLE chapter_doc (
    translation_id TEXT    NOT NULL REFERENCES translation(id) ON DELETE CASCADE,
    book_id        TEXT    NOT NULL,
    chapter        INTEGER NOT NULL,
    doc_format     INTEGER NOT NULL,
    doc            BLOB    NOT NULL,
    PRIMARY KEY (translation_id, book_id, chapter)
) STRICT, WITHOUT ROWID;

-- Ordering support for "next/previous chapter" without decoding any document.
CREATE TABLE chapter_index (
    translation_id TEXT    NOT NULL REFERENCES translation(id) ON DELETE CASCADE,
    book_id        TEXT    NOT NULL,
    chapter        INTEGER NOT NULL,
    book_order     INTEGER NOT NULL,
    verse_count    INTEGER NOT NULL,
    PRIMARY KEY (translation_id, book_id, chapter)
) STRICT, WITHOUT ROWID;

CREATE INDEX ix_chapter_index_order
    ON chapter_index (translation_id, book_order, chapter);

-- Flattened per-verse projection: the search index's content, and the source
-- for cross-reference previews. Derived, never authoritative.
CREATE TABLE verse_plain (
    id             INTEGER PRIMARY KEY,
    translation_id TEXT    NOT NULL REFERENCES translation(id) ON DELETE CASCADE,
    book_id        TEXT    NOT NULL,
    chapter        INTEGER NOT NULL,
    verse          INTEGER NOT NULL,
    text           TEXT    NOT NULL,
    UNIQUE (translation_id, book_id, chapter, verse)
) STRICT;

-- ── Annotation layers (ADR-0009) ─────────────────────────────────────────
-- Present from migration 001 even though the first dataset ships later:
-- retrofitting these tables is exactly what ADR-0003 and ADR-0004 exist to
-- prevent. Layers are separately licensed works, so they carry the same
-- provenance columns as translations.
CREATE TABLE annotation_layer (
    id                 TEXT PRIMARY KEY,
    kind               TEXT NOT NULL CHECK (kind IN ('messianic_prophecy', 'theme', 'cross_reference', 'commentary')),
    name               TEXT NOT NULL,
    spoken_label       TEXT NOT NULL CHECK (length(trim(spoken_label)) > 0),
    license_spdx       TEXT,
    redistributable    INTEGER NOT NULL CHECK (redistributable IN (0, 1)),
    commercial_use     INTEGER NOT NULL CHECK (commercial_use IN (0, 1)),
    attribution        TEXT NOT NULL CHECK (length(trim(attribution)) > 0),
    source_url         TEXT NOT NULL,
    source_sha256      TEXT NOT NULL,
    enabled_by_default INTEGER NOT NULL CHECK (enabled_by_default IN (0, 1))
) STRICT;

CREATE TABLE annotation (
    id            INTEGER PRIMARY KEY,
    layer_id      TEXT    NOT NULL REFERENCES annotation_layer(id) ON DELETE CASCADE,
    -- Always the org scheme (ADR-0004): a prophecy list keyed to English
    -- versification would highlight the wrong verse in a translation using
    -- Hebrew numbering, and Joel 2:28 versus Joel 3:1 is exactly that case.
    scheme_id     TEXT    NOT NULL DEFAULT 'org' CHECK (scheme_id = 'org'),
    start_book    TEXT    NOT NULL,
    start_chapter INTEGER NOT NULL,
    start_verse   INTEGER NOT NULL,
    end_book      TEXT    NOT NULL,
    end_chapter   INTEGER NOT NULL,
    end_verse     INTEGER NOT NULL,
    payload       BLOB    NOT NULL
) STRICT;

CREATE INDEX ix_annotation_span
    ON annotation (start_book, start_chapter, start_verse);
CREATE INDEX ix_annotation_layer ON annotation (layer_id);
