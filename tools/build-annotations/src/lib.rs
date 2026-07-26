//! `build-annotations` — turns extracted reference pairs into an annotation
//! layer dataset (ADR-0009).
//!
//! Input: a TSV of `OT_REF<TAB>FULFILMENT;FULFILMENT…` in **English (KJV)
//! versification**, exactly as prophecy lists are published.
//!
//! Output: `messianic-prophecy.json`, authored in **`org`** — the canonical
//! scheme ADR-0009 requires, so the layer highlights the right verses in a
//! translation using Hebrew numbering (Joel 2:28 and Joel 3:1 are the same
//! verse; Malachi 4:5 is Malachi 3:23). The mapping is done here, once, at
//! dataset build time, by the same versification tables the reader uses.

#![forbid(unsafe_code)]

use bible_domain::{
    Annotation, AnnotationLayer, AnnotationPayload, BookId, LayerId, LayerKind, License,
    SchemeId, SourceRef, VerseRange, VerseRef,
};
use bible_versification::{SchemeRegistry, Versifier};
use std::fmt;

#[derive(Debug)]
pub enum AnnError {
    Parse { line: usize, reason: String },
    Unmappable { line: usize, reference: String },
    Validation(String),
    Versification(String),
}

impl fmt::Display for AnnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnnError::Parse { line, reason } => write!(f, "line {line}: {reason}"),
            AnnError::Unmappable { line, reference } => write!(
                f,
                "line {line}: {reference} has no position in org; \
                 a silent drop here would un-highlight a prophecy"
            ),
            AnnError::Validation(e) => write!(f, "{e}"),
            AnnError::Versification(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for AnnError {}

/// Parses `"GEN 3:15"`, `"GEN 9:26-27"`, `"ISA 52:13-53:12"`, or a whole
/// chapter `"PSA 88"` (expanded via the scheme's last verse).
pub fn parse_range(
    s: &str,
    scheme: SchemeId,
    reg: &SchemeRegistry,
) -> Result<VerseRange, String> {
    let (book_s, rest) = s
        .rsplit_once(' ')
        .ok_or_else(|| format!("malformed reference {s:?}"))?;
    let book =
        BookId::from_usfm_code(book_s).ok_or_else(|| format!("unknown book code {book_s:?}"))?;

    let make = |c: u16, v: u16| VerseRef::new(book, c, v, scheme);

    let (start, end) = match rest.split_once('-') {
        None => match rest.split_once(':') {
            // Whole chapter.
            None => {
                let c: u16 = rest.parse().map_err(|_| format!("bad chapter in {s:?}"))?;
                let last = reg
                    .last_verse(scheme, book, c)
                    .map_err(|e| e.to_string())?
                    .ok_or_else(|| format!("{s}: chapter absent from {scheme}"))?;
                (make(c, 1), make(c, last))
            }
            Some((c, v)) => {
                let c: u16 = c.parse().map_err(|_| format!("bad chapter in {s:?}"))?;
                let v: u16 = v.parse().map_err(|_| format!("bad verse in {s:?}"))?;
                (make(c, v), make(c, v))
            }
        },
        Some((first, second)) => {
            let (c1, v1) = first
                .split_once(':')
                .ok_or_else(|| format!("malformed reference {s:?}"))?;
            let c1: u16 = c1.parse().map_err(|_| format!("bad chapter in {s:?}"))?;
            let v1: u16 = v1.parse().map_err(|_| format!("bad verse in {s:?}"))?;
            match second.split_once(':') {
                // Cross-chapter: ISA 52:13-53:12.
                Some((c2, v2)) => {
                    let c2: u16 = c2.parse().map_err(|_| format!("bad chapter in {s:?}"))?;
                    let v2: u16 = v2.parse().map_err(|_| format!("bad verse in {s:?}"))?;
                    (make(c1, v1), make(c2, v2))
                }
                None => {
                    let v2: u16 = second.parse().map_err(|_| format!("bad verse in {s:?}"))?;
                    (make(c1, v1), make(c1, v2))
                }
            }
        }
    };
    VerseRange::new(start, end).map_err(|e| format!("{s}: {e}"))
}

/// Expresses an English-versification range in `org`.
///
/// Start and end are mapped independently; where the target splits a verse
/// the earliest (for start) and latest (for end) parts are taken, so the
/// range stays inclusive of everything the source range covered.
pub fn to_org(range: &VerseRange, reg: &SchemeRegistry) -> Result<Option<VerseRange>, AnnError> {
    let map = |r: VerseRef| -> Result<Vec<VerseRef>, AnnError> {
        reg.map(r, SchemeId::ORG)
            .map_err(|e| AnnError::Versification(e.to_string()))
    };
    let starts = map(range.start())?;
    let ends = map(range.end())?;
    let (Some(start), Some(end)) = (starts.into_iter().min(), ends.into_iter().max()) else {
        return Ok(None);
    };
    VerseRange::new(start, end)
        .map(Some)
        .map_err(|e| AnnError::Validation(format!("{range}: {e} after mapping to org")))
}

/// Formats a range the way the dataset and loader agree on.
pub fn fmt_range(r: &VerseRange) -> String {
    let (s, e) = (r.start(), r.end());
    let book = s.book.usfm_code();
    if s.chapter == e.chapter && s.verse == e.verse {
        format!("{book} {}:{}", s.chapter, s.verse)
    } else if s.chapter == e.chapter {
        format!("{book} {}:{}-{}", s.chapter, s.verse, e.verse)
    } else {
        format!("{book} {}:{}-{}:{}", s.chapter, s.verse, e.chapter, e.verse)
    }
}

/// One TSV line, mapped into org. `None` when the OT reference has no
/// position in org — reported by the caller, never silently dropped.
pub struct MappedRow {
    pub range: VerseRange,
    pub fulfilled_in: Vec<VerseRange>,
}

pub fn map_row(
    line_no: usize,
    line: &str,
    reg: &SchemeRegistry,
) -> Result<MappedRow, AnnError> {
    let (ot, fulfilments) = line.split_once('\t').ok_or_else(|| AnnError::Parse {
        line: line_no,
        reason: "expected two tab-separated fields".into(),
    })?;
    let parse = |s: &str| {
        parse_range(s, SchemeId::ENG, reg).map_err(|reason| AnnError::Parse {
            line: line_no,
            reason,
        })
    };

    let range = to_org(&parse(ot)?, reg)?.ok_or_else(|| AnnError::Unmappable {
        line: line_no,
        reference: ot.to_string(),
    })?;

    let mut fulfilled_in = Vec::new();
    for f in fulfilments.split(';').filter(|f| !f.trim().is_empty()) {
        // A fulfilment that cannot be expressed in org is also a hard error:
        // the domain rejects a prophecy with no fulfilment, so quietly losing
        // one here could invalidate the whole annotation.
        let mapped = to_org(&parse(f.trim())?, reg)?.ok_or_else(|| AnnError::Unmappable {
            line: line_no,
            reference: f.trim().to_string(),
        })?;
        if !fulfilled_in.contains(&mapped) {
            fulfilled_in.push(mapped);
        }
    }
    Ok(MappedRow {
        range,
        fulfilled_in,
    })
}

/// The layer this tool builds, matching what the loader and theme key on.
pub fn prophecy_layer(source_sha256: String) -> AnnotationLayer {
    AnnotationLayer {
        id: LayerId::new("messianic-prophecy").expect("static id is valid"),
        kind: LayerKind::MessianicProphecy,
        name: "Messianic prophecy".into(),
        spoken_label: "messianic prophecy".into(),
        license: License::public_domain(
            "Old Testament prophecy references with their New Testament fulfilment \
             citations. Scripture reference pairs are matters of textual record; \
             compiled with reference to \u{201c}351 Old Testament Prophecies Fulfilled \
             in Jesus Christ\u{201d} (newtestamentchristians.com). That page's \
             descriptive text is its author's and is not included.",
            SourceRef {
                url: "https://www.newtestamentchristians.com/bible-study-resources/351-old-testament-prophecies-fulfilled-in-jesus-christ/".into(),
                sha256: source_sha256,
                publisher: "newtestamentchristians.com (reference list)".into(),
            },
        ),
        // ADR-0009: interpretive layers default off. The application must not
        // assert a contested reading the user did not ask for.
        enabled_by_default: false,
    }
}

/// Builds, merges and validates the full annotation set from TSV text.
///
/// Rows sharing an identical org range merge into one annotation with the
/// union of fulfilments — the source list numbers prophecies by *claim*
/// (Genesis 3:15 appears twice), but a highlight layer marks *spans*.
pub fn build_annotations(
    tsv: &str,
    reg: &SchemeRegistry,
) -> Result<Vec<Annotation>, AnnError> {
    let layer_id = LayerId::new("messianic-prophecy").expect("static id is valid");
    let mut out: Vec<Annotation> = Vec::new();
    for (i, line) in tsv.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let row = map_row(i + 1, line, reg)?;
        if let Some(existing) = out.iter_mut().find(|a| a.range == row.range) {
            if let AnnotationPayload::MessianicProphecy { fulfilled_in, .. } =
                &mut existing.payload
            {
                for f in row.fulfilled_in {
                    if !fulfilled_in.contains(&f) {
                        fulfilled_in.push(f);
                    }
                }
            }
        } else {
            out.push(Annotation {
                layer: layer_id.clone(),
                range: row.range,
                payload: AnnotationPayload::MessianicProphecy {
                    fulfilled_in: row.fulfilled_in,
                    note: None,
                },
            });
        }
    }

    let layer = prophecy_layer("0".repeat(64));
    for a in &out {
        a.validate(&layer)
            .map_err(|e| AnnError::Validation(e.to_string()))?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reg() -> SchemeRegistry {
        SchemeRegistry::embedded().expect("embedded schemes load")
    }

    #[test]
    fn plain_and_ranged_references_parse() {
        let r = reg();
        let single = parse_range("GEN 3:15", SchemeId::ENG, &r).unwrap();
        assert!(single.is_single_verse());
        let ranged = parse_range("GEN 9:26-27", SchemeId::ENG, &r).unwrap();
        assert_eq!(ranged.start().verse, 26);
        assert_eq!(ranged.end().verse, 27);
        let cross = parse_range("ISA 52:13-53:12", SchemeId::ENG, &r).unwrap();
        assert_eq!(cross.start().chapter, 52);
        assert_eq!(cross.end().chapter, 53);
    }

    #[test]
    fn a_whole_chapter_reference_expands_to_the_schemes_last_verse() {
        let r = reg();
        let psalm88 = parse_range("PSA 88", SchemeId::ENG, &r).unwrap();
        assert_eq!(psalm88.start().verse, 1);
        assert_eq!(
            Some(psalm88.end().verse),
            r.last_verse(SchemeId::ENG, BookId::Psa, 88).unwrap()
        );
    }

    #[test]
    fn malachi_4_5_lands_in_hebrew_numbering() {
        // The whole reason mapping happens at build time: English Malachi 4:5
        // is org Malachi 3:23. A dataset that skipped this would highlight
        // nothing in a translation using Hebrew numbering.
        let r = reg();
        let eng = parse_range("MAL 4:5", SchemeId::ENG, &r).unwrap();
        let org = to_org(&eng, &r).unwrap().unwrap();
        assert_eq!(org.start().chapter, 3);
        assert_eq!(org.start().verse, 23);
    }

    #[test]
    fn duplicate_ot_ranges_merge_their_fulfilments() {
        let r = reg();
        let tsv = "GEN 3:15\tGAL 4:4-5;MAT 1:18\nGEN 3:15\tHEB 2:14;1JN 3:8\n";
        let anns = build_annotations(tsv, &r).unwrap();
        assert_eq!(anns.len(), 1);
        let AnnotationPayload::MessianicProphecy { fulfilled_in, .. } = &anns[0].payload else {
            panic!("wrong payload kind");
        };
        assert_eq!(fulfilled_in.len(), 4);
    }

    #[test]
    fn every_annotation_comes_out_in_org() {
        let r = reg();
        let anns = build_annotations("MAL 4:5\tMAT 3:1-3\n", &r).unwrap();
        assert!(anns.iter().all(|a| a.range.scheme() == SchemeId::ORG));
    }

    #[test]
    fn a_garbled_line_is_an_error_with_its_line_number() {
        let r = reg();
        let e = build_annotations("not a reference\n", &r).unwrap_err();
        assert!(e.to_string().contains("line 1"), "{e}");
    }
}
