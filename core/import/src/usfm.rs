//! USFM 3 parser producing [`ChapterDoc`]s.
//!
//! Grounded in a full marker census of the vendored World English Bible: every
//! marker that occurs in the corpus is classified here. Anything unclassified
//! becomes an `Unknown` variant carrying the marker name, so it is preserved in
//! the document and reported by [`ChapterDoc::unknown_markers`] rather than
//! silently discarded. `build-corpus` refuses to ship a translation with any.

use crate::tokenizer::{tokenize, Marker, Token};
use bible_domain::{
    Block, BookId, ChapterDoc, CharStyle, HeadingStyle, Inline, Note, NoteId, NoteKind, ParaStyle,
    StrongsRef, TranslationId, VerseSpan,
};
use std::fmt;

/// Book-level metadata, which lives outside any chapter.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BookMeta {
    /// `\id` — the whole line, e.g. `GEN World English Bible (WEB)`.
    pub id_line: String,
    /// `\h` — running header.
    pub heading: Option<String>,
    /// `\toc1` — long table-of-contents name.
    pub toc_long: Option<String>,
    /// `\toc2` — short name.
    pub toc_short: Option<String>,
    /// `\toc3` — abbreviation.
    pub toc_abbrev: Option<String>,
    /// `\mt1`–`\mt3` — major title lines, in order.
    pub major_titles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedBook {
    pub book: BookId,
    pub meta: BookMeta,
    pub chapters: Vec<ChapterDoc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    /// No `\id` marker, so the book cannot be identified.
    MissingIdMarker,
    UnknownBookCode(String),
    MalformedChapterNumber(String),
    MalformedVerseNumber(String),
    /// A verse appeared before any `\c`.
    VerseOutsideChapter(String),
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportError::MissingIdMarker => write!(f, "no \\id marker; cannot identify the book"),
            ImportError::UnknownBookCode(c) => write!(f, "unknown book code {c:?} in \\id"),
            ImportError::MalformedChapterNumber(s) => write!(f, "malformed chapter number {s:?}"),
            ImportError::MalformedVerseNumber(s) => write!(f, "malformed verse number {s:?}"),
            ImportError::VerseOutsideChapter(s) => {
                write!(f, "verse {s:?} appears before any chapter marker")
            }
        }
    }
}

impl std::error::Error for ImportError {}

// ── Marker classification ─────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
enum Class {
    Para(ParaStyle),
    Heading(HeadingStyle),
    Blank,
    Chapter,
    Verse,
    Char(CharStyle),
    /// `\w` and `\wh` carry a `|strong="..."` attribute on their content.
    WordChar {
        hebrew: bool,
    },
    Note(NoteKind),
    Meta(MetaKind),
    /// Recognised but carrying no content we model: `\ide` (encoding),
    /// `\cl` (chapter label), `\cp` (published chapter character).
    Ignored,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MetaKind {
    Id,
    Heading,
    TocLong,
    TocShort,
    TocAbbrev,
    MajorTitle,
}

/// Splits a trailing level digit: `q1` -> `("q", Some(1))`, `p` -> `("p", None)`.
fn split_level(name: &str) -> (&str, Option<u8>) {
    let stem_len = name.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    if stem_len == name.len() || stem_len == 0 {
        return (name, None);
    }
    (&name[..stem_len], name[stem_len..].parse().ok())
}

fn classify(name: &str) -> Class {
    let (stem, level) = split_level(name);
    let lvl = level.unwrap_or(1);
    match (stem, level) {
        ("c", None) => Class::Chapter,
        ("v", None) => Class::Verse,
        ("b", None) => Class::Blank,

        // Paragraphs
        ("p", None) => Class::Para(ParaStyle::Normal),
        ("m", None) => Class::Para(ParaStyle::Margin),
        ("mi", None) => Class::Para(ParaStyle::MarginIndented),
        ("nb", None) => Class::Para(ParaStyle::NoBreak),
        ("pc", None) => Class::Para(ParaStyle::Centered),
        ("d", None) => Class::Para(ParaStyle::Descriptive),
        ("sp", None) => Class::Para(ParaStyle::Speaker),
        ("ip", None) => Class::Para(ParaStyle::IntroParagraph),
        ("ili", _) => Class::Para(ParaStyle::IntroListItem),
        ("q", Some(n)) => Class::Para(ParaStyle::Poetry(n)),
        ("pi", _) => Class::Para(ParaStyle::Indented(lvl)),
        ("li", _) => Class::Para(ParaStyle::ListItem(lvl)),

        // Headings
        ("ms", _) => Class::Heading(HeadingStyle::MajorSection(lvl)),
        ("s", Some(n)) => Class::Heading(HeadingStyle::Section(n)),
        ("is", _) => Class::Heading(HeadingStyle::IntroSection(lvl)),

        // Notes
        ("f", None) => Class::Note(NoteKind::Footnote),
        ("x", None) => Class::Note(NoteKind::CrossReference),

        // Character styles
        ("w", None) => Class::WordChar { hebrew: false },
        ("wh", None) => Class::WordChar { hebrew: true },
        ("wj", None) => Class::Char(CharStyle::WordsOfJesus),
        ("k", None) => Class::Char(CharStyle::Keyword),
        ("bk", None) => Class::Char(CharStyle::BookTitle),
        ("qs", None) => Class::Char(CharStyle::Selah),
        ("add", None) => Class::Char(CharStyle::TranslatorAdded),
        ("nd", None) => Class::Char(CharStyle::DivineName),
        ("it", None) => Class::Char(CharStyle::Italic),
        ("bd", None) => Class::Char(CharStyle::Bold),
        ("sc", None) => Class::Char(CharStyle::SmallCaps),
        ("xt", None) => Class::Char(CharStyle::CrossRefTarget),
        ("xo", None) => Class::Char(CharStyle::CrossRefOrigin),
        ("fr", None) => Class::Char(CharStyle::NoteOriginRef),
        ("ft", None) => Class::Char(CharStyle::NoteText),
        ("fq", None) => Class::Char(CharStyle::NoteQuote),
        ("fqa", None) => Class::Char(CharStyle::NoteQuoteAlt),
        ("fl", None) => Class::Char(CharStyle::NoteLabel),

        // Book metadata
        ("id", None) => Class::Meta(MetaKind::Id),
        ("h", None) => Class::Meta(MetaKind::Heading),
        ("toc", Some(1)) => Class::Meta(MetaKind::TocLong),
        ("toc", Some(2)) => Class::Meta(MetaKind::TocShort),
        ("toc", Some(3)) => Class::Meta(MetaKind::TocAbbrev),
        ("mt", _) => Class::Meta(MetaKind::MajorTitle),

        ("ide", None) | ("cl", None) | ("cp", None) => Class::Ignored,

        _ => Class::Unknown,
    }
}

// ── Parser ────────────────────────────────────────────────────────────────

#[derive(Debug)]
enum BlockStyle {
    Para(ParaStyle),
    Heading(HeadingStyle),
}

#[derive(Debug)]
enum Frame {
    Block {
        style: BlockStyle,
        inlines: Vec<Inline>,
    },
    Char {
        marker: String,
        style: CharStyle,
        inlines: Vec<Inline>,
    },
    Note {
        kind: NoteKind,
        caller: char,
        inlines: Vec<Inline>,
    },
}

impl Frame {
    fn inlines_mut(&mut self) -> &mut Vec<Inline> {
        match self {
            Frame::Block { inlines, .. }
            | Frame::Char { inlines, .. }
            | Frame::Note { inlines, .. } => inlines,
        }
    }
}

/// What the next text token should be consumed as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pending {
    None,
    Meta(MetaKind),
    Chapter,
    Verse,
    NoteCaller,
    /// Consume and discard the argument of a recognised but unmodelled marker.
    Discard,
}

struct Parser<'a> {
    translation: &'a TranslationId,
    book: Option<BookId>,
    meta: BookMeta,
    chapters: Vec<ChapterDoc>,
    chapter: Option<u16>,
    blocks: Vec<Block>,
    frames: Vec<Frame>,
    notes: Vec<Note>,
    open_verse: Option<VerseSpan>,
    pending: Pending,
}

/// Collapses whitespace runs to single spaces, preserving one leading and one
/// trailing space where present.
///
/// USFM is line-oriented and the vendored corpus pads most lines with trailing
/// double spaces; without this the stored text is full of incidental
/// whitespace, and golden files become noise.
/// A token that is entirely whitespace collapses to a single space rather than
/// to nothing: in `\w Yahweh\w* \w is\w*` the gap between the two spans is the
/// only thing separating two words, and dropping it welds them together.
fn normalise(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut pending = false;
    for c in s.chars() {
        if c.is_whitespace() {
            pending = true;
        } else {
            if pending {
                out.push(' ');
                pending = false;
            }
            out.push(c);
        }
    }
    if pending {
        out.push(' ');
    }
    out
}

/// Splits `Yahweh|strong="H3068"` into its content and its Strong's number.
fn split_word_attributes(mut inlines: Vec<Inline>) -> (Vec<Inline>, Option<StrongsRef>) {
    let mut strong = None;
    for i in inlines.iter_mut() {
        let Inline::Text(t) = i else { continue };
        let Some(bar) = t.find('|') else { continue };
        let attrs = t[bar + 1..].to_string();
        t.truncate(bar);
        // `strong="H3068"`, or the default-attribute shorthand `|H3068`.
        let value = match attrs.split_once('=') {
            Some((k, v)) if k.trim() == "strong" => v.trim().trim_matches('"').to_string(),
            None => attrs.trim().to_string(),
            _ => String::new(),
        };
        if !value.is_empty() {
            strong = StrongsRef::parse(&value).ok();
        }
    }
    (inlines, strong)
}

/// Trims whitespace at the block's outer edges only.
///
/// Milestones are skipped rather than treated as content, because a block
/// almost always opens with a `VerseStart` and the padding to remove sits
/// after it.
fn trim_edges(inlines: &mut Vec<Inline>) {
    for i in inlines.iter_mut() {
        match i {
            Inline::VerseStart(_) | Inline::VerseEnd(_) => continue,
            Inline::Text(t) => {
                *t = t.trim_start().to_string();
                // A block often opens as `\q1` on its own line followed by
                // `\v 1 `, giving two whitespace-only runs around the
                // milestone. Keep going until real content is reached.
                if t.is_empty() {
                    continue;
                }
                break;
            }
            _ => break,
        }
    }
    for i in inlines.iter_mut().rev() {
        match i {
            Inline::VerseStart(_) | Inline::VerseEnd(_) => continue,
            Inline::Text(t) => {
                *t = t.trim_end().to_string();
                if t.is_empty() {
                    continue;
                }
                break;
            }
            _ => break,
        }
    }
    inlines.retain(|i| !matches!(i, Inline::Text(t) if t.is_empty()));
}

impl<'a> Parser<'a> {
    fn new(translation: &'a TranslationId) -> Self {
        Parser {
            translation,
            book: None,
            meta: BookMeta::default(),
            chapters: Vec::new(),
            chapter: None,
            blocks: Vec::new(),
            frames: Vec::new(),
            notes: Vec::new(),
            open_verse: None,
            pending: Pending::None,
        }
    }

    fn push_inline(&mut self, i: Inline) {
        if self.frames.is_empty() {
            // Text before any paragraph marker. Real USFM does this after a
            // `\c` when the publisher relies on an implicit paragraph.
            self.frames.push(Frame::Block {
                style: BlockStyle::Para(ParaStyle::Normal),
                inlines: Vec::new(),
            });
        }
        self.frames
            .last_mut()
            .expect("just ensured")
            .inlines_mut()
            .push(i);
    }

    fn close_char_frames(&mut self) {
        while matches!(self.frames.last(), Some(Frame::Char { .. })) {
            self.pop_char_frame();
        }
    }

    fn pop_char_frame(&mut self) {
        let Some(Frame::Char {
            marker,
            style,
            inlines,
        }) = self.frames.pop()
        else {
            return;
        };
        let (inner, strong) = if matches!(marker.as_str(), "w" | "wh") {
            split_word_attributes(inlines)
        } else {
            (inlines, None)
        };
        let style = match (style, strong) {
            (CharStyle::Word { .. }, s) => CharStyle::Word { strong: s },
            (CharStyle::WordHebrew { .. }, s) => CharStyle::WordHebrew { strong: s },
            (other, _) => other,
        };
        self.push_inline(Inline::Char { style, inner });
    }

    fn finish_block(&mut self) {
        self.close_char_frames();
        // An unterminated note at a block boundary: close it so its content is
        // kept rather than dropped with the frame.
        if matches!(self.frames.last(), Some(Frame::Note { .. })) {
            self.close_note();
        }
        if let Some(Frame::Block { style, mut inlines }) = self.frames.pop() {
            trim_edges(&mut inlines);
            if inlines.is_empty() {
                return;
            }
            self.blocks.push(match style {
                BlockStyle::Para(style) => Block::Paragraph { style, inlines },
                BlockStyle::Heading(style) => Block::Heading { style, inlines },
            });
        }
    }

    fn open_block(&mut self, style: BlockStyle) {
        self.finish_block();
        self.frames.push(Frame::Block {
            style,
            inlines: Vec::new(),
        });
    }

    fn close_note(&mut self) {
        self.close_char_frames();
        let Some(Frame::Note {
            kind,
            caller,
            mut inlines,
        }) = self.frames.pop()
        else {
            return;
        };
        trim_edges(&mut inlines);
        let id = NoteId(self.notes.len() as u32);
        self.notes.push(Note {
            id,
            kind,
            caller,
            inlines,
        });
        self.push_inline(Inline::NoteRef(id));
    }

    fn finish_chapter(&mut self) {
        if let Some(span) = self.open_verse.take() {
            self.push_inline(Inline::VerseEnd(span));
        }
        self.finish_block();
        let (Some(book), Some(chapter)) = (self.book, self.chapter.take()) else {
            self.blocks.clear();
            self.notes.clear();
            return;
        };
        let blocks = std::mem::take(&mut self.blocks);
        let notes = std::mem::take(&mut self.notes);
        if blocks.is_empty() {
            return;
        }
        self.chapters.push(ChapterDoc {
            translation: self.translation.clone(),
            book,
            chapter,
            blocks,
            notes,
        });
    }

    fn set_meta(&mut self, kind: MetaKind, text: &str) -> Result<(), ImportError> {
        let value = normalise(text).trim().to_string();
        match kind {
            MetaKind::Id => {
                let code = value.split_whitespace().next().unwrap_or_default();
                self.book = Some(
                    BookId::from_usfm_code(code)
                        .ok_or_else(|| ImportError::UnknownBookCode(code.to_string()))?,
                );
                self.meta.id_line = value;
            }
            MetaKind::Heading => self.meta.heading = Some(value),
            MetaKind::TocLong => self.meta.toc_long = Some(value),
            MetaKind::TocShort => self.meta.toc_short = Some(value),
            MetaKind::TocAbbrev => self.meta.toc_abbrev = Some(value),
            MetaKind::MajorTitle => self.meta.major_titles.push(value),
        }
        Ok(())
    }

    /// Parses `19-27 Some text` or `4 Some text`, returning the span and the
    /// remaining text.
    fn parse_verse_arg(text: &str) -> Result<(VerseSpan, &str), ImportError> {
        let t = text.trim_start();
        let end = t
            .find(|c: char| !(c.is_ascii_digit() || c == '-'))
            .unwrap_or(t.len());
        let (num, rest) = t.split_at(end);
        let malformed = || ImportError::MalformedVerseNumber(text.trim().to_string());
        let span = match num.split_once('-') {
            Some((a, b)) => VerseSpan::bridge(
                a.parse().map_err(|_| malformed())?,
                b.parse().map_err(|_| malformed())?,
            ),
            None => VerseSpan::single(num.parse().map_err(|_| malformed())?),
        };
        Ok((span, rest))
    }

    fn handle_marker(&mut self, m: Marker<'_>) {
        // A closing marker never opens anything.
        if m.closing {
            match classify(m.name) {
                Class::Note(_) => self.close_note(),
                _ => {
                    // Pop up to and including the matching character frame.
                    let matching = self
                        .frames
                        .iter()
                        .rposition(|f| matches!(f, Frame::Char { marker, .. } if marker == m.name));
                    if let Some(idx) = matching {
                        while self.frames.len() > idx {
                            self.pop_char_frame();
                        }
                    }
                }
            }
            return;
        }

        match classify(m.name) {
            Class::Chapter => {
                self.finish_chapter();
                self.pending = Pending::Chapter;
            }
            Class::Verse => self.pending = Pending::Verse,
            Class::Blank => {
                self.finish_block();
                self.blocks.push(Block::Blank);
            }
            Class::Para(style) => self.open_block(BlockStyle::Para(style)),
            Class::Heading(style) => self.open_block(BlockStyle::Heading(style)),
            Class::Note(kind) => {
                self.frames.push(Frame::Note {
                    kind,
                    caller: '+',
                    inlines: Vec::new(),
                });
                self.pending = Pending::NoteCaller;
            }
            Class::Char(style) => self.frames.push(Frame::Char {
                marker: m.name.to_string(),
                style,
                inlines: Vec::new(),
            }),
            Class::WordChar { hebrew } => self.frames.push(Frame::Char {
                marker: m.name.to_string(),
                style: if hebrew {
                    CharStyle::WordHebrew { strong: None }
                } else {
                    CharStyle::Word { strong: None }
                },
                inlines: Vec::new(),
            }),
            Class::Meta(kind) => self.pending = Pending::Meta(kind),
            Class::Ignored => self.pending = Pending::Discard,
            Class::Unknown => {
                // Preserved, not dropped: an unmodelled marker becomes an
                // Unknown character span carrying its own name, which
                // ChapterDoc::unknown_markers reports and build-corpus refuses.
                self.frames.push(Frame::Char {
                    marker: m.name.to_string(),
                    style: CharStyle::Unknown(format!("\\{}", m.name)),
                    inlines: Vec::new(),
                });
            }
        }
    }

    fn handle_text(&mut self, text: &str) -> Result<(), ImportError> {
        match std::mem::replace(&mut self.pending, Pending::None) {
            Pending::Meta(kind) => return self.set_meta(kind, text),
            Pending::Discard => return Ok(()),
            Pending::Chapter => {
                let t = text.trim_start();
                let end = t.find(|c: char| !c.is_ascii_digit()).unwrap_or(t.len());
                let (num, rest) = t.split_at(end);
                self.chapter = Some(
                    num.parse()
                        .map_err(|_| ImportError::MalformedChapterNumber(text.trim().into()))?,
                );
                let rest = normalise(rest);
                if !rest.trim().is_empty() {
                    self.push_inline(Inline::Text(rest));
                }
                return Ok(());
            }
            Pending::Verse => {
                if self.chapter.is_none() {
                    return Err(ImportError::VerseOutsideChapter(text.trim().into()));
                }
                let (span, rest) = Self::parse_verse_arg(text)?;
                if let Some(prev) = self.open_verse.replace(span) {
                    self.push_inline(Inline::VerseEnd(prev));
                }
                self.push_inline(Inline::VerseStart(span));
                let rest = normalise(rest);
                if !rest.is_empty() {
                    self.push_inline(Inline::Text(rest));
                }
                return Ok(());
            }
            Pending::NoteCaller => {
                let t = text.trim_start();
                let caller = t.chars().next().unwrap_or('+');
                if let Some(Frame::Note { caller: c, .. }) = self.frames.last_mut() {
                    *c = caller;
                }
                let rest = normalise(&t[caller.len_utf8().min(t.len())..]);
                if !rest.trim().is_empty() {
                    self.push_inline(Inline::Text(rest));
                }
                return Ok(());
            }
            Pending::None => {}
        }

        let t = normalise(text);
        if !t.is_empty() {
            self.push_inline(Inline::Text(t));
        }
        Ok(())
    }
}

/// Parses one USFM book file.
pub fn parse_book(translation: &TranslationId, src: &str) -> Result<ImportedBook, ImportError> {
    let mut p = Parser::new(translation);
    for token in tokenize(src) {
        match token {
            Token::Marker(m) => p.handle_marker(m),
            Token::Text(t) => p.handle_text(t)?,
        }
    }
    p.finish_chapter();

    let book = p.book.ok_or(ImportError::MissingIdMarker)?;
    Ok(ImportedBook {
        book,
        meta: p.meta,
        chapters: p.chapters,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tid() -> TranslationId {
        TranslationId::new("eng-web").unwrap()
    }

    fn parse(src: &str) -> ImportedBook {
        parse_book(&tid(), src).expect("must parse")
    }

    #[test]
    fn splits_level_suffixes() {
        assert_eq!(split_level("q1"), ("q", Some(1)));
        assert_eq!(split_level("p"), ("p", None));
        assert_eq!(split_level("toc3"), ("toc", Some(3)));
        assert_eq!(split_level("ms1"), ("ms", Some(1)));
    }

    #[test]
    fn classifies_every_marker_present_in_the_corpus() {
        // The census of the vendored WEB corpus. None of these may fall
        // through to Unknown, because build-corpus refuses on Unknown.
        let census = [
            "w", "v", "q2", "q1", "p", "wj", "fr", "f", "ft", "c", "b", "fqa", "xt", "xo", "x",
            "id", "h", "toc3", "toc2", "toc1", "m", "ip", "bk", "sp", "fl", "is1", "q3", "mt3",
            "mt1", "d", "ili", "k", "li1", "ide", "pi1", "mt2", "qs", "wh", "mi", "nb", "fq", "s1",
            "ms1", "pc", "cp", "cl",
        ];
        for name in census {
            assert_ne!(
                classify(name),
                Class::Unknown,
                "marker \\{name} is in the corpus but unclassified"
            );
        }
    }

    #[test]
    fn genuinely_unknown_markers_are_reported() {
        assert_eq!(classify("zfuture"), Class::Unknown);
        let b = parse("\\id GEN Test\n\\c 1\n\\p \\v 1 \\zfuture text\\zfuture*\n");
        assert_eq!(b.chapters[0].unknown_markers(), vec!["\\zfuture"]);
    }

    #[test]
    fn parses_id_and_book_metadata() {
        let b = parse(
            "\\id PSA World English Bible (WEB)\n\\h Psalms\n\\toc1 The Psalms\n\
             \\toc2 Psalms\n\\toc3 Psa\n\\mt1 The Psalms\n\\c 1\n\\p \\v 1 Blessed.\n",
        );
        assert_eq!(b.book, BookId::Psa);
        assert_eq!(b.meta.id_line, "PSA World English Bible (WEB)");
        assert_eq!(b.meta.heading.as_deref(), Some("Psalms"));
        assert_eq!(b.meta.toc_long.as_deref(), Some("The Psalms"));
        assert_eq!(b.meta.toc_abbrev.as_deref(), Some("Psa"));
        assert_eq!(b.meta.major_titles, vec!["The Psalms"]);
    }

    #[test]
    fn missing_id_is_an_error() {
        assert_eq!(
            parse_book(&tid(), "\\c 1\n\\p \\v 1 x\n"),
            Err(ImportError::MissingIdMarker)
        );
    }

    #[test]
    fn unknown_book_code_is_an_error() {
        assert_eq!(
            parse_book(&tid(), "\\id ZZZ Nope\n"),
            Err(ImportError::UnknownBookCode("ZZZ".into()))
        );
    }

    #[test]
    fn a_verse_before_any_chapter_is_an_error() {
        assert!(matches!(
            parse_book(&tid(), "\\id GEN x\n\\p \\v 1 orphan\n"),
            Err(ImportError::VerseOutsideChapter(_))
        ));
    }

    #[test]
    fn splits_chapters() {
        let b = parse("\\id GEN x\n\\c 1\n\\p \\v 1 a\n\\c 2\n\\p \\v 1 b\n");
        assert_eq!(b.chapters.len(), 2);
        assert_eq!(b.chapters[0].chapter, 1);
        assert_eq!(b.chapters[1].chapter, 2);
        assert_eq!(b.chapters[0].verse_plain_text(1).unwrap(), "a");
        assert_eq!(b.chapters[1].verse_plain_text(1).unwrap(), "b");
    }

    #[test]
    fn trailing_whitespace_in_the_corpus_is_normalised_away() {
        // Every chapter marker in the vendored corpus is `\c 23  ` with
        // trailing spaces, and most text lines are padded too.
        let b = parse("\\id GEN x  \n\\c 1  \n\\p  \n\\v 1 In   the  beginning.  \n");
        assert_eq!(
            b.chapters[0].verse_plain_text(1).unwrap(),
            "In the beginning."
        );
    }

    #[test]
    fn extracts_strongs_numbers_from_tagged_words() {
        let b = parse(
            "\\id PSA x\n\\c 23\n\\q1\n\\v 1 \\w Yahweh|strong=\"H3068\"\\w* is my shepherd;\n",
        );
        let Block::Paragraph { inlines, .. } = &b.chapters[0].blocks[0] else {
            panic!("expected a paragraph");
        };
        let word = inlines
            .iter()
            .find_map(|i| match i {
                Inline::Char {
                    style: CharStyle::Word { strong },
                    inner,
                } => Some((strong, inner)),
                _ => None,
            })
            .expect("tagged word");
        assert_eq!(*word.0, Some(StrongsRef::parse("H3068").unwrap()));
        assert_eq!(word.1, &vec![Inline::Text("Yahweh".into())]);
        // The attribute must not leak into the searchable text.
        let plain = b.chapters[0].verse_plain_text(1).unwrap();
        assert_eq!(plain, "Yahweh is my shepherd;");
        assert!(!plain.contains("strong"));
    }

    #[test]
    fn parses_nested_character_markers() {
        let b = parse(
            "\\id JHN x\n\\c 1\n\\p \\v 38 He said, \\wj “\\+w What|strong=\"G5101\"\\+w*?”\\wj*\n",
        );
        let Block::Paragraph { inlines, .. } = &b.chapters[0].blocks[0] else {
            panic!("expected a paragraph");
        };
        let wj = inlines
            .iter()
            .find_map(|i| match i {
                Inline::Char {
                    style: CharStyle::WordsOfJesus,
                    inner,
                } => Some(inner),
                _ => None,
            })
            .expect("words of Jesus span");
        let nested = wj.iter().any(|i| {
            matches!(i, Inline::Char { style: CharStyle::Word { strong: Some(s) }, .. }
                     if *s == StrongsRef::parse("G5101").unwrap())
        });
        assert!(nested, "the \\+w span must nest inside \\wj: {wj:?}");
        assert_eq!(
            b.chapters[0].verse_plain_text(38).unwrap(),
            "He said, “What?”"
        );
    }

    #[test]
    fn hoists_footnotes_out_of_the_inline_stream() {
        let b = parse(
            "\\id JHN x\n\\c 1\n\\p \\v 5 The light shines\\f + \\fr 1:5 \\ft It refers to \
             getting a grip.\\f* in the darkness.\n",
        );
        let ch = &b.chapters[0];
        assert_eq!(ch.notes.len(), 1);
        assert_eq!(ch.notes[0].kind, NoteKind::Footnote);
        assert_eq!(ch.notes[0].caller, '+');
        // The note is referenced in place, not inlined.
        let Block::Paragraph { inlines, .. } = &ch.blocks[0] else {
            panic!()
        };
        assert!(inlines
            .iter()
            .any(|i| matches!(i, Inline::NoteRef(NoteId(0)))));
        // And its body stays out of the searchable text.
        let plain = ch.verse_plain_text(5).unwrap();
        assert_eq!(plain, "The light shines in the darkness.");
        assert!(!plain.contains("grip"));
    }

    #[test]
    fn preserves_a_verse_spanning_several_poetry_blocks() {
        // Psalm 23:4 in the real corpus: one verse opening three blocks.
        let b = parse(
            "\\id PSA x\n\\c 23\n\\q1\n\\v 4 Even though I walk,\n\
             \\q2 I will fear no evil,\n\\q1 your rod and your staff,\n\
             \\q2 they comfort me.\n\\q1\n\\v 5 You prepare a table.\n",
        );
        let ch = &b.chapters[0];
        assert_eq!(ch.blocks.len(), 5, "{:#?}", ch.blocks);
        assert_eq!(
            ch.verse_plain_text(4).unwrap(),
            "Even though I walk, I will fear no evil, your rod and your staff, they comfort me."
        );
        assert_eq!(ch.verse_plain_text(5).unwrap(), "You prepare a table.");
        assert_eq!(ch.verse_numbers(), vec![4, 5]);
    }

    #[test]
    fn preserves_a_paragraph_break_inside_a_verse() {
        // John 1:21 in the real corpus.
        let b = parse(
            "\\id JHN x\n\\c 1\n\\p \\v 21 They asked him, “Are you Elijah?”\n\
             \\p He said, “I am not.”\n",
        );
        let ch = &b.chapters[0];
        assert_eq!(ch.blocks.len(), 2, "the paragraph break must survive");
        assert_eq!(
            ch.verse_plain_text(21).unwrap(),
            "They asked him, “Are you Elijah?” He said, “I am not.”"
        );
    }

    #[test]
    fn keeps_the_hebrew_title_of_a_psalm_as_its_own_block() {
        let b = parse("\\id PSA x\n\\c 23\n\\d A Psalm by David.\n\\q1\n\\v 1 Yahweh.\n");
        let ch = &b.chapters[0];
        assert!(matches!(
            &ch.blocks[0],
            Block::Paragraph {
                style: ParaStyle::Descriptive,
                ..
            }
        ));
        // The title is not part of any verse.
        assert_eq!(ch.verse_plain_text(1).unwrap(), "Yahweh.");
    }

    #[test]
    fn section_headings_become_heading_blocks() {
        let b = parse("\\id PSA x\n\\c 1\n\\ms1 BOOK 1\n\\q1\n\\v 1 Blessed.\n");
        assert!(matches!(
            &b.chapters[0].blocks[0],
            Block::Heading {
                style: HeadingStyle::MajorSection(1),
                ..
            }
        ));
    }

    #[test]
    fn blank_markers_survive_as_stanza_separators() {
        let b = parse("\\id PSA x\n\\c 1\n\\q1\n\\v 1 a\n\\b\n\\q1\n\\v 2 b\n");
        assert!(b.chapters[0]
            .blocks
            .iter()
            .any(|x| matches!(x, Block::Blank)));
    }

    #[test]
    fn parses_bridged_verses() {
        // Six occur in the corpus, e.g. `\v 19-27`. Both numbers must resolve
        // to the shared text.
        let b = parse("\\id GEN x\n\\c 1\n\\p \\v 15-16 Shared text.\n\\v 17 Next.\n");
        let ch = &b.chapters[0];
        assert_eq!(ch.verse_numbers(), vec![15, 16, 17]);
        assert_eq!(ch.verse_plain_text(15).unwrap(), "Shared text.");
        assert_eq!(ch.verse_plain_text(16).unwrap(), "Shared text.");
        assert_eq!(ch.verse_plain_text(17).unwrap(), "Next.");
    }

    #[test]
    fn a_verse_whose_only_content_is_a_footnote_exists_but_has_no_text() {
        // Acts 8:37 in the real corpus. The WEB omits the verse from the main
        // text and prints only a note recording the variant. The verse must
        // stay navigable and referenceable, the note must be preserved, and
        // search must not return it as though it were scripture.
        let b = parse(
            "\\id ACT x\n\\c 8\n\\p \\v 36 They came to some water.\n\
             \\p\n\\v 37 \\f + \\fr 8:37 \\ft TR adds \\fqa Philip said, \
             “If you believe...”\\f*\n\\v 38 He commanded the chariot to stand still.\n",
        );
        let ch = &b.chapters[0];
        assert!(
            ch.verse_numbers().contains(&37),
            "the verse must still exist"
        );
        assert_eq!(
            ch.verse_plain_text(37),
            None,
            "but it has no scripture text"
        );
        assert_eq!(ch.notes.len(), 1, "the explanatory note must be preserved");
        assert!(ch.verse_plain_text(36).is_some());
        assert!(ch.verse_plain_text(38).is_some());
    }

    #[test]
    fn a_bridged_footnote_only_verse_covers_every_number_it_spans() {
        // Sirach 26:19-27 in the real corpus: a bridge whose only content is
        // the note "Verses 19-27 are omitted by the best authorities." All
        // nine numbers must exist; none may carry text.
        let b = parse(
            "\\id SIR x\n\\c 26\n\\p \\v 18 A sound heart.\n\
             \\v 19-27 \\f + \\fr 26:19-27 \\ft Verses 19-27 are omitted by the \
             best authorities. \\f*\n\\v 28 For two things.\n",
        );
        let ch = &b.chapters[0];
        let verses = ch.verse_numbers();
        for v in 19..=27 {
            assert!(verses.contains(&v), "verse {v} of the bridge must exist");
            assert_eq!(ch.verse_plain_text(v), None, "verse {v} must carry no text");
        }
        assert_eq!(ch.verse_plain_text(18).as_deref(), Some("A sound heart."));
        assert_eq!(ch.verse_plain_text(28).as_deref(), Some("For two things."));
    }

    #[test]
    fn malformed_verse_numbers_are_errors() {
        assert!(matches!(
            parse_book(&tid(), "\\id GEN x\n\\c 1\n\\p \\v 99999999 too big\n"),
            Err(ImportError::MalformedVerseNumber(_))
        ));
    }
}
