//! The corpus repository.
//!
//! This is the only module in the project permitted to contain SQL, and it
//! exposes domain types rather than rows. `bible-domain` cannot contain SQL
//! because it does not depend on `rusqlite`, so the rule is enforced by the
//! dependency graph rather than by review.

use crate::migrations::{migrate, Database, MigrationError};
use crate::wire::{self, CodecError, DOC_FORMAT};
use bible_domain::{
    BookId, Canon, ChapterDoc, LangTag, License, SourceRef, TextDirection, Translation,
    TranslationId,
};
use rusqlite::{Connection, OptionalExtension};
use std::fmt;
use std::path::Path;

#[derive(Debug)]
pub enum StorageError {
    Sqlite(rusqlite::Error),
    Migration(MigrationError),
    Codec(CodecError),
    /// A stored value no longer maps onto a domain type — an unknown book
    /// code, canon or direction. Means the database was written by a different
    /// build, and is reported rather than guessed around.
    Corrupt(String),
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StorageError::Sqlite(e) => write!(f, "sqlite error: {e}"),
            StorageError::Migration(e) => write!(f, "migration failed: {e}"),
            StorageError::Codec(e) => write!(f, "{e}"),
            StorageError::Corrupt(s) => write!(f, "corpus contains unreadable data: {s}"),
        }
    }
}

impl std::error::Error for StorageError {}

impl From<rusqlite::Error> for StorageError {
    fn from(e: rusqlite::Error) -> Self {
        StorageError::Sqlite(e)
    }
}
impl From<MigrationError> for StorageError {
    fn from(e: MigrationError) -> Self {
        StorageError::Migration(e)
    }
}
impl From<CodecError> for StorageError {
    fn from(e: CodecError) -> Self {
        StorageError::Codec(e)
    }
}

fn canon_to_sql(c: Canon) -> &'static str {
    match c {
        Canon::Protestant => "protestant",
        Canon::Catholic => "catholic",
        Canon::EasternOrthodox => "eastern_orthodox",
    }
}

fn canon_from_sql(s: &str) -> Result<Canon, StorageError> {
    Ok(match s {
        "protestant" => Canon::Protestant,
        "catholic" => Canon::Catholic,
        "eastern_orthodox" => Canon::EasternOrthodox,
        other => return Err(StorageError::Corrupt(format!("unknown canon {other:?}"))),
    })
}

fn direction_from_sql(s: &str) -> Result<TextDirection, StorageError> {
    Ok(match s {
        "ltr" => TextDirection::Ltr,
        "rtl" => TextDirection::Rtl,
        other => {
            return Err(StorageError::Corrupt(format!(
                "unknown direction {other:?}"
            )))
        }
    })
}

fn book_from_sql(s: &str) -> Result<BookId, StorageError> {
    BookId::from_usfm_code(s)
        .ok_or_else(|| StorageError::Corrupt(format!("unknown book code {s:?}")))
}

/// One row of `translation`, before validation.
#[derive(Debug)]
struct TranslationRow {
    id: String,
    abbrev: String,
    name: String,
    language_tag: String,
    text_direction: String,
    scheme_id: String,
    canon: String,
    license_spdx: Option<String>,
    redistributable: bool,
    commercial_use: bool,
    attribution: String,
    source_url: String,
    source_sha256: String,
}

impl TranslationRow {
    /// Every stored value that no longer maps onto a domain type is reported.
    /// None is silently replaced with a plausible substitute: a corpus written
    /// by an incompatible build should say so, not render the wrong text in
    /// the wrong direction.
    fn into_domain(self) -> Result<Translation, StorageError> {
        let id = TranslationId::new(self.id.clone())
            .map_err(|e| StorageError::Corrupt(format!("translation id {:?}: {e}", self.id)))?;
        let scheme = bible_domain::SchemeId::new(&self.scheme_id)
            .map_err(|e| StorageError::Corrupt(format!("scheme id {:?}: {e}", self.scheme_id)))?;
        Ok(Translation {
            id,
            abbrev: self.abbrev,
            name: self.name,
            language: LangTag::new(self.language_tag),
            direction: direction_from_sql(&self.text_direction)?,
            scheme,
            canon: canon_from_sql(&self.canon)?,
            license: License {
                spdx: self.license_spdx,
                redistributable: self.redistributable,
                commercial_use: self.commercial_use,
                attribution: self.attribution,
                source: SourceRef {
                    url: self.source_url,
                    sha256: self.source_sha256,
                    // Not stored: the manifest is the record of who published
                    // a translation, and duplicating it here would create a
                    // second copy free to drift from the first.
                    publisher: String::new(),
                },
            },
        })
    }
}

/// A corpus database. Read-only in the application; written by `build-corpus`.
#[derive(Debug)]
pub struct CorpusDb {
    conn: Connection,
}

impl CorpusDb {
    pub fn open(path: impl AsRef<Path>) -> Result<CorpusDb, StorageError> {
        let conn = Connection::open(path)?;
        Self::prepare(conn)
    }

    pub fn open_in_memory() -> Result<CorpusDb, StorageError> {
        Self::prepare(Connection::open_in_memory()?)
    }

    fn prepare(conn: Connection) -> Result<CorpusDb, StorageError> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", true)?;
        migrate(&conn, Database::Corpus)?;
        Ok(CorpusDb { conn })
    }

    /// Escape hatch for `build-corpus`, which needs to wrap a whole import in
    /// one transaction. Not exposed to the application.
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    // ── Versification schemes ────────────────────────────────────────────

    pub fn put_scheme(&self, id: &str, name: &str, sha256: &str) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO versification_scheme (id, name, source_sha256)
             VALUES (?1, ?2, ?3)",
            rusqlite::params![id, name, sha256],
        )?;
        Ok(())
    }

    // ── Translations ─────────────────────────────────────────────────────

    pub fn put_translation(
        &self,
        t: &Translation,
        imported_at: &str,
        importer_version: &str,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO translation
                (id, abbrev, name, language_tag, text_direction, scheme_id, canon,
                 license_spdx, redistributable, commercial_use, attribution,
                 source_url, source_sha256, imported_at, importer_version)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
            rusqlite::params![
                t.id.as_str(),
                t.abbrev,
                t.name,
                t.language.as_str(),
                t.direction.as_html(),
                t.scheme.as_str(),
                canon_to_sql(t.canon),
                t.license.spdx,
                t.license.redistributable as i64,
                t.license.commercial_use as i64,
                t.license.attribution,
                t.license.source.url,
                t.license.source.sha256,
                imported_at,
                importer_version,
            ],
        )?;
        Ok(())
    }

    /// The raw column values, before any domain validation.
    ///
    /// Kept separate from the conversion because `rusqlite`'s row mapper can
    /// only fail with a `rusqlite::Error`, and every interesting failure here
    /// is a domain failure. Squeezing the conversion into the mapper is what
    /// forces the "substitute a plausible value and carry on" bugs that this
    /// codebase reports instead.
    fn translation_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TranslationRow> {
        Ok(TranslationRow {
            id: row.get("id")?,
            abbrev: row.get("abbrev")?,
            name: row.get("name")?,
            language_tag: row.get("language_tag")?,
            text_direction: row.get("text_direction")?,
            scheme_id: row.get("scheme_id")?,
            canon: row.get("canon")?,
            license_spdx: row.get("license_spdx")?,
            redistributable: row.get::<_, i64>("redistributable")? != 0,
            commercial_use: row.get::<_, i64>("commercial_use")? != 0,
            attribution: row.get("attribution")?,
            source_url: row.get("source_url")?,
            source_sha256: row.get("source_sha256")?,
        })
    }

    pub fn translation(&self, id: &TranslationId) -> Result<Option<Translation>, StorageError> {
        let row = self
            .conn
            .query_row(
                "SELECT * FROM translation WHERE id = ?1",
                [id.as_str()],
                Self::translation_row,
            )
            .optional()?;
        row.map(TranslationRow::into_domain).transpose()
    }

    pub fn translations(&self) -> Result<Vec<Translation>, StorageError> {
        let mut stmt = self.conn.prepare("SELECT * FROM translation ORDER BY id")?;
        let rows = stmt
            .query_map([], Self::translation_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows.into_iter().map(TranslationRow::into_domain).collect()
    }

    // ── Chapters ─────────────────────────────────────────────────────────

    /// Stores a chapter, its ordering entry and its flattened verse
    /// projection. All three in one call, because a chapter present in one and
    /// absent from another is a corrupt corpus.
    pub fn put_chapter(&self, doc: &ChapterDoc) -> Result<(), StorageError> {
        let blob = wire::encode(doc)?;
        let tid = doc.translation.as_str();
        let book = doc.book.usfm_code();

        self.conn.execute(
            "INSERT OR REPLACE INTO chapter_doc
                (translation_id, book_id, chapter, doc_format, doc)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![tid, book, doc.chapter, DOC_FORMAT, blob],
        )?;

        let verses = doc.verse_numbers();
        self.conn.execute(
            "INSERT OR REPLACE INTO chapter_index
                (translation_id, book_id, chapter, book_order, verse_count)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                tid,
                book,
                doc.chapter,
                doc.book.order() as i64,
                verses.len() as i64
            ],
        )?;

        let mut stmt = self.conn.prepare_cached(
            "INSERT OR REPLACE INTO verse_plain
                (translation_id, book_id, chapter, verse, text)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for v in verses {
            if let Some(text) = doc.verse_plain_text(v) {
                stmt.execute(rusqlite::params![tid, book, doc.chapter, v, text])?;
            }
        }
        Ok(())
    }

    /// The one coarse read the application makes per screen of content
    /// (ADR-0001): a whole chapter in a single call.
    pub fn chapter(
        &self,
        translation: &TranslationId,
        book: BookId,
        chapter: u16,
    ) -> Result<Option<ChapterDoc>, StorageError> {
        let row: Option<(i64, Vec<u8>)> = self
            .conn
            .query_row(
                "SELECT doc_format, doc FROM chapter_doc
                 WHERE translation_id = ?1 AND book_id = ?2 AND chapter = ?3",
                rusqlite::params![translation.as_str(), book.usfm_code(), chapter],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;

        let Some((format, blob)) = row else {
            return Ok(None);
        };
        if format != DOC_FORMAT {
            return Err(StorageError::Corrupt(format!(
                "chapter stored in format {format}, this build reads {DOC_FORMAT}; re-import required"
            )));
        }
        Ok(Some(wire::decode(
            &blob,
            translation.clone(),
            book,
            chapter,
        )?))
    }

    /// Books present for a translation, in canonical order.
    pub fn books(&self, translation: &TranslationId) -> Result<Vec<BookId>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT book_id, book_order FROM chapter_index
             WHERE translation_id = ?1 ORDER BY book_order",
        )?;
        let codes = stmt
            .query_map([translation.as_str()], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        codes.iter().map(|c| book_from_sql(c)).collect()
    }

    /// Chapter numbers present for a book, ascending.
    pub fn chapters(
        &self,
        translation: &TranslationId,
        book: BookId,
    ) -> Result<Vec<u16>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT chapter FROM chapter_index
             WHERE translation_id = ?1 AND book_id = ?2 ORDER BY chapter",
        )?;
        let v = stmt
            .query_map(
                rusqlite::params![translation.as_str(), book.usfm_code()],
                |r| r.get::<_, u16>(0),
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    /// Rebuilds the word index from `verse_plain`.
    ///
    /// Called once by `build-corpus` after a bulk import rather than
    /// maintained by triggers — there is no second writer to protect against,
    /// and triggers on a bulk load are a measurable cost for no benefit.
    pub fn rebuild_search_index(&self, include_trigram: bool) -> Result<(), StorageError> {
        self.conn
            .execute_batch("INSERT INTO verse_fts(verse_fts) VALUES('rebuild');")?;
        if include_trigram {
            self.conn
                .execute_batch("INSERT INTO verse_fts_tri(verse_fts_tri) VALUES('rebuild');")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bible_domain::{Block, Inline, ParaStyle, SchemeId, VerseSpan};

    fn tid() -> TranslationId {
        TranslationId::new("eng-web").unwrap()
    }

    fn web() -> Translation {
        Translation {
            id: tid(),
            abbrev: "WEB".into(),
            name: "World English Bible".into(),
            language: LangTag::new("en"),
            direction: TextDirection::Ltr,
            scheme: SchemeId::ENG,
            canon: Canon::Protestant,
            license: License::public_domain(
                "World English Bible, public domain",
                SourceRef {
                    url: "https://ebible.org/Scriptures/eng-web_usfm.zip".into(),
                    sha256: "102fbf75".into(),
                    publisher: "eBible.org".into(),
                },
            ),
        }
    }

    fn db() -> CorpusDb {
        let db = CorpusDb::open_in_memory().unwrap();
        db.put_scheme("eng", "English", "abc").unwrap();
        db.put_translation(&web(), "2026-07-24", "1").unwrap();
        db
    }

    fn psalm23() -> ChapterDoc {
        ChapterDoc {
            translation: tid(),
            book: BookId::Psa,
            chapter: 23,
            notes: vec![],
            blocks: vec![
                Block::Paragraph {
                    style: ParaStyle::Descriptive,
                    inlines: vec![Inline::Text("A Psalm by David.".into())],
                },
                Block::Paragraph {
                    style: ParaStyle::Poetry(1),
                    inlines: vec![
                        Inline::VerseStart(VerseSpan::single(1)),
                        Inline::Text("Yahweh is my shepherd;".into()),
                    ],
                },
                Block::Paragraph {
                    style: ParaStyle::Poetry(2),
                    inlines: vec![Inline::Text("I shall lack nothing.".into())],
                },
            ],
        }
    }

    #[test]
    fn a_stored_chapter_round_trips_through_a_real_database() {
        let db = db();
        let doc = psalm23();
        db.put_chapter(&doc).unwrap();
        let back = db.chapter(&tid(), BookId::Psa, 23).unwrap().unwrap();
        assert_eq!(back, doc);
    }

    #[test]
    fn a_missing_chapter_is_none_not_an_error() {
        let db = db();
        assert!(db.chapter(&tid(), BookId::Psa, 151).unwrap().is_none());
    }

    #[test]
    fn translations_round_trip_with_their_licence_terms() {
        let db = db();
        let t = db.translation(&tid()).unwrap().unwrap();
        assert_eq!(t.abbrev, "WEB");
        assert_eq!(t.scheme, SchemeId::ENG);
        assert_eq!(t.canon, Canon::Protestant);
        assert_eq!(t.direction, TextDirection::Ltr);
        assert!(t.license.redistributable);
        assert!(t.license.attribution.contains("public domain"));
        assert_eq!(db.translations().unwrap().len(), 1);
    }

    #[test]
    fn a_right_to_left_translation_keeps_its_direction() {
        let db = db();
        db.put_scheme("org", "Original", "x").unwrap();
        let mut hebrew = web();
        hebrew.id = TranslationId::new("hbo-wlc").unwrap();
        hebrew.language = LangTag::new("hbo");
        hebrew.direction = TextDirection::Rtl;
        hebrew.scheme = SchemeId::ORG;
        db.put_translation(&hebrew, "2026-07-24", "1").unwrap();

        let back = db.translation(&hebrew.id).unwrap().unwrap();
        assert_eq!(back.direction, TextDirection::Rtl);
        assert_eq!(back.direction.as_html(), "rtl");
    }

    #[test]
    fn the_verse_projection_is_written_alongside_the_document() {
        let db = db();
        db.put_chapter(&psalm23()).unwrap();
        let text: String = db
            .connection()
            .query_row(
                "SELECT text FROM verse_plain WHERE book_id='PSA' AND chapter=23 AND verse=1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(text, "Yahweh is my shepherd; I shall lack nothing.");
    }

    #[test]
    fn the_search_index_finds_a_stored_verse() {
        let db = db();
        db.put_chapter(&psalm23()).unwrap();
        db.rebuild_search_index(true).unwrap();

        let n: i64 = db
            .connection()
            .query_row(
                "SELECT count(*) FROM verse_fts WHERE verse_fts MATCH 'shepherd'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);

        // Phrase query, which is why FTS5 is used rather than LIKE.
        let n: i64 = db
            .connection()
            .query_row(
                "SELECT count(*) FROM verse_fts WHERE verse_fts MATCH '\"is my shepherd\"'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);

        // Substring search via the trigram index.
        let n: i64 = db
            .connection()
            .query_row(
                "SELECT count(*) FROM verse_fts_tri WHERE verse_fts_tri MATCH 'hepher'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn book_and_chapter_listings_are_in_canonical_order() {
        let db = db();
        db.put_chapter(&psalm23()).unwrap();
        let mut john = psalm23();
        john.book = BookId::Jhn;
        john.chapter = 1;
        john.translation = tid();
        db.put_chapter(&john).unwrap();
        let mut psalm24 = psalm23();
        psalm24.chapter = 24;
        db.put_chapter(&psalm24).unwrap();

        assert_eq!(db.books(&tid()).unwrap(), vec![BookId::Psa, BookId::Jhn]);
        assert_eq!(db.chapters(&tid(), BookId::Psa).unwrap(), vec![23, 24]);
    }

    #[test]
    fn a_chapter_stored_in_an_older_format_is_refused_rather_than_misread() {
        let db = db();
        db.put_chapter(&psalm23()).unwrap();
        db.connection()
            .execute("UPDATE chapter_doc SET doc_format = 999", [])
            .unwrap();
        let e = db.chapter(&tid(), BookId::Psa, 23);
        assert!(matches!(e, Err(StorageError::Corrupt(_))), "{e:?}");
    }

    #[test]
    fn the_schema_itself_rejects_an_unknown_canon() {
        let db = db();
        // First line of defence: the CHECK constraint.
        assert!(db
            .connection()
            .execute("UPDATE translation SET canon = 'ethiopian'", [])
            .is_err());
    }

    #[test]
    fn a_corpus_with_an_unreadable_enum_value_is_reported_not_guessed_at() {
        let db = db();
        // Second line of defence. Simulates a database written by a build
        // whose CHECK constraints differed — which is exactly the case where
        // silently substituting a plausible value would render the wrong text.
        db.connection()
            .pragma_update(None, "ignore_check_constraints", true)
            .unwrap();
        db.connection()
            .execute("UPDATE translation SET canon = 'ethiopian'", [])
            .unwrap();

        let e = db.translation(&tid());
        assert!(matches!(e, Err(StorageError::Corrupt(_))), "{e:?}");
        assert!(e.unwrap_err().to_string().contains("ethiopian"));
    }

    #[test]
    fn foreign_keys_prevent_orphaned_chapters() {
        let db = CorpusDb::open_in_memory().unwrap();
        // No translation row exists, so a chapter referencing one must fail.
        assert!(db.put_chapter(&psalm23()).is_err());
    }

    #[test]
    fn reopening_a_file_database_preserves_its_contents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("corpus.db");
        {
            let db = CorpusDb::open(&path).unwrap();
            db.put_scheme("eng", "English", "abc").unwrap();
            db.put_translation(&web(), "2026-07-24", "1").unwrap();
            db.put_chapter(&psalm23()).unwrap();
        }
        let db = CorpusDb::open(&path).unwrap();
        assert!(db.chapter(&tid(), BookId::Psa, 23).unwrap().is_some());
    }
}
