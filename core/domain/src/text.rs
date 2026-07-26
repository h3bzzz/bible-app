//! Structured chapter text.
//!
//! ADR-0003: the chapter is the storage unit. A [`ChapterDoc`] is a flat
//! sequence of [`Block`]s; verse boundaries are zero-width milestones inside
//! the inline stream, not containers. This is the only representation that
//! survives a round trip through real USFM, where a verse routinely begins in
//! one poetry line and ends in another (Psalm 23:4) or has a paragraph break
//! in its middle (John 1:21).
//!
//! The variants below are grounded in a full marker census of the vendored
//! World English Bible corpus, plus the markers the KJV/ASV/BSB imports will
//! certainly introduce (`\add`, `\nd`, `\it`, `\bd`, `\sc`). Anything else
//! becomes [`ParaStyle::Unknown`] or [`CharStyle::Unknown`] rather than being
//! silently discarded — see [`ChapterDoc::unknown_markers`].

use crate::book::BookId;
use crate::translation::TranslationId;
use core::fmt;

/// One stored chapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChapterDoc {
    pub translation: TranslationId,
    pub book: BookId,
    pub chapter: u16,
    /// The chapter's content, in document order.
    pub blocks: Vec<Block>,
    /// Notes hoisted out of the inline stream and referenced by [`NoteId`], so
    /// the renderer may present them as popovers, endnotes, or not at all
    /// without restructuring the document.
    pub notes: Vec<Note>,
}

/// A block-level element. Blocks never nest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Paragraph {
        style: ParaStyle,
        inlines: Vec<Inline>,
    },
    Heading {
        style: HeadingStyle,
        inlines: Vec<Inline>,
    },
    /// `\b` — an explicit blank line between poetry stanzas. Semantic, not
    /// decorative: it separates stanzas and must survive to the renderer.
    Blank,
}

/// Paragraph-level styles observed in the corpus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParaStyle {
    /// `\p`
    Normal,
    /// `\m` — flush-left, no first-line indent.
    Margin,
    /// `\mi` — indented, no first-line indent.
    MarginIndented,
    /// `\nb` — continues the previous paragraph across a chapter boundary.
    NoBreak,
    /// `\pc` — centred.
    Centered,
    /// `\pi1`–`\pi3`
    Indented(u8),
    /// `\li1`–`\li3`
    ListItem(u8),
    /// `\q1`–`\q3` — poetry lines. The level is an indent depth, rendered with
    /// `padding-inline-start` so it works in RTL and is not spoken.
    Poetry(u8),
    /// `\d` — a descriptive title, i.e. a Hebrew psalm superscription.
    Descriptive,
    /// `\sp` — a speaker label, as in Song of Solomon.
    Speaker,
    /// `\ip` — introduction paragraph.
    IntroParagraph,
    /// `\ili` — introduction list item.
    IntroListItem,
    /// A marker not yet modelled. Preserved rather than dropped.
    Unknown(String),
}

/// Heading levels. Real `<h2>`/`<h3>` elements downstream, per ADR-0002.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeadingStyle {
    /// `\ms1`–`\ms3` — major section, e.g. "BOOK 1" in the Psalter.
    MajorSection(u8),
    /// `\s1`–`\s3` — section heading.
    Section(u8),
    /// `\is1`–`\is2` — introduction section heading.
    IntroSection(u8),
}

/// The verse or verses a milestone covers.
///
/// Usually a single verse. USFM also permits a **bridge** — `\v 19-27` — where
/// one run of text covers several verse numbers because the translation does
/// not divide them. Six occur in the vendored corpus. A milestone carrying a
/// bare `u16` would silently lose every verse of a bridge but the first, so the
/// span is modelled rather than assumed away.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VerseSpan {
    pub start: u16,
    pub end: u16,
}

impl VerseSpan {
    pub const fn single(n: u16) -> VerseSpan {
        VerseSpan { start: n, end: n }
    }

    /// A bridge. `end` is clamped up to `start`, so an inverted span is
    /// impossible to construct.
    pub const fn bridge(start: u16, end: u16) -> VerseSpan {
        VerseSpan {
            start,
            end: if end < start { start } else { end },
        }
    }

    pub const fn is_bridge(&self) -> bool {
        self.end > self.start
    }

    pub const fn contains(&self, verse: u16) -> bool {
        self.start <= verse && verse <= self.end
    }

    pub fn iter(&self) -> impl Iterator<Item = u16> {
        self.start..=self.end
    }
}

impl fmt::Display for VerseSpan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_bridge() {
            write!(f, "{}-{}", self.start, self.end)
        } else {
            write!(f, "{}", self.start)
        }
    }
}

/// An inline element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inline {
    Text(String),
    /// Zero-width milestone: the span begins here.
    VerseStart(VerseSpan),
    /// Zero-width milestone: the span ends here.
    VerseEnd(VerseSpan),
    /// A character-styled span. Recursive, because USFM nests character
    /// markers with a `+` prefix — `\wj “\+w What\+w*…\wj*` is ordinary.
    Char {
        style: CharStyle,
        inner: Vec<Inline>,
    },
    /// Reference to a note hoisted into [`ChapterDoc::notes`].
    NoteRef(NoteId),
}

/// Character-level styles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CharStyle {
    /// `\wj` — words of Jesus. Rendered red by convention, but never by colour
    /// alone (ADR-0002 accessibility floor).
    WordsOfJesus,
    /// `\w` / `\+w` — a wordlist entry, carrying its Strong's number where the
    /// edition provides one. The WEB edition tags roughly 678,000 words.
    Word {
        strong: Option<StrongsRef>,
    },
    /// `\wh` — a Hebrew wordlist entry.
    WordHebrew {
        strong: Option<StrongsRef>,
    },
    /// `\k` — a keyword.
    Keyword,
    /// `\bk` — a quoted book title.
    BookTitle,
    /// `\qs` — "Selah".
    Selah,
    /// `\add` — words supplied by the translator. Italic by convention. Absent
    /// from WEB; universal in KJV and ASV.
    TranslatorAdded,
    /// `\nd` — the divine name. Small caps by convention.
    DivineName,
    /// `\it`, `\bd`, `\sc`
    Italic,
    Bold,
    SmallCaps,
    /// `\xt` — a cross-reference target.
    CrossRefTarget,
    /// `\xo` — a cross-reference origin.
    CrossRefOrigin,
    /// `\fr` — the origin reference of a footnote.
    NoteOriginRef,
    /// `\ft` — footnote body text.
    NoteText,
    /// `\fq` / `\fqa` — a quotation from the text within a footnote.
    NoteQuote,
    NoteQuoteAlt,
    /// `\fl` — a footnote label, e.g. "Heb.".
    NoteLabel,
    /// A marker not yet modelled. Preserved rather than dropped.
    Unknown(String),
}

/// Index into [`ChapterDoc::notes`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NoteId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub id: NoteId,
    pub kind: NoteKind,
    /// The caller glyph: `+` means auto-number, `-` means suppress, anything
    /// else is used literally.
    pub caller: char,
    pub inlines: Vec<Inline>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteKind {
    /// `\f`
    Footnote,
    /// `\x`
    CrossReference,
}

/// A Strong's concordance reference, e.g. `H3068` (YHWH) or `G2424` (Jesus).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StrongsRef {
    pub language: StrongsLang,
    pub number: u16,
    /// Homograph discriminator, e.g. the `a` in `H1254a`. Absent from WEB but
    /// present in several other tagged editions.
    pub suffix: Option<char>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StrongsLang {
    Hebrew,
    Greek,
}

/// Why a Strong's attribute was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrongsParseError(pub String);

impl fmt::Display for StrongsParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "malformed Strong's reference: {:?}", self.0)
    }
}

impl std::error::Error for StrongsParseError {}

impl StrongsRef {
    /// Parses `H3068`, `G2424`, `H1254a`.
    pub fn parse(s: &str) -> Result<StrongsRef, StrongsParseError> {
        let err = || StrongsParseError(s.to_string());
        let mut chars = s.chars();
        let language = match chars.next() {
            Some('H') => StrongsLang::Hebrew,
            Some('G') => StrongsLang::Greek,
            _ => return Err(err()),
        };
        let rest = chars.as_str();
        let digits_end = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        if digits_end == 0 {
            return Err(err());
        }
        let number: u16 = rest[..digits_end].parse().map_err(|_| err())?;
        let suffix = match &rest[digits_end..] {
            "" => None,
            tail => {
                let mut t = tail.chars();
                let c = t.next().ok_or_else(err)?;
                if t.next().is_some() || !c.is_ascii_lowercase() {
                    return Err(err());
                }
                Some(c)
            }
        };
        Ok(StrongsRef {
            language,
            number,
            suffix,
        })
    }
}

impl fmt::Display for StrongsRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let l = match self.language {
            StrongsLang::Hebrew => 'H',
            StrongsLang::Greek => 'G',
        };
        write!(f, "{l}{}", self.number)?;
        if let Some(c) = self.suffix {
            write!(f, "{c}")?;
        }
        Ok(())
    }
}

impl ChapterDoc {
    /// Every unknown marker in the document, deduplicated and sorted.
    ///
    /// The importer preserves unknown markers so no information is lost, and
    /// `build-corpus` refuses to build in strict mode when this is non-empty.
    /// That combination means a new marker is a loud build failure rather than
    /// quiet data loss — the exact failure mode ADR-0003 exists to prevent.
    pub fn unknown_markers(&self) -> Vec<String> {
        fn walk_inlines(inlines: &[Inline], out: &mut Vec<String>) {
            for i in inlines {
                if let Inline::Char { style, inner } = i {
                    if let CharStyle::Unknown(m) = style {
                        out.push(m.clone());
                    }
                    walk_inlines(inner, out);
                }
            }
        }
        let mut out = Vec::new();
        for b in &self.blocks {
            match b {
                Block::Paragraph { style, inlines } => {
                    if let ParaStyle::Unknown(m) = style {
                        out.push(m.clone());
                    }
                    walk_inlines(inlines, &mut out);
                }
                Block::Heading { inlines, .. } => walk_inlines(inlines, &mut out),
                Block::Blank => {}
            }
        }
        for n in &self.notes {
            walk_inlines(&n.inlines, &mut out);
        }
        out.sort();
        out.dedup();
        out
    }

    /// Every verse number present, in document order, deduplicated.
    ///
    /// A verse may open more than one block (Psalm 23:4 opens three), so this
    /// counts distinct verses rather than milestones.
    pub fn verse_numbers(&self) -> Vec<u16> {
        let mut out = Vec::new();
        for b in &self.blocks {
            let inlines = match b {
                Block::Paragraph { inlines, .. } | Block::Heading { inlines, .. } => inlines,
                Block::Blank => continue,
            };
            collect_verse_starts(inlines, &mut out);
        }
        out.dedup();
        out
    }

    /// Concatenated plain text of one verse, with all markup removed.
    ///
    /// This is the projection that feeds FTS5 and cross-reference previews. It
    /// is derived, never authoritative — ADR-0003.
    pub fn verse_plain_text(&self, verse: u16) -> Option<String> {
        let mut out = String::new();
        let mut inside = false;
        for b in &self.blocks {
            let inlines = match b {
                Block::Paragraph { inlines, .. } => inlines,
                // Headings and blanks are not part of any verse's text.
                Block::Heading { .. } | Block::Blank => continue,
            };
            // A block boundary is a word boundary. Psalm 23:4 spans three
            // poetry lines; without this the last word of one line and the
            // first of the next are concatenated into a single token, which
            // then fails to match in search.
            if !out.is_empty() {
                out.push(' ');
            }
            flatten_verse(inlines, verse, &mut inside, &mut out);
        }
        let trimmed = normalise_spaces(&out);
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }
}

fn collect_verse_starts(inlines: &[Inline], out: &mut Vec<u16>) {
    for i in inlines {
        match i {
            Inline::VerseStart(span) => out.extend(span.iter()),
            Inline::Char { inner, .. } => collect_verse_starts(inner, out),
            _ => {}
        }
    }
}

/// Walks the inline stream accumulating text while inside `verse`.
///
/// `inside` is threaded across blocks because a verse spans them.
fn flatten_verse(inlines: &[Inline], verse: u16, inside: &mut bool, out: &mut String) {
    for i in inlines {
        match i {
            // A bridged span covers every verse in it, so each of them
            // flattens to the same shared text.
            Inline::VerseStart(span) => *inside = span.contains(verse),
            Inline::VerseEnd(span) => {
                if span.contains(verse) {
                    *inside = false;
                }
            }
            Inline::Text(t) => {
                if *inside {
                    out.push_str(t);
                }
            }
            Inline::Char { inner, .. } => flatten_verse(inner, verse, inside, out),
            // Note bodies are not part of the verse's searchable text: a
            // footnote about the Greek must not match a search for the verse.
            Inline::NoteRef(_) => {}
        }
    }
}

/// Collapses runs of whitespace to single spaces and trims.
///
/// USFM is line-oriented and pads generously; the corpus has trailing
/// double-spaces on most lines.
fn normalise_spaces(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut pending_space = false;
    for c in s.chars() {
        if c.is_whitespace() {
            pending_space = !out.is_empty();
        } else {
            if pending_space {
                out.push(' ');
                pending_space = false;
            }
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::translation::TranslationId;

    fn tid() -> TranslationId {
        TranslationId::new("eng-web").unwrap()
    }

    #[test]
    fn strongs_parses_the_shapes_in_the_corpus() {
        assert_eq!(
            StrongsRef::parse("H3068").unwrap(),
            StrongsRef {
                language: StrongsLang::Hebrew,
                number: 3068,
                suffix: None
            }
        );
        assert_eq!(
            StrongsRef::parse("G2424").unwrap(),
            StrongsRef {
                language: StrongsLang::Greek,
                number: 2424,
                suffix: None
            }
        );
        assert_eq!(
            StrongsRef::parse("H1254a").unwrap(),
            StrongsRef {
                language: StrongsLang::Hebrew,
                number: 1254,
                suffix: Some('a')
            }
        );
    }

    #[test]
    fn strongs_round_trips_through_display() {
        for s in ["H3068", "G2424", "H1254a", "G1"] {
            assert_eq!(StrongsRef::parse(s).unwrap().to_string(), s);
        }
    }

    #[test]
    fn strongs_rejects_malformed() {
        for s in [
            "", "H", "X3068", "3068", "H30a68", "HAB", "G2424ab", "G2424A",
        ] {
            assert!(StrongsRef::parse(s).is_err(), "should reject {s:?}");
        }
    }

    /// The Psalm 23:4 shape: one verse opening three consecutive poetry lines.
    fn psalm23_verse4_shape() -> ChapterDoc {
        ChapterDoc {
            translation: tid(),
            book: BookId::Psa,
            chapter: 23,
            notes: vec![],
            blocks: vec![
                Block::Paragraph {
                    style: ParaStyle::Poetry(1),
                    inlines: vec![
                        Inline::VerseStart(VerseSpan::single(4)),
                        Inline::Text("Even though I walk through the valley,".into()),
                    ],
                },
                Block::Paragraph {
                    style: ParaStyle::Poetry(2),
                    inlines: vec![Inline::Text("I will fear no evil,".into())],
                },
                Block::Paragraph {
                    style: ParaStyle::Poetry(1),
                    inlines: vec![
                        Inline::Text("your rod and your staff,".into()),
                        Inline::VerseEnd(VerseSpan::single(4)),
                        Inline::VerseStart(VerseSpan::single(5)),
                        Inline::Text("You prepare a table.".into()),
                    ],
                },
            ],
        }
    }

    #[test]
    fn a_verse_spanning_three_blocks_flattens_to_one_string() {
        let doc = psalm23_verse4_shape();
        assert_eq!(
            doc.verse_plain_text(4).unwrap(),
            "Even though I walk through the valley, I will fear no evil, your rod and your staff,"
        );
        assert_eq!(doc.verse_plain_text(5).unwrap(), "You prepare a table.");
        assert_eq!(doc.verse_plain_text(99), None);
    }

    #[test]
    fn verse_numbers_are_deduplicated_across_blocks() {
        assert_eq!(psalm23_verse4_shape().verse_numbers(), vec![4, 5]);
    }

    #[test]
    fn headings_and_blanks_are_not_part_of_verse_text() {
        let doc = ChapterDoc {
            translation: tid(),
            book: BookId::Psa,
            chapter: 1,
            notes: vec![],
            blocks: vec![
                Block::Heading {
                    style: HeadingStyle::MajorSection(1),
                    inlines: vec![Inline::Text("BOOK 1".into())],
                },
                Block::Paragraph {
                    style: ParaStyle::Poetry(1),
                    inlines: vec![
                        Inline::VerseStart(VerseSpan::single(1)),
                        Inline::Text("Blessed is the man".into()),
                    ],
                },
                Block::Blank,
            ],
        };
        assert_eq!(doc.verse_plain_text(1).unwrap(), "Blessed is the man");
    }

    #[test]
    fn note_bodies_are_excluded_from_searchable_verse_text() {
        let doc = ChapterDoc {
            translation: tid(),
            book: BookId::Jhn,
            chapter: 1,
            notes: vec![Note {
                id: NoteId(0),
                kind: NoteKind::Footnote,
                caller: '+',
                inlines: vec![Inline::Text("kataleipo means to seize".into())],
            }],
            blocks: vec![Block::Paragraph {
                style: ParaStyle::Normal,
                inlines: vec![
                    Inline::VerseStart(VerseSpan::single(5)),
                    Inline::Text("The light shines".into()),
                    Inline::NoteRef(NoteId(0)),
                    Inline::Text(" in the darkness.".into()),
                ],
            }],
        };
        let t = doc.verse_plain_text(5).unwrap();
        assert_eq!(t, "The light shines in the darkness.");
        assert!(!t.contains("kataleipo"));
    }

    #[test]
    fn nested_char_styles_contribute_their_text() {
        let doc = ChapterDoc {
            translation: tid(),
            book: BookId::Jhn,
            chapter: 1,
            notes: vec![],
            blocks: vec![Block::Paragraph {
                style: ParaStyle::Normal,
                inlines: vec![
                    Inline::VerseStart(VerseSpan::single(38)),
                    Inline::Text("He said, ".into()),
                    Inline::Char {
                        style: CharStyle::WordsOfJesus,
                        inner: vec![Inline::Char {
                            style: CharStyle::Word {
                                strong: Some(StrongsRef::parse("G5101").unwrap()),
                            },
                            inner: vec![Inline::Text("What".into())],
                        }],
                    },
                ],
            }],
        };
        assert_eq!(doc.verse_plain_text(38).unwrap(), "He said, What");
    }

    #[test]
    fn unknown_markers_are_reported_not_dropped() {
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
        assert_eq!(doc.unknown_markers(), vec!["\\zfuture", "\\zinline"]);
    }

    #[test]
    fn whitespace_from_line_oriented_usfm_is_normalised() {
        assert_eq!(normalise_spaces("  a   b  \n c  "), "a b c");
        assert_eq!(normalise_spaces("   "), "");
        assert_eq!(normalise_spaces(""), "");
    }
}
