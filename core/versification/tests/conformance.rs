//! Versification conformance suite (ADR-0004).
//!
//! Runs against the real vendored scheme data, not fixtures. Risk #1 in the
//! register is that a mapping bug silently relocates a user's bookmarks and
//! notes, with no server to repair the damage. This suite is the control.
//!
//! Every expectation below was read out of `data/versification/*.json` before
//! it was written down here, not recalled from memory.

use bible_domain::{BookId, SchemeId, VerseRef};
use bible_versification::{SchemeRegistry, Versifier};

fn reg() -> SchemeRegistry {
    SchemeRegistry::embedded().expect("embedded schemes must load")
}

fn eng(book: BookId, c: u16, v: u16) -> VerseRef {
    VerseRef::new(book, c, v, SchemeId::ENG)
}

fn org(book: BookId, c: u16, v: u16) -> VerseRef {
    VerseRef::new(book, c, v, SchemeId::ORG)
}

/// Asserts an exact one-to-one mapping in both directions.
#[track_caller]
fn assert_bijective(r: &SchemeRegistry, from: VerseRef, to: VerseRef) {
    assert_eq!(
        r.map(from, to.scheme).unwrap(),
        vec![to],
        "forward: {from} -> {}",
        to.scheme
    );
    assert_eq!(
        r.map(to, from.scheme).unwrap(),
        vec![from],
        "reverse: {to} -> {}",
        from.scheme
    );
}

// ── Psalms superscriptions ────────────────────────────────────────────────
//
// The Hebrew tradition numbers a psalm's superscription as verse 1 (and
// sometimes verses 1-2); English translations print it unnumbered. Psalm 51
// therefore has 21 verses in `org` and 19 in `eng`.

#[test]
fn psalm_51_superscription_shifts_the_whole_psalm_by_two() {
    let r = reg();
    assert_bijective(&r, eng(BookId::Psa, 51, 1), org(BookId::Psa, 51, 3));
    assert_bijective(&r, eng(BookId::Psa, 51, 19), org(BookId::Psa, 51, 21));
}

#[test]
fn psalm_51_verse_counts_differ_between_schemes() {
    let r = reg();
    assert_eq!(
        r.last_verse(SchemeId::ORG, BookId::Psa, 51).unwrap(),
        Some(21)
    );
    assert_eq!(
        r.last_verse(SchemeId::ENG, BookId::Psa, 51).unwrap(),
        Some(19)
    );
}

#[test]
fn psalm_23_does_not_shift_despite_having_a_hebrew_title() {
    // Corrects an assumption in the original design document: Psalm 23 has six
    // verses in both schemes. Its `\d` superscription is unnumbered in both.
    let r = reg();
    assert_eq!(
        r.last_verse(SchemeId::ORG, BookId::Psa, 23).unwrap(),
        Some(6)
    );
    assert_eq!(
        r.last_verse(SchemeId::ENG, BookId::Psa, 23).unwrap(),
        Some(6)
    );
    for v in 1..=6 {
        assert_bijective(&r, eng(BookId::Psa, 23, v), org(BookId::Psa, 23, v));
    }
}

// ── Chapter-boundary disagreements ────────────────────────────────────────

#[test]
fn genesis_31_55_is_hebrew_genesis_32_1() {
    // English Genesis 31 has 55 verses; Hebrew Genesis 31 has 54, and the
    // extra verse opens chapter 32. The whole of Genesis 32 shifts by one.
    let r = reg();
    assert_bijective(&r, eng(BookId::Gen, 31, 55), org(BookId::Gen, 32, 1));
    assert_bijective(&r, eng(BookId::Gen, 32, 1), org(BookId::Gen, 32, 2));
    assert_bijective(&r, eng(BookId::Gen, 32, 32), org(BookId::Gen, 32, 33));
}

#[test]
fn malachi_has_four_chapters_in_english_and_three_in_hebrew() {
    let r = reg();
    let s_eng = r.scheme(SchemeId::ENG).unwrap();
    let s_org = r.scheme(SchemeId::ORG).unwrap();
    assert_eq!(s_eng.chapter_count(BookId::Mal), Some(4));
    assert_eq!(s_org.chapter_count(BookId::Mal), Some(3));

    assert_bijective(&r, eng(BookId::Mal, 4, 1), org(BookId::Mal, 3, 19));
    assert_bijective(&r, eng(BookId::Mal, 4, 6), org(BookId::Mal, 3, 24));
}

#[test]
fn joel_has_three_chapters_in_english_and_four_in_hebrew() {
    let r = reg();
    assert_eq!(
        r.scheme(SchemeId::ENG).unwrap().chapter_count(BookId::Jol),
        Some(3)
    );
    assert_eq!(
        r.scheme(SchemeId::ORG).unwrap().chapter_count(BookId::Jol),
        Some(4)
    );

    // The famous "I will pour out my Spirit" passage.
    assert_bijective(&r, eng(BookId::Jol, 2, 28), org(BookId::Jol, 3, 1));
    assert_bijective(&r, eng(BookId::Jol, 3, 1), org(BookId::Jol, 4, 1));
}

// ── Absence is a real answer ──────────────────────────────────────────────

#[test]
fn three_john_has_fifteen_verses_in_both_schemes() {
    // Often cited as a versification edge case, and in these two schemes it is
    // not one. Asserting that keeps a future "fix" from inventing a mapping.
    let r = reg();
    assert_eq!(
        r.last_verse(SchemeId::ORG, BookId::Jn3, 1).unwrap(),
        Some(15)
    );
    assert_eq!(
        r.last_verse(SchemeId::ENG, BookId::Jn3, 1).unwrap(),
        Some(15)
    );
    assert_bijective(&r, eng(BookId::Jn3, 1, 15), org(BookId::Jn3, 1, 15));
}

#[test]
fn a_verse_absent_from_the_target_scheme_maps_to_nothing() {
    let r = reg();
    // Hebrew Malachi has three chapters, so English Malachi 4 exists only via
    // mapping — but a chapter beyond either scheme maps to nothing at all.
    let beyond = eng(BookId::Mal, 4, 99);
    assert_eq!(r.map(beyond, SchemeId::ORG).unwrap(), vec![]);
}

#[test]
fn romans_16_is_not_a_versification_difference() {
    // Corrects a second assumption in the design document. The Romans
    // 16:25-27 problem is one of manuscript placement, not of verse
    // numbering: `eng` and `org` agree on all 27 verses. Encoding it as a
    // versification mapping would be wrong.
    let r = reg();
    assert_eq!(
        r.last_verse(SchemeId::ORG, BookId::Rom, 16).unwrap(),
        Some(27)
    );
    assert_eq!(
        r.last_verse(SchemeId::ENG, BookId::Rom, 16).unwrap(),
        Some(27)
    );
    for v in 25..=27 {
        assert_bijective(&r, eng(BookId::Rom, 16, v), org(BookId::Rom, 16, v));
    }
}

// ── Cross-book mapping ────────────────────────────────────────────────────

#[test]
fn the_letter_of_jeremiah_maps_across_a_book_boundary() {
    // 130 mappings in the vendored data cross book boundaries. English Bibles
    // print the Letter of Jeremiah as Baruch chapter 6; the original treats it
    // as its own book. A reference type that assumed a fixed book could not
    // represent this at all.
    let r = reg();
    let mapped = r.map(eng(BookId::Bar, 6, 7), SchemeId::ORG).unwrap();
    assert_eq!(mapped, vec![org(BookId::Lje, 1, 7)]);
    assert_ne!(mapped[0].book, BookId::Bar, "the book itself changes");
}

#[test]
fn both_english_addresses_for_the_letter_of_jeremiah_resolve() {
    // The inverse direction of the above: `eng` contains the passage under two
    // names, and an English translation may print it either way, so mapping
    // out of `org` must offer both rather than picking one.
    let r = reg();
    let images = r.map(org(BookId::Lje, 1, 7), SchemeId::ENG).unwrap();
    assert!(images.contains(&eng(BookId::Bar, 6, 7)), "{images:?}");
    assert!(images.contains(&eng(BookId::Lje, 1, 7)), "{images:?}");
}

#[test]
fn the_song_of_the_three_stays_addressable_in_the_canonical_scheme() {
    // The upstream `org` and `eng` files both redirect S3Y onto `DAG 3` (Greek
    // Daniel), yet `org` does not list DAG among its books. Followed
    // literally, that would leave the Song of the Three with no canonical
    // address — and so un-bookmarkable, since ADR-0004 stores every user
    // reference in `org`.
    //
    // The registry prunes those mappings and lets the positional identity
    // stand, which `org` does support: it gives S3Y 67 verses.
    let r = reg();
    assert_eq!(
        r.last_verse(SchemeId::ORG, BookId::S3y, 1).unwrap(),
        Some(67)
    );

    let mapped = r
        .map(
            VerseRef::new(BookId::S3y, 1, 1, SchemeId::ENG),
            SchemeId::ORG,
        )
        .unwrap();
    assert_eq!(mapped, vec![org(BookId::S3y, 1, 1)]);
    assert!(
        r.contains(mapped[0]).unwrap(),
        "the result must exist in org"
    );

    // The pruning is recorded rather than silent.
    let pruned = r.pruned_mappings();
    assert!(
        pruned
            .iter()
            .any(|p| p.from.starts_with("S3Y") && p.to.starts_with("DAG")),
        "S3Y -> DAG pruning must be reported"
    );
    // And it is not misfiled as a data defect.
    assert!(r.defects().iter().all(|d| !d.from.starts_with("S3Y")));
}

// ── Verse zero ────────────────────────────────────────────────────────────

#[test]
fn verse_zero_exists_in_the_russian_schemes() {
    // The Russian Synodal schemes number superscriptions as verse 0. Code that
    // assumed verses start at 1 would drop them silently.
    let r = reg();
    let v0 = VerseRef::new(BookId::Psa, 141, 0, SchemeId::RSC);
    assert!(r.contains(v0).unwrap(), "PSA 141:0 must exist in rsc");
    let mapped = r.map(v0, SchemeId::ORG).unwrap();
    assert!(!mapped.is_empty(), "verse 0 must map somewhere");
}

// ── Properties over the whole corpus of schemes ───────────────────────────

/// Round trip: every mapped reference must return to itself.
///
/// This is the strongest available guarantee that a bookmark stored in `org`
/// and displayed in a translation's own scheme lands back on the same verse.
#[test]
fn mapping_round_trips_for_every_verse_of_every_scheme() {
    let r = reg();
    let schemes = [
        SchemeId::ENG,
        SchemeId::LXX,
        SchemeId::VUL,
        SchemeId::RSO,
        SchemeId::RSC,
    ];

    let mut checked = 0usize;
    let mut lossy = Vec::new();

    for scheme_id in schemes {
        let scheme = r.scheme(scheme_id).unwrap();
        for &book in BookId::ALL {
            let Some(nch) = scheme.chapter_count(book) else {
                continue;
            };
            for chapter in 1..=nch {
                let Some(last) = scheme.last_verse(book, chapter) else {
                    continue;
                };
                for verse in 0..=last {
                    let v = VerseRef::new(book, chapter, verse, scheme_id);
                    if !scheme.contains(v) {
                        continue;
                    }
                    checked += 1;
                    let via_org = r.map(v, SchemeId::ORG).unwrap();
                    if via_org.is_empty() {
                        continue; // legitimately absent from org
                    }
                    let back: Vec<VerseRef> = via_org
                        .iter()
                        .flat_map(|o| r.map(*o, scheme_id).unwrap())
                        .collect();
                    if !back.contains(&v) {
                        lossy.push((v, via_org.clone(), back));
                    }
                }
            }
        }
    }

    assert!(
        checked > 100_000,
        "suite must exercise the whole corpus, saw {checked}"
    );
    assert!(
        lossy.is_empty(),
        "{} of {checked} references did not survive a round trip; first 10: {:#?}",
        lossy.len(),
        &lossy[..lossy.len().min(10)]
    );
}

/// Nothing may map to a position the target scheme does not contain.
#[test]
fn mapping_never_produces_a_reference_the_target_scheme_lacks() {
    let r = reg();
    let mut checked = 0usize;
    for scheme_id in [SchemeId::ENG, SchemeId::LXX, SchemeId::RSC] {
        let scheme = r.scheme(scheme_id).unwrap();
        for &book in BookId::ALL {
            let Some(nch) = scheme.chapter_count(book) else {
                continue;
            };
            for chapter in 1..=nch {
                let Some(last) = scheme.last_verse(book, chapter) else {
                    continue;
                };
                for verse in 0..=last {
                    let v = VerseRef::new(book, chapter, verse, scheme_id);
                    for target in [SchemeId::ORG, SchemeId::VUL] {
                        for m in r.map(v, target).unwrap() {
                            checked += 1;
                            assert!(
                                r.contains(m).unwrap(),
                                "{v} -> {target} produced {m}, which {target} does not contain"
                            );
                        }
                    }
                }
            }
        }
    }
    assert!(checked > 100_000, "expected broad coverage, saw {checked}");
}
