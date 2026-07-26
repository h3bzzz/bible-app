//! Golden-file tests for the USFM importer.
//!
//! The fixtures are real, unedited chapters extracted from the vendored World
//! English Bible, given only an `\id` header so each is a complete book file.
//! Psalm 23 exercises Hebrew titles, poetry levels and a verse spanning three
//! blocks; John 1 exercises words of Jesus, nested character markers,
//! footnotes and a paragraph break inside a verse.
//!
//! Regenerate with `UPDATE_GOLDEN=1 cargo test -p bible-import --test golden`,
//! then **read the diff** — the point of a golden file is that a human agreed
//! to it once.

use bible_domain::{Block, ChapterDoc, CharStyle, HeadingStyle, Inline, ParaStyle, TranslationId};
use bible_import::parse_book;
use std::path::{Path, PathBuf};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

/// Renders a document to a stable, readable form. Deliberately lossless about
/// structure: a change in blocks, styles, milestones, Strong's numbers or note
/// hoisting must show up as a diff.
fn render(doc: &ChapterDoc) -> String {
    fn para_style(s: &ParaStyle) -> String {
        match s {
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
            ParaStyle::Unknown(m) => format!("UNKNOWN({m})"),
        }
    }

    fn char_style(s: &CharStyle) -> String {
        match s {
            CharStyle::WordsOfJesus => "wj".into(),
            CharStyle::Word { strong: Some(s) } => format!("w:{s}"),
            CharStyle::Word { strong: None } => "w".into(),
            CharStyle::WordHebrew { strong: Some(s) } => format!("wh:{s}"),
            CharStyle::WordHebrew { strong: None } => "wh".into(),
            CharStyle::Keyword => "k".into(),
            CharStyle::BookTitle => "bk".into(),
            CharStyle::Selah => "qs".into(),
            CharStyle::TranslatorAdded => "add".into(),
            CharStyle::DivineName => "nd".into(),
            CharStyle::Italic => "it".into(),
            CharStyle::Bold => "bd".into(),
            CharStyle::SmallCaps => "sc".into(),
            CharStyle::CrossRefTarget => "xt".into(),
            CharStyle::CrossRefOrigin => "xo".into(),
            CharStyle::NoteOriginRef => "fr".into(),
            CharStyle::NoteText => "ft".into(),
            CharStyle::NoteQuote => "fq".into(),
            CharStyle::NoteQuoteAlt => "fqa".into(),
            CharStyle::NoteLabel => "fl".into(),
            CharStyle::Unknown(m) => format!("UNKNOWN({m})"),
        }
    }

    fn inlines(v: &[Inline], out: &mut String, depth: usize) {
        for i in v {
            let pad = "  ".repeat(depth);
            match i {
                Inline::Text(t) => out.push_str(&format!("{pad}text {t:?}\n")),
                Inline::VerseStart(s) => out.push_str(&format!("{pad}verse-start {s}\n")),
                Inline::VerseEnd(s) => out.push_str(&format!("{pad}verse-end {s}\n")),
                Inline::NoteRef(n) => out.push_str(&format!("{pad}note-ref {}\n", n.0)),
                Inline::Char { style, inner } => {
                    out.push_str(&format!("{pad}char {}\n", char_style(style)));
                    inlines(inner, out, depth + 1);
                }
            }
        }
    }

    let mut out = String::new();
    out.push_str(&format!(
        "{} {} {}\n",
        doc.translation, doc.book, doc.chapter
    ));
    out.push_str(&format!("verses: {:?}\n", doc.verse_numbers()));
    out.push_str("---- blocks ----\n");
    for b in &doc.blocks {
        match b {
            Block::Paragraph { style, inlines: v } => {
                out.push_str(&format!("para {}\n", para_style(style)));
                inlines(v, &mut out, 1);
            }
            Block::Heading { style, inlines: v } => {
                let s = match style {
                    HeadingStyle::MajorSection(n) => format!("ms{n}"),
                    HeadingStyle::Section(n) => format!("s{n}"),
                    HeadingStyle::IntroSection(n) => format!("is{n}"),
                };
                out.push_str(&format!("heading {s}\n"));
                inlines(v, &mut out, 1);
            }
            Block::Blank => out.push_str("blank\n"),
        }
    }
    out.push_str("---- notes ----\n");
    for n in &doc.notes {
        out.push_str(&format!(
            "note {} {:?} caller {:?}\n",
            n.id.0, n.kind, n.caller
        ));
        inlines(&n.inlines, &mut out, 1);
    }
    out.push_str("---- plain text ----\n");
    for v in doc.verse_numbers() {
        if let Some(t) = doc.verse_plain_text(v) {
            out.push_str(&format!("{v}: {t}\n"));
        }
    }
    out
}

#[track_caller]
fn assert_golden(name: &str, actual: &str) {
    let path = fixtures().join("golden").join(format!("{name}.txt"));
    if std::env::var("UPDATE_GOLDEN").is_ok() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "missing golden {}: {e}; run with UPDATE_GOLDEN=1",
            path.display()
        )
    });
    assert_eq!(actual, expected, "golden mismatch for {name}");
}

fn import(file: &str) -> bible_import::ImportedBook {
    let tid = TranslationId::new("eng-web").unwrap();
    let src = std::fs::read_to_string(fixtures().join("usfm").join(file)).expect("fixture");
    parse_book(&tid, &src).expect("fixture must parse")
}

#[test]
fn psalm_23_matches_its_golden() {
    let b = import("psa023-eng-web.usfm");
    assert_eq!(b.chapters.len(), 1);
    assert_golden("psa023-eng-web", &render(&b.chapters[0]));
}

#[test]
fn john_1_matches_its_golden() {
    let b = import("jhn001-eng-web.usfm");
    assert_eq!(b.chapters.len(), 1);
    assert_golden("jhn001-eng-web", &render(&b.chapters[0]));
}

// ── Structural assertions independent of the golden files ────────────────
//
// A golden file proves nothing changed. These prove the right thing happens,
// and would fail loudly if someone regenerated a golden over a regression.

#[test]
fn psalm_23_keeps_its_hebrew_title_out_of_verse_one() {
    let b = import("psa023-eng-web.usfm");
    let ch = &b.chapters[0];
    assert!(
        matches!(
            &ch.blocks[0],
            Block::Paragraph {
                style: ParaStyle::Descriptive,
                ..
            }
        ),
        "first block must be the \\d superscription"
    );
    let v1 = ch.verse_plain_text(1).unwrap();
    assert!(v1.starts_with("Yahweh is my shepherd"), "{v1}");
    assert!(
        !v1.contains("A Psalm by David"),
        "the title is not part of verse 1"
    );
}

#[test]
fn psalm_23_verse_4_spans_three_poetry_blocks() {
    // The case ADR-0003 is built around. In the real corpus verse 4 opens a
    // \q1, continues through a \q2, then a further \q1 and \q2.
    let b = import("psa023-eng-web.usfm");
    let ch = &b.chapters[0];

    let mut blocks_touching_v4 = 0;
    let mut inside = false;
    for block in &ch.blocks {
        let Block::Paragraph { inlines, .. } = block else {
            continue;
        };
        let mut counted = false;
        for i in inlines {
            match i {
                Inline::VerseStart(s) => inside = s.contains(4),
                Inline::VerseEnd(s) if s.contains(4) => inside = false,
                Inline::Text(_) | Inline::Char { .. } if inside && !counted => {
                    blocks_touching_v4 += 1;
                    counted = true;
                }
                _ => {}
            }
        }
    }
    assert!(
        blocks_touching_v4 >= 3,
        "verse 4 must span at least three blocks, saw {blocks_touching_v4}"
    );

    let text = ch.verse_plain_text(4).unwrap();
    assert!(
        text.starts_with("Even though I walk through the valley"),
        "{text}"
    );
    assert!(text.ends_with("they comfort me."), "{text}");
    // The join across block boundaries must not weld words together.
    assert!(text.contains("with me. Your rod"), "{text}");
}

#[test]
fn psalm_23_has_six_verses_and_strongs_numbers_throughout() {
    let b = import("psa023-eng-web.usfm");
    let ch = &b.chapters[0];
    assert_eq!(ch.verse_numbers(), vec![1, 2, 3, 4, 5, 6]);

    fn count_strongs(v: &[Inline]) -> usize {
        v.iter()
            .map(|i| match i {
                Inline::Char {
                    style: CharStyle::Word { strong: Some(_) },
                    inner,
                } => 1 + count_strongs(inner),
                Inline::Char { inner, .. } => count_strongs(inner),
                _ => 0,
            })
            .sum()
    }
    let total: usize = ch
        .blocks
        .iter()
        .map(|b| match b {
            Block::Paragraph { inlines, .. } | Block::Heading { inlines, .. } => {
                count_strongs(inlines)
            }
            Block::Blank => 0,
        })
        .sum();
    assert!(
        total > 60,
        "expected the tagged edition's Strong's numbers, saw {total}"
    );
}

#[test]
fn john_1_preserves_words_of_jesus_with_nested_tagging() {
    let b = import("jhn001-eng-web.usfm");
    let ch = &b.chapters[0];

    fn find_wj(v: &[Inline], out: &mut Vec<Vec<Inline>>) {
        for i in v {
            if let Inline::Char { style, inner } = i {
                if *style == CharStyle::WordsOfJesus {
                    out.push(inner.clone());
                }
                find_wj(inner, out);
            }
        }
    }
    let mut spans = Vec::new();
    for block in &ch.blocks {
        if let Block::Paragraph { inlines, .. } = block {
            find_wj(inlines, &mut spans);
        }
    }
    assert!(!spans.is_empty(), "John 1 contains words of Jesus");

    // The \+w spans nested inside \wj must survive as children, with Strong's.
    let nested_tagged = spans.iter().any(|s| {
        s.iter().any(|i| {
            matches!(
                i,
                Inline::Char {
                    style: CharStyle::Word { strong: Some(_) },
                    ..
                }
            )
        })
    });
    assert!(nested_tagged, "nested \\+w spans must survive inside \\wj");

    // Verse 38 is the first: "What are you looking for?"
    let v38 = ch.verse_plain_text(38).unwrap();
    assert!(v38.contains("looking for"), "{v38}");
}

#[test]
fn john_1_hoists_every_footnote_and_keeps_them_out_of_the_text() {
    let b = import("jhn001-eng-web.usfm");
    let ch = &b.chapters[0];
    assert!(!ch.notes.is_empty(), "John 1 has footnotes");

    // Verse 5's footnote explains "overcome"; the body must not be searchable
    // as if it were scripture.
    let v5 = ch.verse_plain_text(5).unwrap();
    assert!(v5.contains("The light shines in the darkness"), "{v5}");
    assert!(
        !v5.contains("comprehended"),
        "footnote body leaked into verse text: {v5}"
    );

    // Every note reference in the blocks resolves to a hoisted note.
    fn refs(v: &[Inline], out: &mut Vec<u32>) {
        for i in v {
            match i {
                Inline::NoteRef(n) => out.push(n.0),
                Inline::Char { inner, .. } => refs(inner, out),
                _ => {}
            }
        }
    }
    let mut ids = Vec::new();
    for block in &ch.blocks {
        if let Block::Paragraph { inlines, .. } = block {
            refs(inlines, &mut ids);
        }
    }
    assert!(!ids.is_empty());
    for id in ids {
        assert!(
            ch.notes.iter().any(|n| n.id.0 == id),
            "dangling note ref {id}"
        );
    }
}

#[test]
fn john_1_preserves_the_paragraph_break_inside_verse_21() {
    let b = import("jhn001-eng-web.usfm");
    let ch = &b.chapters[0];
    let t = ch.verse_plain_text(21).unwrap();
    assert!(t.contains("Are you Elijah?"), "{t}");
    assert!(t.contains("I am not."), "{t}");
    // Both halves are present, and they came from two different blocks. The
    // join must insert a word space rather than welding the closing quote of
    // one block to the first word of the next.
    assert!(
        t.contains("Elijah?” He said"),
        "the block join must insert a space: {t}"
    );
}

#[test]
fn neither_fixture_contains_an_unmodelled_marker() {
    // This is the gate that makes "preserved, not dropped" meaningful: if the
    // importer ever meets a marker it does not model, this fails rather than
    // quietly losing the markup.
    for f in ["psa023-eng-web.usfm", "jhn001-eng-web.usfm"] {
        let b = import(f);
        for ch in &b.chapters {
            assert!(
                ch.unknown_markers().is_empty(),
                "{f} chapter {} has unmodelled markers: {:?}",
                ch.chapter,
                ch.unknown_markers()
            );
        }
    }
}
