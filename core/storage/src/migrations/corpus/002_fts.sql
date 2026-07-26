-- corpus.db, migration 002: full-text search.
--
-- Separate from 001 so the migration chain has more than one link to test, and
-- because the indexes are optional weight: a corpus can be built and read
-- without them.
--
-- §4.4: do not write a search engine. FTS5 already provides tokenisation,
-- phrase queries, boolean operators, prefix wildcards and BM25 ranking.

-- Word index. `external content` means the text lives once in verse_plain and
-- is not duplicated into the index.
CREATE VIRTUAL TABLE verse_fts USING fts5(
    text,
    content = 'verse_plain',
    content_rowid = 'id',
    -- remove_diacritics 2 folds accents without mangling the combining marks
    -- that carry meaning in transliterated Hebrew and Greek.
    tokenize = "unicode61 remove_diacritics 2"
);

-- Substring and fuzzy index, deliberately a separate table. Trigram indexes
-- are several times the size of a word index, and a reader who never runs a
-- substring search should not carry one. build-corpus populates it only when
-- asked.
CREATE VIRTUAL TABLE verse_fts_tri USING fts5(
    text,
    content = 'verse_plain',
    content_rowid = 'id',
    tokenize = "trigram"
);

-- No synchronisation triggers by design: corpus.db is written only by
-- build-corpus, in a single transaction, which populates the indexes
-- explicitly. Triggers on a bulk import are a measurable slowdown for no
-- benefit, and there is no other writer to protect against.
