//! Catalogue use cases: which translations exist and what each one contains.
//!
//! These feed the two screens that precede reading — the translation picker
//! and the table of contents. Same boundary discipline as `get_chapter`
//! (ADR-0001): the shell makes one coarse call per screen and renders what
//! comes back. Where to open when nothing is saved is not decided here: the
//! first book's first chapter *is* the first page of `TocView`, so the UI has
//! nothing to invent.

use crate::get_chapter::AppError;
use crate::view::{TocBookView, TocView, TranslationView};
use bible_domain::{Translation, TranslationId};
use bible_storage::CorpusDb;

/// Maps a domain translation onto its boundary DTO. The one place the
/// mapping is written; `get_chapter` uses it too, so the picker and the
/// reading screen cannot disagree about a translation's name or direction.
pub(crate) fn translation_view(t: &Translation) -> TranslationView {
    TranslationView {
        id: t.id.to_string(),
        abbrev: t.abbrev.clone(),
        name: t.name.clone(),
        language: t.language.to_string(),
        direction: t.direction.as_html().to_string(),
        attribution: t.license.attribution.clone(),
    }
}

/// Every translation in the corpus, ready for the picker.
pub fn list_translations(db: &CorpusDb) -> Result<Vec<TranslationView>, AppError> {
    Ok(db.translations()?.iter().map(translation_view).collect())
}

/// The full table of contents for one translation.
pub fn table_of_contents(db: &CorpusDb, translation_id: &str) -> Result<TocView, AppError> {
    let tid = TranslationId::new(translation_id)
        .map_err(|_| AppError::UnknownTranslation(translation_id.to_string()))?;
    let translation = db
        .translation(&tid)?
        .ok_or_else(|| AppError::UnknownTranslation(translation_id.to_string()))?;

    let mut books = Vec::new();
    for book in db.books(&tid)? {
        books.push(TocBookView {
            id: book.usfm_code().to_string(),
            // English only for now, honestly so — same reasoning as
            // `get_chapter`: localised names are translation data, not a
            // lookup table to invent here.
            name: book.english_name().to_string(),
            chapters: db.chapters(&tid, book)?,
        });
    }

    Ok(TocView {
        translation: translation_view(&translation),
        books,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bible_domain::{
        Block, BookId, Canon, ChapterDoc, Inline, LangTag, License, ParaStyle, SchemeId,
        SourceRef, TextDirection, TranslationId, VerseSpan,
    };

    fn web() -> Translation {
        Translation {
            id: TranslationId::new("eng-web").unwrap(),
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

    fn chapter(book: BookId, chapter: u16) -> ChapterDoc {
        ChapterDoc {
            translation: TranslationId::new("eng-web").unwrap(),
            book,
            chapter,
            notes: vec![],
            blocks: vec![Block::Paragraph {
                style: ParaStyle::Normal,
                inlines: vec![
                    Inline::VerseStart(VerseSpan::single(1)),
                    Inline::Text("text".into()),
                ],
            }],
        }
    }

    fn db() -> CorpusDb {
        let db = CorpusDb::open_in_memory().unwrap();
        db.put_scheme("eng", "English", "abc").unwrap();
        db.put_translation(&web(), "2026-07-25", "1").unwrap();
        // Inserted out of canonical order on purpose.
        db.put_chapter(&chapter(BookId::Psa, 23)).unwrap();
        db.put_chapter(&chapter(BookId::Gen, 2)).unwrap();
        db.put_chapter(&chapter(BookId::Gen, 1)).unwrap();
        db
    }

    #[test]
    fn the_picker_sees_every_translation_with_its_attribution() {
        let list = list_translations(&db()).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "eng-web");
        assert_eq!(list[0].abbrev, "WEB");
        assert_eq!(list[0].direction, "ltr");
        assert!(list[0].attribution.contains("public domain"));
    }

    #[test]
    fn an_empty_corpus_lists_no_translations_rather_than_erroring() {
        let db = CorpusDb::open_in_memory().unwrap();
        assert_eq!(list_translations(&db).unwrap().len(), 0);
    }

    #[test]
    fn the_first_page_of_the_contents_is_genesis_one() {
        let toc = table_of_contents(&db(), "eng-web").unwrap();
        assert_eq!(toc.books[0].id, "GEN");
        assert_eq!(toc.books[0].name, "Genesis");
        assert_eq!(toc.books[0].chapters, vec![1, 2]);
    }

    #[test]
    fn books_come_back_in_canonical_order_not_insertion_order() {
        let toc = table_of_contents(&db(), "eng-web").unwrap();
        let ids: Vec<&str> = toc.books.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(ids, vec!["GEN", "PSA"]);
    }

    #[test]
    fn the_contents_carry_the_translation_for_attribution() {
        let toc = table_of_contents(&db(), "eng-web").unwrap();
        assert_eq!(toc.translation.abbrev, "WEB");
        assert!(toc.translation.attribution.contains("public domain"));
    }

    #[test]
    fn an_unknown_translation_is_reported_by_name() {
        let e = table_of_contents(&db(), "spa-rvr").unwrap_err();
        assert!(e.to_string().contains("spa-rvr"), "{e}");
    }

    #[test]
    fn a_malformed_translation_id_is_the_same_error_not_a_panic() {
        let e = table_of_contents(&db(), "no such id!").unwrap_err();
        assert!(matches!(e, AppError::UnknownTranslation(_)), "{e}");
    }
}
