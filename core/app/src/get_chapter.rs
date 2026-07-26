//! The `get_chapter` use case.
//!
//! One coarse call returns an entire chapter ready to render (ADR-0001). There
//! is deliberately no `get_verse`, no `get_verse_count` and no per-verse call:
//! a chatty boundary is a bug paid for on every platform, and the reading UI
//! displays whole chapters anyway.

use crate::view::*;
use bible_domain::{
    Annotation, AnnotationLayer, Block, BookId, ChapterDoc, ChapterOverlay, CharStyle,
    HeadingStyle, Inline, NoteKind, ParaStyle, TranslationId,
};
use bible_storage::{CorpusDb, StorageError};
use std::fmt;

#[derive(Debug)]
pub enum AppError {
    Storage(StorageError),
    UnknownTranslation(String),
    UnknownBook(String),
    ChapterNotFound {
        translation: String,
        book: String,
        chapter: u16,
    },
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Storage(e) => write!(f, "{e}"),
            AppError::UnknownTranslation(t) => write!(f, "no such translation: {t}"),
            AppError::UnknownBook(b) => write!(f, "no such book: {b}"),
            AppError::ChapterNotFound {
                translation,
                book,
                chapter,
            } => {
                write!(f, "{translation} has no {book} {chapter}")
            }
        }
    }
}

impl std::error::Error for AppError {}

impl From<StorageError> for AppError {
    fn from(e: StorageError) -> Self {
        AppError::Storage(e)
    }
}

/// Active annotation layers and their spans, already mapped into the display
/// scheme by the caller.
#[derive(Debug, Default)]
pub struct ActiveLayers {
    pub layers: Vec<AnnotationLayer>,
    pub annotations: Vec<Annotation>,
}

impl ActiveLayers {
    fn spoken(&self, id: &str) -> Option<&str> {
        self.layers
            .iter()
            .find(|l| l.id.as_str() == id)
            .map(|l| l.spoken_label.as_str())
    }
}

/// Fetches one chapter, fully prepared for rendering.
pub fn get_chapter(
    db: &CorpusDb,
    translation_id: &str,
    book_code: &str,
    chapter: u16,
    active: &ActiveLayers,
) -> Result<ChapterView, AppError> {
    let tid = TranslationId::new(translation_id)
        .map_err(|_| AppError::UnknownTranslation(translation_id.to_string()))?;
    let book = BookId::from_usfm_code(book_code)
        .ok_or_else(|| AppError::UnknownBook(book_code.to_string()))?;

    let translation = db
        .translation(&tid)?
        .ok_or_else(|| AppError::UnknownTranslation(translation_id.to_string()))?;

    let doc = db
        .chapter(&tid, book, chapter)?
        .ok_or_else(|| AppError::ChapterNotFound {
            translation: translation_id.to_string(),
            book: book_code.to_string(),
            chapter,
        })?;

    let verses = doc.verse_numbers();
    let overlay = ChapterOverlay::build(book, chapter, &verses, active.annotations.iter());

    let book_name = translation_book_name(book);
    let title = format!("{book_name} {chapter}");

    let blocks = doc
        .blocks
        .iter()
        .map(|b| block_view(b, &book_name, chapter, &overlay, active))
        .collect();

    let notes = doc
        .notes
        .iter()
        .map(|n| NoteView {
            id: n.id.0,
            kind: match n.kind {
                NoteKind::Footnote => "footnote".into(),
                NoteKind::CrossReference => "crossReference".into(),
            },
            caller: n.caller.to_string(),
            inlines: inline_views(&n.inlines, &book_name, chapter, &overlay, active),
            text: flatten(&n.inlines),
        })
        .collect();

    Ok(ChapterView {
        translation: crate::catalog::translation_view(&translation),
        book: BookView {
            id: book.usfm_code().to_string(),
            name: book_name,
        },
        chapter,
        title,
        blocks,
        notes,
        verses,
        nav: navigation(db, &tid, book, chapter)?,
    })
}

/// Display name for a book.
///
/// English only for now, and honestly so: localised book names are a
/// translation-data problem, not a formatting one, and inventing a lookup
/// table here would be a stub. The `\toc1`/`\h` values captured at import are
/// where localised names will come from.
fn translation_book_name(book: BookId) -> String {
    book.english_name().to_string()
}

fn navigation(
    db: &CorpusDb,
    tid: &TranslationId,
    book: BookId,
    chapter: u16,
) -> Result<NavView, AppError> {
    let chapters = db.chapters(tid, book)?;
    let books = db.books(tid)?;
    let name = translation_book_name(book);

    let idx = chapters.iter().position(|c| *c == chapter);
    let previous = match idx {
        Some(i) if i > 0 => Some(ChapterCursor {
            book: book.usfm_code().into(),
            chapter: chapters[i - 1],
            label: format!("{name} {}", chapters[i - 1]),
        }),
        _ => previous_book_last_chapter(db, tid, &books, book)?,
    };
    let next = match idx {
        Some(i) if i + 1 < chapters.len() => Some(ChapterCursor {
            book: book.usfm_code().into(),
            chapter: chapters[i + 1],
            label: format!("{name} {}", chapters[i + 1]),
        }),
        _ => next_book_first_chapter(db, tid, &books, book)?,
    };
    Ok(NavView { previous, next })
}

fn previous_book_last_chapter(
    db: &CorpusDb,
    tid: &TranslationId,
    books: &[BookId],
    book: BookId,
) -> Result<Option<ChapterCursor>, AppError> {
    let Some(i) = books.iter().position(|b| *b == book) else {
        return Ok(None);
    };
    if i == 0 {
        return Ok(None);
    }
    let prev = books[i - 1];
    let chapters = db.chapters(tid, prev)?;
    Ok(chapters.last().map(|c| ChapterCursor {
        book: prev.usfm_code().into(),
        chapter: *c,
        label: format!("{} {c}", translation_book_name(prev)),
    }))
}

fn next_book_first_chapter(
    db: &CorpusDb,
    tid: &TranslationId,
    books: &[BookId],
    book: BookId,
) -> Result<Option<ChapterCursor>, AppError> {
    let Some(i) = books.iter().position(|b| *b == book) else {
        return Ok(None);
    };
    let Some(next) = books.get(i + 1) else {
        return Ok(None);
    };
    let chapters = db.chapters(tid, *next)?;
    Ok(chapters.first().map(|c| ChapterCursor {
        book: next.usfm_code().into(),
        chapter: *c,
        label: format!("{} {c}", translation_book_name(*next)),
    }))
}

fn para_token(style: &ParaStyle) -> String {
    match style {
        ParaStyle::Normal => "p".into(),
        ParaStyle::Margin => "m".into(),
        ParaStyle::MarginIndented => "mi".into(),
        ParaStyle::NoBreak => "nb".into(),
        ParaStyle::Centered => "pc".into(),
        ParaStyle::Indented(n) => format!("pi{n}"),
        ParaStyle::ListItem(n) => format!("li{n}"),
        ParaStyle::Poetry(n) => format!("q{n}"),
        ParaStyle::Descriptive => "d".into(),
        ParaStyle::Speaker => "sp".into(),
        ParaStyle::IntroParagraph => "ip".into(),
        ParaStyle::IntroListItem => "ili".into(),
        ParaStyle::Unknown(_) => "p".into(),
    }
}

fn char_token(style: &CharStyle) -> (&'static str, Option<String>) {
    match style {
        CharStyle::WordsOfJesus => ("wj", None),
        CharStyle::Word { strong } => ("w", strong.map(|s| s.to_string())),
        CharStyle::WordHebrew { strong } => ("wh", strong.map(|s| s.to_string())),
        CharStyle::Keyword => ("k", None),
        CharStyle::BookTitle => ("bk", None),
        CharStyle::Selah => ("qs", None),
        CharStyle::TranslatorAdded => ("add", None),
        CharStyle::DivineName => ("nd", None),
        CharStyle::Italic => ("it", None),
        CharStyle::Bold => ("bd", None),
        CharStyle::SmallCaps => ("sc", None),
        CharStyle::CrossRefTarget => ("xt", None),
        CharStyle::CrossRefOrigin => ("xo", None),
        CharStyle::NoteOriginRef => ("fr", None),
        CharStyle::NoteText => ("ft", None),
        CharStyle::NoteQuote => ("fq", None),
        CharStyle::NoteQuoteAlt => ("fqa", None),
        CharStyle::NoteLabel => ("fl", None),
        CharStyle::Unknown(_) => ("unknown", None),
    }
}

fn block_view(
    b: &Block,
    book_name: &str,
    chapter: u16,
    overlay: &ChapterOverlay,
    active: &ActiveLayers,
) -> BlockView {
    match b {
        Block::Paragraph { style, inlines } => BlockView::Paragraph {
            style: para_token(style),
            inlines: inline_views(inlines, book_name, chapter, overlay, active),
        },
        Block::Heading { style, inlines } => {
            let (level, token) = match style {
                // The chapter title is the page's <h1>, so section headings
                // start at <h2> and never skip a level.
                HeadingStyle::MajorSection(n) => (2u8, format!("ms{n}")),
                HeadingStyle::Section(n) => (3, format!("s{n}")),
                HeadingStyle::IntroSection(n) => (3, format!("is{n}")),
            };
            BlockView::Heading {
                level,
                style: token,
                inlines: inline_views(inlines, book_name, chapter, overlay, active),
            }
        }
        Block::Blank => BlockView::Blank,
    }
}

fn inline_views(
    v: &[Inline],
    book_name: &str,
    chapter: u16,
    overlay: &ChapterOverlay,
    active: &ActiveLayers,
) -> Vec<InlineView> {
    v.iter()
        .map(|i| match i {
            Inline::Text(t) => InlineView::Text { text: t.clone() },
            Inline::VerseStart(span) => {
                let layers: Vec<String> = overlay
                    .layers_for(span.start)
                    .iter()
                    .map(|l| l.as_str().to_string())
                    .collect();
                let spoken_layers = layers
                    .iter()
                    .filter_map(|id| active.spoken(id).map(str::to_string))
                    .collect();
                let number = span.to_string();
                InlineView::VerseStart {
                    start: span.start,
                    end: span.end,
                    label: if span.is_bridge() {
                        format!("{book_name} {chapter} verses {}", number)
                    } else {
                        format!("{book_name} {chapter} verse {}", span.start)
                    },
                    number,
                    layers,
                    spoken_layers,
                }
            }
            Inline::VerseEnd(span) => InlineView::VerseEnd {
                start: span.start,
                end: span.end,
            },
            Inline::Char { style, inner } => {
                let (token, strong) = char_token(style);
                InlineView::Span {
                    style: token.to_string(),
                    strong,
                    inner: inline_views(inner, book_name, chapter, overlay, active),
                }
            }
            Inline::NoteRef(id) => InlineView::Note {
                id: id.0,
                caller: "+".into(),
            },
        })
        .collect()
}

fn flatten(v: &[Inline]) -> String {
    let mut out = String::new();
    fn walk(v: &[Inline], out: &mut String) {
        for i in v {
            match i {
                Inline::Text(t) => out.push_str(t),
                Inline::Char { inner, .. } => walk(inner, out),
                _ => {}
            }
        }
    }
    walk(v, &mut out);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Convenience for tests and tooling: the chapters a translation has for a book.
pub fn chapters_of(
    db: &CorpusDb,
    translation_id: &str,
    book_code: &str,
) -> Result<Vec<u16>, AppError> {
    let tid = TranslationId::new(translation_id)
        .map_err(|_| AppError::UnknownTranslation(translation_id.to_string()))?;
    let book = BookId::from_usfm_code(book_code)
        .ok_or_else(|| AppError::UnknownBook(book_code.to_string()))?;
    Ok(db.chapters(&tid, book)?)
}

/// Re-exported so callers can build a view directly from a document in tests
/// without a database round trip.
pub fn view_of(
    doc: &ChapterDoc,
    translation: TranslationView,
    overlay: &ChapterOverlay,
    active: &ActiveLayers,
) -> ChapterView {
    let book_name = translation_book_name(doc.book);
    ChapterView {
        translation,
        book: BookView {
            id: doc.book.usfm_code().into(),
            name: book_name.clone(),
        },
        chapter: doc.chapter,
        title: format!("{book_name} {}", doc.chapter),
        blocks: doc
            .blocks
            .iter()
            .map(|b| block_view(b, &book_name, doc.chapter, overlay, active))
            .collect(),
        notes: doc
            .notes
            .iter()
            .map(|n| NoteView {
                id: n.id.0,
                kind: match n.kind {
                    NoteKind::Footnote => "footnote".into(),
                    NoteKind::CrossReference => "crossReference".into(),
                },
                caller: n.caller.to_string(),
                inlines: inline_views(&n.inlines, &book_name, doc.chapter, overlay, active),
                text: flatten(&n.inlines),
            })
            .collect(),
        verses: doc.verse_numbers(),
        nav: NavView {
            previous: None,
            next: None,
        },
    }
}
