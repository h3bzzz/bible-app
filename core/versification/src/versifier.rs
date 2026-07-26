//! Conversion between versification schemes, pivoting through `org`.

use crate::scheme::{MappingDefect, PrunedMapping, Scheme, SchemeLoadError};
use bible_domain::{SchemeId, VerseRef};
use std::collections::HashMap;
use std::fmt;

/// The six schemes vendored under `data/versification/`.
///
/// Embedded in the binary rather than read from disk at runtime: requirement
/// §1 is that the application works with zero network access and no external
/// data directory, and versification is not optional to reading a verse.
const EMBEDDED: &[(&str, &str)] = &[
    ("org", include_str!("../../../data/versification/org.json")),
    ("eng", include_str!("../../../data/versification/eng.json")),
    ("lxx", include_str!("../../../data/versification/lxx.json")),
    ("vul", include_str!("../../../data/versification/vul.json")),
    ("rso", include_str!("../../../data/versification/rso.json")),
    ("rsc", include_str!("../../../data/versification/rsc.json")),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersificationError {
    UnknownScheme(SchemeId),
}

impl fmt::Display for VersificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VersificationError::UnknownScheme(s) => write!(f, "unknown versification scheme {s}"),
        }
    }
}

impl std::error::Error for VersificationError {}

/// Maps references between versification schemes.
///
/// A trait rather than a bare struct because the conformance suite drives a
/// fixture-backed implementation — a named second implementation, which is the
/// bar the architecture sets for introducing an interface.
pub trait Versifier {
    /// Express `r` in scheme `to`.
    ///
    /// Returns:
    /// - one reference for the ordinary case,
    /// - **several** where the target scheme splits the verse,
    /// - **none** where the verse does not exist in the target scheme.
    ///
    /// Absence is a real answer, not an error: 3 John 15 genuinely does not
    /// exist in schemes that merge it into verse 14. Callers that assume a
    /// single result are wrong by construction rather than at runtime.
    fn map(&self, r: VerseRef, to: SchemeId) -> Result<Vec<VerseRef>, VersificationError>;

    /// Whether the position exists in its own scheme.
    fn contains(&self, r: VerseRef) -> Result<bool, VersificationError>;

    /// Last verse of a chapter in the given scheme.
    fn last_verse(
        &self,
        scheme: SchemeId,
        book: bible_domain::BookId,
        chapter: u16,
    ) -> Result<Option<u16>, VersificationError>;
}

/// All loaded schemes.
#[derive(Debug, Clone)]
pub struct SchemeRegistry {
    schemes: HashMap<SchemeId, Scheme>,
    defects: Vec<MappingDefect>,
    pruned: Vec<PrunedMapping>,
}

impl SchemeRegistry {
    /// Loads the six embedded schemes.
    ///
    /// Defects in the upstream data are collected, not fatal — see
    /// [`SchemeRegistry::defects`].
    pub fn embedded() -> Result<SchemeRegistry, SchemeLoadError> {
        let mut schemes = HashMap::new();
        let mut defects = Vec::new();
        for (id, json) in EMBEDDED {
            let (scheme, mut d) = Scheme::from_json(id, json)?;
            defects.append(&mut d);
            schemes.insert(scheme.id(), scheme);
        }

        // Cross-scheme validation, which no single scheme can perform alone:
        // drop mappings that point at positions the pivot does not contain.
        // `org` is pruned against a snapshot of itself so its own S3Y -> DAG
        // redirects are evaluated on the same footing as every other scheme's.
        let org = schemes
            .get(&SchemeId::ORG)
            .ok_or_else(|| SchemeLoadError::BadSchemeId("org".into()))?
            .clone();
        let mut pruned = Vec::new();
        for scheme in schemes.values_mut() {
            pruned.append(&mut scheme.prune_mappings_absent_from_org(&org));
        }

        let key =
            |s: &SchemeId, a: &str, b: &str| (s.as_str().to_string(), a.to_string(), b.to_string());
        defects.sort_by_key(|d| key(&d.scheme, &d.from, &d.to));
        pruned.sort_by_key(|p| key(&p.scheme, &p.from, &p.to));
        Ok(SchemeRegistry {
            schemes,
            defects,
            pruned,
        })
    }

    /// Mappings dropped as pointing outside `org`.
    ///
    /// Reported separately from [`SchemeRegistry::defects`] because these are
    /// well-formed data, not errors — see [`PrunedMapping`].
    pub fn pruned_mappings(&self) -> &[PrunedMapping] {
        &self.pruned
    }

    /// Defects found while loading, in a stable order.
    ///
    /// `build-corpus` refuses in strict mode to bundle a translation whose
    /// scheme has any, so a data defect surfaces as a build failure for the
    /// schemes that matter while remaining merely reported for those that do
    /// not.
    pub fn defects(&self) -> &[MappingDefect] {
        &self.defects
    }

    pub fn defects_for(&self, scheme: SchemeId) -> impl Iterator<Item = &MappingDefect> {
        self.defects.iter().filter(move |d| d.scheme == scheme)
    }

    pub fn scheme(&self, id: SchemeId) -> Result<&Scheme, VersificationError> {
        self.schemes
            .get(&id)
            .ok_or(VersificationError::UnknownScheme(id))
    }

    pub fn scheme_ids(&self) -> Vec<SchemeId> {
        let mut v: Vec<SchemeId> = self.schemes.keys().copied().collect();
        v.sort();
        v
    }
}

impl Versifier for SchemeRegistry {
    fn map(&self, r: VerseRef, to: SchemeId) -> Result<Vec<VerseRef>, VersificationError> {
        let target = self.scheme(to)?;

        if r.scheme == to {
            return Ok(if target.contains(r) { vec![r] } else { vec![] });
        }

        let source = self.scheme(r.scheme)?;
        let org_refs = source.to_org(r);

        let mut out: Vec<VerseRef> = Vec::new();
        for o in org_refs {
            for candidate in target.from_org(o) {
                // A mapping may point at a position the target scheme does not
                // have. Dropping it here is what makes "absent" a real answer
                // instead of a dangling reference.
                if target.contains(candidate) && !out.contains(&candidate) {
                    out.push(candidate);
                }
            }
        }
        out.sort();
        Ok(out)
    }

    fn contains(&self, r: VerseRef) -> Result<bool, VersificationError> {
        Ok(self.scheme(r.scheme)?.contains(r))
    }

    fn last_verse(
        &self,
        scheme: SchemeId,
        book: bible_domain::BookId,
        chapter: u16,
    ) -> Result<Option<u16>, VersificationError> {
        Ok(self.scheme(scheme)?.last_verse(book, chapter))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bible_domain::BookId;

    fn reg() -> SchemeRegistry {
        SchemeRegistry::embedded().expect("embedded schemes must load")
    }

    #[test]
    fn all_six_schemes_load() {
        let r = reg();
        assert_eq!(r.scheme_ids().len(), 6);
        for id in [
            SchemeId::ORG,
            SchemeId::ENG,
            SchemeId::LXX,
            SchemeId::VUL,
            SchemeId::RSO,
            SchemeId::RSC,
        ] {
            assert!(r.scheme(id).is_ok(), "{id} missing");
        }
    }

    /// The two schemes the v1 corpus actually uses must be defect-free.
    ///
    /// This is the assertion that matters: a defect in `eng` or `org` would
    /// mean the shipping product can mis-map a reference.
    #[test]
    fn the_schemes_v1_ships_are_defect_free() {
        let r = reg();
        for scheme in [SchemeId::ORG, SchemeId::ENG] {
            let d: Vec<_> = r.defects_for(scheme).collect();
            assert!(d.is_empty(), "defects in shipping scheme {scheme}: {d:#?}");
        }
    }

    /// Pins the exact inventory of known upstream data defects.
    ///
    /// Two entries in the Copenhagen Alliance files cannot be interpreted:
    ///
    /// - `vul`: `DAG 3:52-23` is an inverted range, almost certainly a typo for
    ///   `3:52-53`.
    /// - `rso`: `PSA 89:2-6` -> `PSA 90:1-6` claims five source verses
    ///   correspond to six target verses, which no shift, merge or split
    ///   reconciles. It overlaps the adjacent `PSA 89:0-1` -> `PSA 90:0`
    ///   mapping, so the intent is ambiguous.
    ///
    /// Both are dropped rather than guessed at. If this count changes, either
    /// upstream corrected the data or the loader regressed — both worth
    /// noticing loudly.
    #[test]
    fn known_upstream_defects_are_exactly_these_two() {
        let r = reg();
        let all = r.defects();
        assert_eq!(all.len(), 2, "defect inventory changed: {all:#?}");

        let vul: Vec<_> = r.defects_for(SchemeId::VUL).collect();
        assert_eq!(vul.len(), 1);
        assert_eq!(vul[0].reason, crate::DefectReason::InvertedRange);
        assert_eq!(vul[0].from, "DAG 3:52-23");

        let rso: Vec<_> = r.defects_for(SchemeId::RSO).collect();
        assert_eq!(rso.len(), 1);
        assert_eq!(
            rso[0].reason,
            crate::DefectReason::IrreconcilableCounts {
                from_len: 5,
                to_len: 6
            }
        );
        assert_eq!(rso[0].from, "PSA 89:2-6");

        for scheme in [SchemeId::LXX, SchemeId::RSC] {
            assert_eq!(r.defects_for(scheme).count(), 0, "{scheme}");
        }
    }

    #[test]
    fn same_scheme_is_identity_for_verses_that_exist() {
        let r = reg();
        let v = VerseRef::new(BookId::Jhn, 3, 16, SchemeId::ENG);
        assert_eq!(r.map(v, SchemeId::ENG).unwrap(), vec![v]);
    }

    #[test]
    fn same_scheme_yields_nothing_for_verses_that_do_not_exist() {
        let r = reg();
        let v = VerseRef::new(BookId::Jhn, 3, 999, SchemeId::ENG);
        assert_eq!(r.map(v, SchemeId::ENG).unwrap(), vec![]);
    }

    #[test]
    fn unknown_scheme_is_an_error() {
        let r = reg();
        let bogus = SchemeId::new("nope").unwrap();
        let v = VerseRef::new(BookId::Gen, 1, 1, SchemeId::ENG);
        assert_eq!(
            r.map(v, bogus),
            Err(VersificationError::UnknownScheme(bogus))
        );
    }
}
