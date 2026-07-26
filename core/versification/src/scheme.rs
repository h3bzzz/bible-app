//! Loading versification schemes from the vendored Copenhagen Alliance JSON.
//!
//! ADR-0004 records why this data source was chosen over raw Paratext `.vrs`
//! files. The four fields consumed here are `maxVerses`, `mappedVerses`,
//! `excludedVerses` and `partialVerses`.
//!
//! # Mapping direction
//!
//! Each scheme file maps **its own references to `org`**. `eng.json` contains
//! `"GEN 31:55": "GEN 32:1"`, meaning English Genesis 31:55 is Hebrew Genesis
//! 32:1. `org` is therefore the pivot for every conversion, which is why
//! ADR-0004 makes it the canonical storage scheme.

use bible_domain::{BookId, SchemeId, Subverse, VerseRef};
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt;

/// A defect found in a scheme file.
///
/// Upstream data is not perfect: `vul.json` ships an inverted range
/// (`DAG 3:52-23`) and two mappings whose endpoint counts cannot be reconciled.
/// Loading collects these rather than aborting, so a defect in a scheme nobody
/// bundles cannot block a build — while `build-corpus` in strict mode still
/// refuses to ship a scheme that has any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappingDefect {
    pub scheme: SchemeId,
    pub from: String,
    pub to: String,
    pub reason: DefectReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefectReason {
    /// A reference string that does not parse.
    UnparseableRef(String),
    /// A range whose end precedes its start.
    InvertedRange,
    /// Endpoint counts that are neither equal, nor a merge, nor a split.
    IrreconcilableCounts { from_len: usize, to_len: usize },
    /// A book code absent from the domain's book table.
    UnknownBook(String),
}

/// A mapping dropped because its target does not exist in `org`.
///
/// Distinct from [`MappingDefect`], and deliberately so. A defect means the
/// data is malformed — a typo, an uninterpretable range. A pruned mapping is
/// well-formed data describing a representation the pivot scheme does not use:
/// several schemes redirect `S3Y` (Song of the Three) onto `DAG 3` (Greek
/// Daniel), but `org` does not list `DAG` among its books at all.
///
/// Following such a mapping would make the source reference unaddressable in
/// the canonical scheme, and therefore un-bookmarkable. Dropping it lets the
/// positional identity stand, which `org` does support — it gives `S3Y` 67
/// verses. This is a normalisation, not an error, and lumping the two together
/// would bury two real typos under a hundred routine entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrunedMapping {
    pub scheme: SchemeId,
    pub from: String,
    pub to: String,
}

impl fmt::Display for PrunedMapping {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: dropped {:?} -> {:?} (target absent from org)",
            self.scheme, self.from, self.to
        )
    }
}

impl fmt::Display for MappingDefect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {:?} -> {:?}: ", self.scheme, self.from, self.to)?;
        match &self.reason {
            DefectReason::UnparseableRef(s) => write!(f, "unparseable reference {s:?}"),
            DefectReason::InvertedRange => write!(f, "range end precedes start"),
            DefectReason::IrreconcilableCounts { from_len, to_len } => write!(
                f,
                "cannot reconcile {from_len} source verses with {to_len} target verses"
            ),
            DefectReason::UnknownBook(b) => write!(f, "unknown book code {b:?}"),
        }
    }
}

/// Why a scheme file could not be loaded at all.
#[derive(Debug)]
pub enum SchemeLoadError {
    Json(serde_json::Error),
    BadSchemeId(String),
    UnknownBook(String),
}

impl fmt::Display for SchemeLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SchemeLoadError::Json(e) => write!(f, "malformed scheme JSON: {e}"),
            SchemeLoadError::BadSchemeId(s) => write!(f, "invalid scheme id {s:?}"),
            SchemeLoadError::UnknownBook(b) => write!(f, "unknown book code {b:?} in maxVerses"),
        }
    }
}

impl std::error::Error for SchemeLoadError {}

impl From<serde_json::Error> for SchemeLoadError {
    fn from(e: serde_json::Error) -> Self {
        SchemeLoadError::Json(e)
    }
}

#[derive(Deserialize)]
struct RawScheme {
    #[serde(rename = "maxVerses")]
    max_verses: HashMap<String, Vec<String>>,
    #[serde(rename = "mappedVerses", default)]
    mapped_verses: HashMap<String, String>,
    #[serde(rename = "excludedVerses", default)]
    excluded_verses: Vec<String>,
    #[serde(rename = "partialVerses", default)]
    partial_verses: HashMap<String, Vec<String>>,
}

/// One versification scheme, indexed for lookup in both directions.
#[derive(Debug, Clone)]
pub struct Scheme {
    id: SchemeId,
    /// `(book, chapter)` -> last verse number.
    last_verse: HashMap<(BookId, u16), u16>,
    /// `(book, chapter)` -> whether chapter exists is implied by `last_verse`;
    /// this records the chapter count for cheap bounds checks.
    chapter_count: HashMap<BookId, u16>,
    /// This scheme's reference -> the `org` reference(s) it corresponds to.
    to_org: HashMap<VerseKey, Vec<VerseRef>>,
    /// `org` reference -> this scheme's reference(s). The inverse index.
    from_org: HashMap<VerseKey, Vec<VerseRef>>,
    /// References this scheme does not contain at all.
    excluded: Vec<VerseRef>,
    /// References this scheme splits into lettered parts.
    partials: HashMap<VerseKey, Vec<Subverse>>,
}

/// Scheme-independent position, used as an index key.
///
/// [`VerseRef`] carries a scheme and would therefore never match across the
/// pivot; this is the positional part alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VerseKey {
    pub book: BookId,
    pub chapter: u16,
    pub verse: u16,
    pub subverse: Option<Subverse>,
}

impl From<VerseRef> for VerseKey {
    fn from(r: VerseRef) -> Self {
        VerseKey {
            book: r.book,
            chapter: r.chapter,
            verse: r.verse,
            subverse: r.subverse,
        }
    }
}

impl VerseKey {
    fn to_ref(self, scheme: SchemeId) -> VerseRef {
        VerseRef {
            book: self.book,
            chapter: self.chapter,
            verse: self.verse,
            subverse: self.subverse,
            scheme,
        }
    }
}

/// A parsed reference or reference range from a scheme file.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedSpan {
    book: BookId,
    chapter: u16,
    start: u16,
    end: u16,
    start_sub: Option<Subverse>,
    end_sub: Option<Subverse>,
}

impl ParsedSpan {
    fn len(&self) -> Option<usize> {
        if self.end < self.start {
            None
        } else {
            Some((self.end - self.start) as usize + 1)
        }
    }

    fn keys(&self) -> Vec<VerseKey> {
        (self.start..=self.end)
            .map(|v| VerseKey {
                book: self.book,
                chapter: self.chapter,
                verse: v,
                // Sub-verse parts are only meaningful on single-verse spans;
                // on a range the letters describe the endpoints, not each
                // member, so they are dropped rather than misapplied.
                subverse: if self.start == self.end {
                    self.start_sub
                } else {
                    None
                },
            })
            .collect()
    }
}

/// Parses `GEN 31:55`, `GEN 32:1-32`, `PSA 51:0`, `PSA 51:1a`.
///
/// Verse `0` is real: the Russian schemes number Hebrew superscriptions as
/// verse 0, so a `verse >= 1` assumption would silently drop them.
fn parse_span(s: &str) -> Result<ParsedSpan, DefectReason> {
    let bad = || DefectReason::UnparseableRef(s.to_string());
    let (book_str, rest) = s.split_once(' ').ok_or_else(bad)?;
    let book = BookId::from_usfm_code(book_str)
        .ok_or_else(|| DefectReason::UnknownBook(book_str.to_string()))?;
    let (chapter_str, verses) = rest.split_once(':').ok_or_else(bad)?;
    let chapter: u16 = chapter_str.parse().map_err(|_| bad())?;

    fn split_verse(v: &str) -> Result<(u16, Option<Subverse>), ()> {
        let digits_end = v.find(|c: char| !c.is_ascii_digit()).unwrap_or(v.len());
        if digits_end == 0 {
            return Err(());
        }
        let n: u16 = v[..digits_end].parse().map_err(|_| ())?;
        let sub = match v[digits_end..].trim() {
            "" => None,
            t => {
                let c = t.chars().next().ok_or(())?;
                if !c.is_ascii_lowercase() {
                    return Err(());
                }
                Some(Subverse(c as u8 - b'a'))
            }
        };
        Ok((n, sub))
    }

    let (start_s, end_s) = match verses.split_once('-') {
        Some((a, b)) => (a, Some(b)),
        None => (verses, None),
    };
    let (start, start_sub) = split_verse(start_s).map_err(|_| bad())?;
    let (end, end_sub) = match end_s {
        Some(b) => split_verse(b).map_err(|_| bad())?,
        None => (start, start_sub),
    };
    Ok(ParsedSpan {
        book,
        chapter,
        start,
        end,
        start_sub,
        end_sub,
    })
}

impl Scheme {
    pub fn id(&self) -> SchemeId {
        self.id
    }

    /// Last verse number of `(book, chapter)`, or `None` if the chapter does
    /// not exist in this scheme.
    pub fn last_verse(&self, book: BookId, chapter: u16) -> Option<u16> {
        self.last_verse.get(&(book, chapter)).copied()
    }

    /// Number of chapters this scheme gives `book`, or `None` if absent.
    pub fn chapter_count(&self, book: BookId) -> Option<u16> {
        self.chapter_count.get(&book).copied()
    }

    /// Whether a position exists in this scheme.
    pub fn contains(&self, r: VerseRef) -> bool {
        if self
            .excluded
            .iter()
            .any(|e| VerseKey::from(*e) == VerseKey::from(r))
        {
            return false;
        }
        match self.last_verse(r.book, r.chapter) {
            // Verse 0 is legitimate (superscriptions), hence `<=` with no
            // lower bound check beyond the type's own.
            Some(last) => r.verse <= last,
            None => false,
        }
    }

    /// Sub-verse parts this scheme defines for a position, if any.
    pub fn partial_parts(&self, r: VerseRef) -> &[Subverse] {
        self.partials
            .get(&VerseKey::from(r.whole_verse()))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Load from the JSON body of a scheme file.
    pub fn from_json(
        id: &str,
        json: &str,
    ) -> Result<(Scheme, Vec<MappingDefect>), SchemeLoadError> {
        let scheme_id =
            SchemeId::new(id).map_err(|_| SchemeLoadError::BadSchemeId(id.to_string()))?;
        let raw: RawScheme = serde_json::from_str(json)?;
        let mut defects = Vec::new();

        let mut last_verse = HashMap::new();
        let mut chapter_count = HashMap::new();
        for (book_code, chapters) in &raw.max_verses {
            let book = BookId::from_usfm_code(book_code)
                .ok_or_else(|| SchemeLoadError::UnknownBook(book_code.clone()))?;
            chapter_count.insert(book, chapters.len() as u16);
            for (i, last) in chapters.iter().enumerate() {
                let n: u16 = last.trim().parse().unwrap_or(0);
                last_verse.insert((book, i as u16 + 1), n);
            }
        }

        let mut to_org: HashMap<VerseKey, Vec<VerseRef>> = HashMap::new();
        let mut from_org: HashMap<VerseKey, Vec<VerseRef>> = HashMap::new();

        for (from_s, to_s) in &raw.mapped_verses {
            let defect = |reason| MappingDefect {
                scheme: scheme_id,
                from: from_s.clone(),
                to: to_s.clone(),
                reason,
            };
            let from = match parse_span(from_s) {
                Ok(v) => v,
                Err(r) => {
                    defects.push(defect(r));
                    continue;
                }
            };
            let to = match parse_span(to_s) {
                Ok(v) => v,
                Err(r) => {
                    defects.push(defect(r));
                    continue;
                }
            };
            let (Some(fl), Some(tl)) = (from.len(), to.len()) else {
                defects.push(defect(DefectReason::InvertedRange));
                continue;
            };

            let from_keys = from.keys();
            let to_keys = to.keys();
            // Three reconcilable shapes: a pairwise shift, a merge of several
            // source verses into one, and a split of one into several.
            let pairs: Vec<(VerseKey, Vec<VerseKey>)> = if fl == tl {
                from_keys
                    .iter()
                    .copied()
                    .zip(to_keys.iter().map(|k| vec![*k]))
                    .collect()
            } else if tl == 1 {
                from_keys.iter().map(|k| (*k, vec![to_keys[0]])).collect()
            } else if fl == 1 {
                vec![(from_keys[0], to_keys.clone())]
            } else {
                defects.push(defect(DefectReason::IrreconcilableCounts {
                    from_len: fl,
                    to_len: tl,
                }));
                continue;
            };

            for (f, ts) in pairs {
                let entry = to_org.entry(f).or_default();
                for t in &ts {
                    let tref = t.to_ref(SchemeId::ORG);
                    if !entry.contains(&tref) {
                        entry.push(tref);
                    }
                    let back = from_org.entry(*t).or_default();
                    let fref = f.to_ref(scheme_id);
                    if !back.contains(&fref) {
                        back.push(fref);
                    }
                }
            }
        }

        let mut excluded = Vec::new();
        for e in &raw.excluded_verses {
            match parse_span(e) {
                Ok(span) => excluded.extend(span.keys().into_iter().map(|k| k.to_ref(scheme_id))),
                Err(r) => defects.push(MappingDefect {
                    scheme: scheme_id,
                    from: e.clone(),
                    to: String::new(),
                    reason: r,
                }),
            }
        }

        let mut partials = HashMap::new();
        for (ref_s, parts) in &raw.partial_verses {
            match parse_span(ref_s) {
                Ok(span) => {
                    let subs: Vec<Subverse> = parts
                        .iter()
                        .filter_map(|p| p.trim().chars().next())
                        .filter(|c| c.is_ascii_lowercase())
                        .map(|c| Subverse(c as u8 - b'a'))
                        .collect();
                    if !subs.is_empty() {
                        partials.insert(span.keys()[0], subs);
                    }
                }
                Err(r) => defects.push(MappingDefect {
                    scheme: scheme_id,
                    from: ref_s.clone(),
                    to: String::new(),
                    reason: r,
                }),
            }
        }

        defects.sort_by(|a, b| (&a.from, &a.to).cmp(&(&b.from, &b.to)));

        Ok((
            Scheme {
                id: scheme_id,
                last_verse,
                chapter_count,
                to_org,
                from_org,
                excluded,
                partials,
            },
            defects,
        ))
    }

    /// This scheme's reference expressed in `org`.
    ///
    /// An unmapped reference maps positionally — that is the standard
    /// semantics, and the mapping tables list only the exceptions.
    pub(crate) fn to_org(&self, r: VerseRef) -> Vec<VerseRef> {
        match self.to_org.get(&VerseKey::from(r)) {
            Some(v) => v.clone(),
            None => vec![r.reinterpret(SchemeId::ORG)],
        }
    }

    /// An `org` reference expressed in this scheme.
    ///
    /// Defined as the true inverse of [`Scheme::to_org`]: a reference `r` is an
    /// image of `o` exactly when `r`'s own forward mapping lands on `o`.
    ///
    /// The identity must be tested rather than assumed, because a scheme can
    /// address the same content two ways. `eng` lists both `LJE` and `BAR`, and
    /// maps `BAR 6:1-73` onto `LJE 1:1-73` — so `org LJE 1:7` has two valid
    /// English addresses, `BAR 6:7` and `LJE 1:7`, and returning only the
    /// mapped one loses a reference the scheme genuinely contains. Conversely
    /// the identity must *not* be assumed valid: `eng GEN 32:1` maps to
    /// `org GEN 32:2`, so it is not an image of `org GEN 32:1` even though the
    /// position exists.
    // `from_*` normally names a constructor, and clippy says so. Here it is
    // half of the `to_org` / `from_org` pair that names the two directions of
    // one conversion, and breaking that symmetry would cost more clarity than
    // the lint saves.
    #[allow(clippy::wrong_self_convention)]
    pub(crate) fn from_org(&self, r: VerseRef) -> Vec<VerseRef> {
        let mut out = self
            .from_org
            .get(&VerseKey::from(r))
            .cloned()
            .unwrap_or_default();
        let identity = r.reinterpret(self.id);
        if !out.contains(&identity) && self.to_org(identity).contains(&r) {
            out.push(identity);
        }
        out
    }

    /// Drops mappings whose target does not exist in `org`, reporting each.
    ///
    /// Cross-scheme validation, so it cannot live in `from_json`. See
    /// [`PrunedMapping`] for why this is a normalisation rather than an error.
    pub(crate) fn prune_mappings_absent_from_org(&mut self, org: &Scheme) -> Vec<PrunedMapping> {
        let mut pruned = Vec::new();
        let mut dropped: Vec<(VerseKey, VerseRef)> = Vec::new();

        for (from, tos) in self.to_org.iter_mut() {
            tos.retain(|t| {
                if org.contains(*t) {
                    true
                } else {
                    dropped.push((*from, *t));
                    false
                }
            });
        }
        self.to_org.retain(|_, tos| !tos.is_empty());

        for (from, to) in dropped {
            if let Some(v) = self.from_org.get_mut(&VerseKey::from(to)) {
                v.retain(|r| VerseKey::from(*r) != from);
            }
            pruned.push(PrunedMapping {
                scheme: self.id,
                from: from.to_ref(self.id).to_string(),
                to: to.to_string(),
            });
        }
        self.from_org.retain(|_, v| !v.is_empty());
        pruned.sort_by(|a, b| (&a.from, &a.to).cmp(&(&b.from, &b.to)));
        pruned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_plain_reference() {
        let s = parse_span("GEN 31:55").unwrap();
        assert_eq!(s.book, BookId::Gen);
        assert_eq!((s.chapter, s.start, s.end), (31, 55, 55));
        assert_eq!(s.len(), Some(1));
    }

    #[test]
    fn parses_a_range() {
        let s = parse_span("GEN 32:1-32").unwrap();
        assert_eq!((s.start, s.end), (1, 32));
        assert_eq!(s.len(), Some(32));
        assert_eq!(s.keys().len(), 32);
    }

    #[test]
    fn parses_verse_zero_because_superscriptions_use_it() {
        let s = parse_span("PSA 89:0-1").unwrap();
        assert_eq!((s.start, s.end), (0, 1));
        assert_eq!(s.len(), Some(2));
    }

    #[test]
    fn parses_a_subverse_letter() {
        let s = parse_span("PSA 51:1a").unwrap();
        assert_eq!(s.start_sub, Some(Subverse(0)));
        assert_eq!(s.keys()[0].subverse, Some(Subverse(0)));
    }

    #[test]
    fn detects_the_inverted_range_shipped_in_vul() {
        let s = parse_span("DAG 3:52-23").unwrap();
        assert_eq!(s.len(), None, "an inverted range must not report a length");
    }

    #[test]
    fn rejects_unknown_books_and_malformed_input() {
        assert!(matches!(
            parse_span("ZZZ 1:1"),
            Err(DefectReason::UnknownBook(_))
        ));
        assert!(parse_span("GEN").is_err());
        assert!(parse_span("GEN 1").is_err());
        assert!(parse_span("GEN x:1").is_err());
        assert!(parse_span("GEN 1:x").is_err());
    }

    #[test]
    fn loads_a_minimal_scheme_and_indexes_both_directions() {
        // Mirrors the real Genesis 31/32 shift, where the source scheme's
        // final verse of chapter 1 opens chapter 2 in org and the whole of
        // chapter 2 shifts by one. Mapping the *whole* range matters: a
        // fixture that maps one verse and leaves its neighbours unmapped
        // describes a scheme where two positions mean the same thing, which
        // real scheme files never do.
        let json = r#"{
            "maxVerses": { "GEN": ["31", "25"], "MAL": ["14", "17", "24"] },
            "mappedVerses": { "GEN 1:31": "GEN 2:1", "GEN 2:1-25": "GEN 2:2-26" },
            "excludedVerses": [],
            "partialVerses": {}
        }"#;
        let (s, defects) = Scheme::from_json("eng", json).unwrap();
        assert!(defects.is_empty());
        assert_eq!(s.id(), SchemeId::ENG);
        assert_eq!(s.last_verse(BookId::Gen, 1), Some(31));
        assert_eq!(s.chapter_count(BookId::Mal), Some(3));
        assert_eq!(s.last_verse(BookId::Gen, 9), None);

        let r = VerseRef::new(BookId::Gen, 1, 31, SchemeId::ENG);
        assert_eq!(
            s.to_org(r),
            vec![VerseRef::new(BookId::Gen, 2, 1, SchemeId::ORG)]
        );
        // The inverse is the true inverse: eng GEN 2:1 is not an image of
        // org GEN 2:1, because its own forward mapping lands on org GEN 2:2.
        assert_eq!(
            s.from_org(VerseRef::new(BookId::Gen, 2, 1, SchemeId::ORG)),
            vec![r]
        );
    }

    #[test]
    fn the_inverse_keeps_an_alias_the_scheme_genuinely_contains() {
        // `eng` addresses the Letter of Jeremiah both as its own book and as
        // Baruch chapter 6. org LJE 1:7 therefore has two valid English
        // addresses, and dropping either loses a reference the scheme has.
        let json = r#"{
            "maxVerses": { "BAR": ["22","35","38","37","9","73"], "LJE": ["73"] },
            "mappedVerses": { "BAR 6:1-73": "LJE 1:1-73" },
            "excludedVerses": [], "partialVerses": {}
        }"#;
        let (s, _) = Scheme::from_json("eng", json).unwrap();
        let images = s.from_org(VerseRef::new(BookId::Lje, 1, 7, SchemeId::ORG));
        assert!(images.contains(&VerseRef::new(BookId::Bar, 6, 7, SchemeId::ENG)));
        assert!(images.contains(&VerseRef::new(BookId::Lje, 1, 7, SchemeId::ENG)));
        assert_eq!(images.len(), 2);
    }

    #[test]
    fn unmapped_references_map_positionally() {
        let json = r#"{ "maxVerses": { "GEN": ["31"] }, "mappedVerses": {},
                        "excludedVerses": [], "partialVerses": {} }"#;
        let (s, _) = Scheme::from_json("eng", json).unwrap();
        let r = VerseRef::new(BookId::Gen, 1, 1, SchemeId::ENG);
        assert_eq!(
            s.to_org(r),
            vec![VerseRef::new(BookId::Gen, 1, 1, SchemeId::ORG)]
        );
    }

    #[test]
    fn malformed_mappings_become_defects_not_panics() {
        let json = r#"{
            "maxVerses": { "DAG": ["10","20","90"] },
            "mappedVerses": { "DAG 3:52-23": "S3Y 1:30-31", "DAG 1:1-3": "DAG 1:1-9" },
            "excludedVerses": [], "partialVerses": {}
        }"#;
        let (_, defects) = Scheme::from_json("vul", json).unwrap();
        assert_eq!(defects.len(), 2);
        assert!(defects
            .iter()
            .any(|d| d.reason == DefectReason::InvertedRange));
        assert!(defects
            .iter()
            .any(|d| matches!(d.reason, DefectReason::IrreconcilableCounts { .. })));
    }

    #[test]
    fn merges_and_splits_are_both_reconciled() {
        let json = r#"{
            "maxVerses": { "SUS": ["63"], "DAG": ["21","49","100","37","31","28","28","27","27","21","45","13","64","42"] },
            "mappedVerses": { "DAG 13:1-63": "SUS 1:63", "SUS 1:1": "DAG 13:1-3" },
            "excludedVerses": [], "partialVerses": {}
        }"#;
        let (s, defects) = Scheme::from_json("vul", json).unwrap();
        assert!(defects.is_empty(), "{defects:?}");
        // Merge: 63 source verses collapse onto one target.
        let merged = s.to_org(VerseRef::new(BookId::Dag, 13, 40, SchemeId::VUL));
        assert_eq!(
            merged,
            vec![VerseRef::new(BookId::Sus, 1, 63, SchemeId::ORG)]
        );
        // Split: one source verse yields three targets.
        let split = s.to_org(VerseRef::new(BookId::Sus, 1, 1, SchemeId::VUL));
        assert_eq!(split.len(), 3);
    }

    #[test]
    fn excluded_verses_are_not_contained() {
        let json = r#"{ "maxVerses": { "3JN": ["15"] }, "mappedVerses": {},
                        "excludedVerses": ["3JN 1:15"], "partialVerses": {} }"#;
        let (s, _) = Scheme::from_json("eng", json).unwrap();
        assert!(s.contains(VerseRef::new(BookId::Jn3, 1, 14, SchemeId::ENG)));
        assert!(!s.contains(VerseRef::new(BookId::Jn3, 1, 15, SchemeId::ENG)));
    }

    #[test]
    fn partial_verses_are_indexed() {
        let json = r#"{ "maxVerses": { "JOS": ["18","24"] }, "mappedVerses": {},
                        "excludedVerses": [],
                        "partialVerses": { "JOS 9:2": ["-", "a", "b", "c "] } }"#;
        let (s, _) = Scheme::from_json("lxx", json).unwrap();
        let parts = s.partial_parts(VerseRef::new(BookId::Jos, 9, 2, SchemeId::LXX));
        assert_eq!(parts, &[Subverse(0), Subverse(1), Subverse(2)]);
    }
}
