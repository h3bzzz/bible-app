//! Scripture references.
//!
//! ADR-0004: a reference without a versification scheme is meaningless, so
//! there is no constructor that omits one. Everything persisted in user data is
//! stored in [`SchemeId::ORG`]; display converts at render time.

use crate::book::BookId;
use core::fmt;

/// A versification scheme identifier, e.g. `org`, `eng`, `lxx`.
///
/// Stored as an inline ASCII string rather than an enum so that adding a
/// scheme costs one data file and no code change — see ADR-0004. Inline rather
/// than `String` so [`VerseRef`] stays `Copy`, which it must be to be pleasant
/// to use in the thousands of places a reference appears.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SchemeId {
    bytes: [u8; 8],
    len: u8,
}

/// Rejected scheme identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemeIdError {
    Empty,
    TooLong(usize),
    NotAscii,
}

impl fmt::Display for SchemeIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SchemeIdError::Empty => write!(f, "scheme id is empty"),
            SchemeIdError::TooLong(n) => write!(f, "scheme id is {n} bytes, maximum is 8"),
            SchemeIdError::NotAscii => write!(f, "scheme id must be lowercase ASCII"),
        }
    }
}

impl std::error::Error for SchemeIdError {}

impl SchemeId {
    pub const ORG: SchemeId = SchemeId::from_static("org");
    pub const ENG: SchemeId = SchemeId::from_static("eng");
    pub const LXX: SchemeId = SchemeId::from_static("lxx");
    pub const VUL: SchemeId = SchemeId::from_static("vul");
    pub const RSO: SchemeId = SchemeId::from_static("rso");
    pub const RSC: SchemeId = SchemeId::from_static("rsc");

    /// `const` constructor for the built-in schemes. Panics at compile time on
    /// a malformed literal.
    pub const fn from_static(s: &'static str) -> SchemeId {
        let src = s.as_bytes();
        assert!(!src.is_empty(), "scheme id is empty");
        assert!(src.len() <= 8, "scheme id longer than 8 bytes");
        let mut bytes = [0u8; 8];
        let mut i = 0;
        while i < src.len() {
            let c = src[i];
            assert!(
                c.is_ascii_lowercase() || c.is_ascii_digit(),
                "scheme id must be lowercase ASCII alphanumeric"
            );
            bytes[i] = c;
            i += 1;
        }
        SchemeId {
            bytes,
            len: src.len() as u8,
        }
    }

    /// Runtime constructor, for scheme ids read from data files.
    pub fn new(s: &str) -> Result<SchemeId, SchemeIdError> {
        if s.is_empty() {
            return Err(SchemeIdError::Empty);
        }
        if s.len() > 8 {
            return Err(SchemeIdError::TooLong(s.len()));
        }
        if !s
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        {
            return Err(SchemeIdError::NotAscii);
        }
        let mut bytes = [0u8; 8];
        bytes[..s.len()].copy_from_slice(s.as_bytes());
        Ok(SchemeId {
            bytes,
            len: s.len() as u8,
        })
    }

    pub fn as_str(&self) -> &str {
        // Safe: `new`/`from_static` admit only ASCII, so the prefix is valid UTF-8.
        core::str::from_utf8(&self.bytes[..self.len as usize]).unwrap_or("")
    }
}

impl fmt::Display for SchemeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Debug for SchemeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SchemeId({})", self.as_str())
    }
}

/// A sub-verse part, where 0 is `a`, 1 is `b`, and so on.
///
/// Needed because several schemes split a verse across a boundary — the
/// `partialVerses` field in the versification data — and because Romans
/// 16:25–27 moves in parts between traditions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Subverse(pub u8);

impl fmt::Display for Subverse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", (b'a' + self.0) as char)
    }
}

/// A single verse, always qualified by the scheme that gives it meaning.
///
/// Field order is load-bearing: the derived `Ord` compares position first and
/// scheme last, so sorting a list of same-scheme references yields canonical
/// order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VerseRef {
    pub book: BookId,
    pub chapter: u16,
    pub verse: u16,
    pub subverse: Option<Subverse>,
    pub scheme: SchemeId,
}

impl VerseRef {
    pub const fn new(book: BookId, chapter: u16, verse: u16, scheme: SchemeId) -> VerseRef {
        VerseRef {
            book,
            chapter,
            verse,
            subverse: None,
            scheme,
        }
    }

    pub const fn with_subverse(
        book: BookId,
        chapter: u16,
        verse: u16,
        subverse: Subverse,
        scheme: SchemeId,
    ) -> VerseRef {
        VerseRef {
            book,
            chapter,
            verse,
            subverse: Some(subverse),
            scheme,
        }
    }

    /// The same position, reinterpreted in another scheme **without mapping**.
    ///
    /// This is not a conversion. It is only correct where the caller already
    /// knows the position is unmapped between the two schemes. Real conversion
    /// goes through the versification crate's `Versifier`.
    pub const fn reinterpret(self, scheme: SchemeId) -> VerseRef {
        VerseRef { scheme, ..self }
    }

    /// Drop the sub-verse part, yielding the whole verse.
    pub const fn whole_verse(self) -> VerseRef {
        VerseRef {
            subverse: None,
            ..self
        }
    }
}

impl fmt::Display for VerseRef {
    /// `GEN 1:1`, `PSA 51:1a`. Machine-facing; UI formats with localised book
    /// names and the user's punctuation conventions.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {}:{}",
            self.book.usfm_code(),
            self.chapter,
            self.verse
        )?;
        if let Some(sv) = self.subverse {
            write!(f, "{sv}")?;
        }
        Ok(())
    }
}

/// An inclusive range of verses within one scheme.
///
/// Ranges routinely cross chapter boundaries (Romans 16:25–27 is tame; reading
/// plans and cross-references are worse), so this is not chapter-scoped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VerseRange {
    start: VerseRef,
    end: VerseRef,
}

/// Why a range was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RangeError {
    /// Endpoints in different schemes cannot be ordered against each other.
    SchemeMismatch { start: SchemeId, end: SchemeId },
    /// `end` precedes `start` in canonical order.
    Inverted { start: VerseRef, end: VerseRef },
}

impl fmt::Display for RangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RangeError::SchemeMismatch { start, end } => {
                write!(
                    f,
                    "range endpoints are in different schemes: {start} and {end}"
                )
            }
            RangeError::Inverted { start, end } => {
                write!(f, "range end {end} precedes start {start}")
            }
        }
    }
}

impl std::error::Error for RangeError {}

impl VerseRange {
    pub fn new(start: VerseRef, end: VerseRef) -> Result<VerseRange, RangeError> {
        if start.scheme != end.scheme {
            return Err(RangeError::SchemeMismatch {
                start: start.scheme,
                end: end.scheme,
            });
        }
        if end < start {
            return Err(RangeError::Inverted { start, end });
        }
        Ok(VerseRange { start, end })
    }

    /// A range covering exactly one verse.
    pub fn single(r: VerseRef) -> VerseRange {
        VerseRange { start: r, end: r }
    }

    pub fn start(&self) -> VerseRef {
        self.start
    }

    pub fn end(&self) -> VerseRef {
        self.end
    }

    pub fn scheme(&self) -> SchemeId {
        self.start.scheme
    }

    pub fn is_single_verse(&self) -> bool {
        self.start == self.end
    }

    /// Whether `r` falls inside this range. Returns `false` for a reference in
    /// a different scheme rather than guessing — the caller must map first.
    pub fn contains(&self, r: VerseRef) -> bool {
        r.scheme == self.scheme() && r >= self.start && r <= self.end
    }

    pub fn overlaps(&self, other: &VerseRange) -> bool {
        self.scheme() == other.scheme() && self.start <= other.end && other.start <= self.end
    }
}

impl fmt::Display for VerseRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_single_verse() {
            return write!(f, "{}", self.start);
        }
        if self.start.book == self.end.book && self.start.chapter == self.end.chapter {
            write!(f, "{}-{}", self.start, self.end.verse)?;
            if let Some(sv) = self.end.subverse {
                write!(f, "{sv}")?;
            }
            Ok(())
        } else {
            write!(f, "{}-{}", self.start, self.end)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheme_ids_round_trip() {
        for s in ["org", "eng", "lxx", "vul", "rso", "rsc"] {
            assert_eq!(SchemeId::new(s).unwrap().as_str(), s);
        }
        assert_eq!(SchemeId::ORG.as_str(), "org");
        assert_eq!(SchemeId::ENG, SchemeId::new("eng").unwrap());
    }

    #[test]
    fn scheme_ids_reject_malformed_input() {
        assert_eq!(SchemeId::new(""), Err(SchemeIdError::Empty));
        assert_eq!(
            SchemeId::new("waaaaytoolong"),
            Err(SchemeIdError::TooLong(13))
        );
        assert_eq!(SchemeId::new("ENG"), Err(SchemeIdError::NotAscii));
        assert_eq!(SchemeId::new("en-gb"), Err(SchemeIdError::NotAscii));
    }

    #[test]
    fn verse_refs_display_in_usfm_style() {
        let r = VerseRef::new(BookId::Gen, 1, 1, SchemeId::ORG);
        assert_eq!(r.to_string(), "GEN 1:1");
        let sv = VerseRef::with_subverse(BookId::Psa, 51, 1, Subverse(0), SchemeId::ENG);
        assert_eq!(sv.to_string(), "PSA 51:1a");
    }

    #[test]
    fn ordering_is_canonical_and_ignores_scheme_until_last() {
        let gen = VerseRef::new(BookId::Gen, 1, 1, SchemeId::ORG);
        let gen2 = VerseRef::new(BookId::Gen, 1, 2, SchemeId::ORG);
        let exo = VerseRef::new(BookId::Exo, 1, 1, SchemeId::ORG);
        assert!(gen < gen2);
        assert!(gen2 < exo);

        let mut refs = vec![exo, gen2, gen];
        refs.sort();
        assert_eq!(refs, vec![gen, gen2, exo]);
    }

    #[test]
    fn subverse_sorts_after_whole_verse() {
        let whole = VerseRef::new(BookId::Psa, 51, 1, SchemeId::ORG);
        let a = VerseRef::with_subverse(BookId::Psa, 51, 1, Subverse(0), SchemeId::ORG);
        let b = VerseRef::with_subverse(BookId::Psa, 51, 1, Subverse(1), SchemeId::ORG);
        assert!(whole < a);
        assert!(a < b);
        assert_eq!(a.whole_verse(), whole);
    }

    #[test]
    fn ranges_reject_mixed_schemes() {
        let s = VerseRef::new(BookId::Rom, 16, 25, SchemeId::ORG);
        let e = VerseRef::new(BookId::Rom, 16, 27, SchemeId::ENG);
        assert!(matches!(
            VerseRange::new(s, e),
            Err(RangeError::SchemeMismatch { .. })
        ));
    }

    #[test]
    fn ranges_reject_inversion() {
        let s = VerseRef::new(BookId::Rom, 16, 27, SchemeId::ORG);
        let e = VerseRef::new(BookId::Rom, 16, 25, SchemeId::ORG);
        assert!(matches!(
            VerseRange::new(s, e),
            Err(RangeError::Inverted { .. })
        ));
    }

    #[test]
    fn ranges_cross_chapter_boundaries() {
        let s = VerseRef::new(BookId::Jhn, 1, 50, SchemeId::ORG);
        let e = VerseRef::new(BookId::Jhn, 2, 3, SchemeId::ORG);
        let r = VerseRange::new(s, e).unwrap();
        assert!(r.contains(VerseRef::new(BookId::Jhn, 1, 51, SchemeId::ORG)));
        assert!(r.contains(VerseRef::new(BookId::Jhn, 2, 1, SchemeId::ORG)));
        assert!(!r.contains(VerseRef::new(BookId::Jhn, 2, 4, SchemeId::ORG)));
        assert_eq!(r.to_string(), "JHN 1:50-JHN 2:3");
    }

    #[test]
    fn contains_is_false_across_schemes_rather_than_guessing() {
        let r = VerseRange::new(
            VerseRef::new(BookId::Psa, 51, 1, SchemeId::ORG),
            VerseRef::new(BookId::Psa, 51, 5, SchemeId::ORG),
        )
        .unwrap();
        assert!(!r.contains(VerseRef::new(BookId::Psa, 51, 3, SchemeId::ENG)));
        assert!(r.contains(VerseRef::new(BookId::Psa, 51, 3, SchemeId::ORG)));
    }

    #[test]
    fn same_chapter_ranges_display_compactly() {
        let r = VerseRange::new(
            VerseRef::new(BookId::Rom, 16, 25, SchemeId::ORG),
            VerseRef::new(BookId::Rom, 16, 27, SchemeId::ORG),
        )
        .unwrap();
        assert_eq!(r.to_string(), "ROM 16:25-27");
        assert_eq!(
            VerseRange::single(VerseRef::new(BookId::Jhn, 3, 16, SchemeId::ORG)).to_string(),
            "JHN 3:16"
        );
    }

    #[test]
    fn overlap_detection() {
        let mk = |a, b| {
            VerseRange::new(
                VerseRef::new(BookId::Isa, 53, a, SchemeId::ORG),
                VerseRef::new(BookId::Isa, 53, b, SchemeId::ORG),
            )
            .unwrap()
        };
        assert!(mk(1, 5).overlaps(&mk(5, 9)));
        assert!(!mk(1, 4).overlaps(&mk(5, 9)));
    }
}
