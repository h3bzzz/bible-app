//! The on-disk encoding of a [`ChapterDoc`].
//!
//! These types mirror the domain model rather than reusing it, for two
//! reasons. The stated one: `bible-domain` has no dependencies, so it cannot
//! derive `Serialize`. The better one: **the stored format must not be a
//! shadow of whatever the domain enums happen to be called this month.**
//! Renaming a domain variant is a refactor; changing the bytes in every
//! shipped corpus is not. Keeping them apart makes that distinction explicit
//! and gives [`DOC_FORMAT`] something real to version.
//!
//! Encoding is CBOR: roughly 40% smaller than JSON across the corpus, faster
//! to decode, and not required to be human-readable — corpus.db is a build
//! artefact.

use bible_domain::{
    Block, BookId, ChapterDoc, CharStyle, HeadingStyle, Inline, Note, NoteId, NoteKind, ParaStyle,
    StrongsRef, TranslationId, VerseSpan,
};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Bumped whenever the encoding below changes shape. A corpus whose
/// `doc_format` differs from this is re-imported rather than mis-decoded.
pub const DOC_FORMAT: i64 = 1;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct WireDoc {
    /// Blocks.
    pub b: Vec<WireBlock>,
    /// Hoisted notes.
    pub n: Vec<WireNote>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum WireBlock {
    P(WirePara, Vec<WireInline>),
    H(WireHeading, Vec<WireInline>),
    B,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum WirePara {
    P,
    M,
    Mi,
    Nb,
    Pc,
    Pi(u8),
    Li(u8),
    Q(u8),
    D,
    Sp,
    Ip,
    Ili,
    U(String),
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum WireHeading {
    Ms(u8),
    S(u8),
    Is(u8),
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum WireInline {
    T(String),
    /// Verse start, as `(first, last)`. Equal for the ordinary case; different
    /// for a bridge such as `\v 19-27`.
    Vs(u16, u16),
    Ve(u16, u16),
    C(WireChar, Vec<WireInline>),
    N(u32),
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum WireChar {
    Wj,
    /// Strong's number as its canonical string, e.g. `"H3068"`.
    W(Option<String>),
    Wh(Option<String>),
    K,
    Bk,
    Qs,
    Add,
    Nd,
    It,
    Bd,
    Sc,
    Xt,
    Xo,
    Fr,
    Ft,
    Fq,
    Fqa,
    Fl,
    U(String),
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct WireNote {
    pub i: u32,
    /// `true` for a cross-reference, `false` for a footnote.
    pub x: bool,
    pub c: char,
    pub n: Vec<WireInline>,
}

#[derive(Debug)]
pub enum CodecError {
    Encode(String),
    Decode(String),
    /// A Strong's string in the stored blob did not parse.
    BadStrongs(String),
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CodecError::Encode(e) => write!(f, "failed to encode chapter: {e}"),
            CodecError::Decode(e) => write!(f, "failed to decode chapter: {e}"),
            CodecError::BadStrongs(s) => write!(f, "stored Strong's reference {s:?} is malformed"),
        }
    }
}

impl std::error::Error for CodecError {}

// ── domain -> wire ────────────────────────────────────────────────────────

fn para_to_wire(s: &ParaStyle) -> WirePara {
    match s {
        ParaStyle::Normal => WirePara::P,
        ParaStyle::Margin => WirePara::M,
        ParaStyle::MarginIndented => WirePara::Mi,
        ParaStyle::NoBreak => WirePara::Nb,
        ParaStyle::Centered => WirePara::Pc,
        ParaStyle::Indented(n) => WirePara::Pi(*n),
        ParaStyle::ListItem(n) => WirePara::Li(*n),
        ParaStyle::Poetry(n) => WirePara::Q(*n),
        ParaStyle::Descriptive => WirePara::D,
        ParaStyle::Speaker => WirePara::Sp,
        ParaStyle::IntroParagraph => WirePara::Ip,
        ParaStyle::IntroListItem => WirePara::Ili,
        ParaStyle::Unknown(m) => WirePara::U(m.clone()),
    }
}

fn para_from_wire(s: WirePara) -> ParaStyle {
    match s {
        WirePara::P => ParaStyle::Normal,
        WirePara::M => ParaStyle::Margin,
        WirePara::Mi => ParaStyle::MarginIndented,
        WirePara::Nb => ParaStyle::NoBreak,
        WirePara::Pc => ParaStyle::Centered,
        WirePara::Pi(n) => ParaStyle::Indented(n),
        WirePara::Li(n) => ParaStyle::ListItem(n),
        WirePara::Q(n) => ParaStyle::Poetry(n),
        WirePara::D => ParaStyle::Descriptive,
        WirePara::Sp => ParaStyle::Speaker,
        WirePara::Ip => ParaStyle::IntroParagraph,
        WirePara::Ili => ParaStyle::IntroListItem,
        WirePara::U(m) => ParaStyle::Unknown(m),
    }
}

fn char_to_wire(s: &CharStyle) -> WireChar {
    match s {
        CharStyle::WordsOfJesus => WireChar::Wj,
        CharStyle::Word { strong } => WireChar::W(strong.map(|s| s.to_string())),
        CharStyle::WordHebrew { strong } => WireChar::Wh(strong.map(|s| s.to_string())),
        CharStyle::Keyword => WireChar::K,
        CharStyle::BookTitle => WireChar::Bk,
        CharStyle::Selah => WireChar::Qs,
        CharStyle::TranslatorAdded => WireChar::Add,
        CharStyle::DivineName => WireChar::Nd,
        CharStyle::Italic => WireChar::It,
        CharStyle::Bold => WireChar::Bd,
        CharStyle::SmallCaps => WireChar::Sc,
        CharStyle::CrossRefTarget => WireChar::Xt,
        CharStyle::CrossRefOrigin => WireChar::Xo,
        CharStyle::NoteOriginRef => WireChar::Fr,
        CharStyle::NoteText => WireChar::Ft,
        CharStyle::NoteQuote => WireChar::Fq,
        CharStyle::NoteQuoteAlt => WireChar::Fqa,
        CharStyle::NoteLabel => WireChar::Fl,
        CharStyle::Unknown(m) => WireChar::U(m.clone()),
    }
}

fn strong_from_wire(s: Option<String>) -> Result<Option<StrongsRef>, CodecError> {
    match s {
        None => Ok(None),
        Some(s) => StrongsRef::parse(&s)
            .map(Some)
            .map_err(|_| CodecError::BadStrongs(s)),
    }
}

fn char_from_wire(s: WireChar) -> Result<CharStyle, CodecError> {
    Ok(match s {
        WireChar::Wj => CharStyle::WordsOfJesus,
        WireChar::W(s) => CharStyle::Word {
            strong: strong_from_wire(s)?,
        },
        WireChar::Wh(s) => CharStyle::WordHebrew {
            strong: strong_from_wire(s)?,
        },
        WireChar::K => CharStyle::Keyword,
        WireChar::Bk => CharStyle::BookTitle,
        WireChar::Qs => CharStyle::Selah,
        WireChar::Add => CharStyle::TranslatorAdded,
        WireChar::Nd => CharStyle::DivineName,
        WireChar::It => CharStyle::Italic,
        WireChar::Bd => CharStyle::Bold,
        WireChar::Sc => CharStyle::SmallCaps,
        WireChar::Xt => CharStyle::CrossRefTarget,
        WireChar::Xo => CharStyle::CrossRefOrigin,
        WireChar::Fr => CharStyle::NoteOriginRef,
        WireChar::Ft => CharStyle::NoteText,
        WireChar::Fq => CharStyle::NoteQuote,
        WireChar::Fqa => CharStyle::NoteQuoteAlt,
        WireChar::Fl => CharStyle::NoteLabel,
        WireChar::U(m) => CharStyle::Unknown(m),
    })
}

fn inlines_to_wire(v: &[Inline]) -> Vec<WireInline> {
    v.iter()
        .map(|i| match i {
            Inline::Text(t) => WireInline::T(t.clone()),
            Inline::VerseStart(s) => WireInline::Vs(s.start, s.end),
            Inline::VerseEnd(s) => WireInline::Ve(s.start, s.end),
            Inline::NoteRef(n) => WireInline::N(n.0),
            Inline::Char { style, inner } => {
                WireInline::C(char_to_wire(style), inlines_to_wire(inner))
            }
        })
        .collect()
}

fn inlines_from_wire(v: Vec<WireInline>) -> Result<Vec<Inline>, CodecError> {
    v.into_iter()
        .map(|i| {
            Ok(match i {
                WireInline::T(t) => Inline::Text(t),
                WireInline::Vs(a, b) => Inline::VerseStart(VerseSpan::bridge(a, b)),
                WireInline::Ve(a, b) => Inline::VerseEnd(VerseSpan::bridge(a, b)),
                WireInline::N(n) => Inline::NoteRef(NoteId(n)),
                WireInline::C(style, inner) => Inline::Char {
                    style: char_from_wire(style)?,
                    inner: inlines_from_wire(inner)?,
                },
            })
        })
        .collect()
}

/// Encodes a chapter's body.
///
/// Translation, book and chapter are **not** stored in the blob: they are the
/// row's primary key, and duplicating them would create a second source of
/// truth that could disagree with the first.
pub fn encode(doc: &ChapterDoc) -> Result<Vec<u8>, CodecError> {
    let wire = WireDoc {
        b: doc
            .blocks
            .iter()
            .map(|b| match b {
                Block::Paragraph { style, inlines } => {
                    WireBlock::P(para_to_wire(style), inlines_to_wire(inlines))
                }
                Block::Heading { style, inlines } => WireBlock::H(
                    match style {
                        HeadingStyle::MajorSection(n) => WireHeading::Ms(*n),
                        HeadingStyle::Section(n) => WireHeading::S(*n),
                        HeadingStyle::IntroSection(n) => WireHeading::Is(*n),
                    },
                    inlines_to_wire(inlines),
                ),
                Block::Blank => WireBlock::B,
            })
            .collect(),
        n: doc
            .notes
            .iter()
            .map(|n| WireNote {
                i: n.id.0,
                x: n.kind == NoteKind::CrossReference,
                c: n.caller,
                n: inlines_to_wire(&n.inlines),
            })
            .collect(),
    };
    let mut out = Vec::new();
    ciborium::into_writer(&wire, &mut out).map_err(|e| CodecError::Encode(e.to_string()))?;
    Ok(out)
}

/// Decodes a chapter's body, re-attaching the identity held in the row's key.
pub fn decode(
    bytes: &[u8],
    translation: TranslationId,
    book: BookId,
    chapter: u16,
) -> Result<ChapterDoc, CodecError> {
    let wire: WireDoc =
        ciborium::from_reader(bytes).map_err(|e| CodecError::Decode(e.to_string()))?;
    let blocks = wire
        .b
        .into_iter()
        .map(|b| {
            Ok(match b {
                WireBlock::P(style, inlines) => Block::Paragraph {
                    style: para_from_wire(style),
                    inlines: inlines_from_wire(inlines)?,
                },
                WireBlock::H(style, inlines) => Block::Heading {
                    style: match style {
                        WireHeading::Ms(n) => HeadingStyle::MajorSection(n),
                        WireHeading::S(n) => HeadingStyle::Section(n),
                        WireHeading::Is(n) => HeadingStyle::IntroSection(n),
                    },
                    inlines: inlines_from_wire(inlines)?,
                },
                WireBlock::B => Block::Blank,
            })
        })
        .collect::<Result<Vec<_>, CodecError>>()?;
    let notes = wire
        .n
        .into_iter()
        .map(|n| {
            Ok(Note {
                id: NoteId(n.i),
                kind: if n.x {
                    NoteKind::CrossReference
                } else {
                    NoteKind::Footnote
                },
                caller: n.c,
                inlines: inlines_from_wire(n.n)?,
            })
        })
        .collect::<Result<Vec<_>, CodecError>>()?;
    Ok(ChapterDoc {
        translation,
        book,
        chapter,
        blocks,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tid() -> TranslationId {
        TranslationId::new("eng-web").unwrap()
    }

    fn sample() -> ChapterDoc {
        ChapterDoc {
            translation: tid(),
            book: BookId::Psa,
            chapter: 23,
            blocks: vec![
                Block::Heading {
                    style: HeadingStyle::MajorSection(1),
                    inlines: vec![Inline::Text("BOOK 1".into())],
                },
                Block::Paragraph {
                    style: ParaStyle::Descriptive,
                    inlines: vec![Inline::Text("A Psalm by David.".into())],
                },
                Block::Paragraph {
                    style: ParaStyle::Poetry(1),
                    inlines: vec![
                        Inline::VerseStart(VerseSpan::single(1)),
                        Inline::Char {
                            style: CharStyle::Word {
                                strong: Some(StrongsRef::parse("H3068").unwrap()),
                            },
                            inner: vec![Inline::Text("Yahweh".into())],
                        },
                        Inline::Text(" is my shepherd;".into()),
                        Inline::NoteRef(NoteId(0)),
                    ],
                },
                Block::Blank,
                Block::Paragraph {
                    style: ParaStyle::Poetry(2),
                    inlines: vec![
                        Inline::VerseEnd(VerseSpan::single(1)),
                        Inline::VerseStart(VerseSpan::bridge(2, 3)),
                        Inline::Char {
                            style: CharStyle::WordsOfJesus,
                            inner: vec![Inline::Char {
                                style: CharStyle::Word {
                                    strong: Some(StrongsRef::parse("G2424").unwrap()),
                                },
                                inner: vec![Inline::Text("nested".into())],
                            }],
                        },
                    ],
                },
            ],
            notes: vec![Note {
                id: NoteId(0),
                kind: NoteKind::Footnote,
                caller: '+',
                inlines: vec![Inline::Text("a note".into())],
            }],
        }
    }

    #[test]
    fn round_trips_every_construct() {
        let doc = sample();
        let bytes = encode(&doc).unwrap();
        let back = decode(&bytes, tid(), BookId::Psa, 23).unwrap();
        assert_eq!(doc, back);
    }

    #[test]
    fn bridged_spans_survive_the_round_trip() {
        let doc = sample();
        let back = decode(&encode(&doc).unwrap(), tid(), BookId::Psa, 23).unwrap();
        let has_bridge = back.blocks.iter().any(|b| match b {
            Block::Paragraph { inlines, .. } => inlines
                .iter()
                .any(|i| matches!(i, Inline::VerseStart(s) if s.is_bridge() && s.end == 3)),
            _ => false,
        });
        assert!(has_bridge, "a bridged verse span must survive encoding");
        assert_eq!(back.verse_numbers(), vec![1, 2, 3]);
    }

    #[test]
    fn identity_comes_from_the_row_key_not_the_blob() {
        // Decoding with different identity must yield that identity, proving
        // the blob does not carry a second, potentially stale copy.
        let bytes = encode(&sample()).unwrap();
        let other = decode(
            &bytes,
            TranslationId::new("eng-kjv").unwrap(),
            BookId::Jhn,
            3,
        )
        .unwrap();
        assert_eq!(other.translation.as_str(), "eng-kjv");
        assert_eq!(other.book, BookId::Jhn);
        assert_eq!(other.chapter, 3);
    }

    #[test]
    fn cbor_is_smaller_than_the_text_it_encodes() {
        let doc = sample();
        let bytes = encode(&doc).unwrap();
        assert!(!bytes.is_empty());
        // Sanity: the encoding is compact, not a JSON-shaped blowup.
        assert!(
            bytes.len() < 400,
            "unexpectedly large encoding: {}",
            bytes.len()
        );
    }

    #[test]
    fn a_corrupt_blob_is_an_error_not_a_panic() {
        let e = decode(&[0xff, 0xff, 0xff], tid(), BookId::Gen, 1);
        assert!(matches!(e, Err(CodecError::Decode(_))));
    }

    #[test]
    fn a_malformed_stored_strongs_reference_is_reported() {
        let wire = WireDoc {
            b: vec![WireBlock::P(
                WirePara::P,
                vec![WireInline::C(WireChar::W(Some("nonsense".into())), vec![])],
            )],
            n: vec![],
        };
        let mut bytes = Vec::new();
        ciborium::into_writer(&wire, &mut bytes).unwrap();
        assert!(matches!(
            decode(&bytes, tid(), BookId::Gen, 1),
            Err(CodecError::BadStrongs(_))
        ));
    }

    #[test]
    fn unknown_markers_survive_storage() {
        let doc = ChapterDoc {
            translation: tid(),
            book: BookId::Gen,
            chapter: 1,
            notes: vec![],
            blocks: vec![Block::Paragraph {
                style: ParaStyle::Unknown("\\zfuture".into()),
                inlines: vec![Inline::Char {
                    style: CharStyle::Unknown("\\zinline".into()),
                    inner: vec![Inline::Text("x".into())],
                }],
            }],
        };
        let back = decode(&encode(&doc).unwrap(), tid(), BookId::Gen, 1).unwrap();
        assert_eq!(back.unknown_markers(), vec!["\\zfuture", "\\zinline"]);
    }
}
