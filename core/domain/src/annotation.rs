//! Annotation layers — the extension point for everything that marks up
//! scripture without being part of the translation's own text.
//!
//! # Why this is not a text style
//!
//! Words of Jesus are red because the *translation* says so: USFM marks them
//! `\wj`, the marker travels in the source file, and it lands in
//! [`crate::text::CharStyle::WordsOfJesus`].
//!
//! Messianic prophecy is different in kind. **No translation marks it.** There
//! is no USFM marker for it and no edition ships it. Which Old Testament
//! passages count as messianic prophecy is an *editorial and theological
//! judgement*, published as a separate work, with its own author and its own
//! copyright. Encoding it as a text style would mean baking one tradition's
//! interpretation irreversibly into the stored corpus, where a user could
//! neither disable it nor disagree with it, and where re-importing the text
//! would destroy it.
//!
//! So it is a **layer**: an independently licensed dataset of verse ranges,
//! resolved over the text at render time, that the user can switch off, swap,
//! or add to. The same mechanism serves commentaries, thematic tagging, reading
//! plans and cross-reference sets — the "plugin-shaped" features, defined once
//! here rather than special-cased four times later.
//!
//! # Accessibility
//!
//! Colour must never be the only cue (WCAG 1.4.1). This applies to red-letter
//! text just as much as to prophecy highlighting. Every layer therefore carries
//! a [`AnnotationLayer::spoken_label`], which is content and belongs in the
//! domain, while the colour itself is presentation and belongs in the theme.

use crate::provenance::License;
use crate::reference::{SchemeId, VerseRange};
use core::fmt;
use std::collections::BTreeMap;

/// Stable identifier for an annotation layer, e.g. `messianic-prophecy`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LayerId(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayerIdError {
    Empty,
    IllegalCharacter(char),
}

impl fmt::Display for LayerIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LayerIdError::Empty => write!(f, "layer id is empty"),
            LayerIdError::IllegalCharacter(c) => {
                write!(f, "layer id contains illegal character {c:?}")
            }
        }
    }
}

impl std::error::Error for LayerIdError {}

impl LayerId {
    pub fn new(s: impl Into<String>) -> Result<LayerId, LayerIdError> {
        let s = s.into();
        if s.is_empty() {
            return Err(LayerIdError::Empty);
        }
        if let Some(c) = s
            .chars()
            .find(|c| !(c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-'))
        {
            return Err(LayerIdError::IllegalCharacter(c));
        }
        Ok(LayerId(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for LayerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What a layer asserts about the ranges it covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerKind {
    /// Old Testament passages held to prophesy the Messiah.
    MessianicProphecy,
    /// Passages grouped by subject.
    Theme,
    /// Passages linked to other passages.
    CrossReference,
    /// Passages with attached exposition.
    Commentary,
}

/// An independently licensed dataset of annotations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotationLayer {
    pub id: LayerId,
    pub kind: LayerKind,
    /// Display name, e.g. "Messianic prophecy".
    pub name: String,
    /// Announced by screen readers when entering an annotated span, e.g.
    /// "messianic prophecy". Content, not presentation: this is what makes the
    /// layer perceivable without colour.
    pub spoken_label: String,
    /// Layers are separately licensed works. A prophecy list is an editorial
    /// compilation with its own copyright, not a fact in the public domain.
    pub license: License,
    /// Whether the layer is on by default. Interpretive layers default off, so
    /// the application does not assert a reading the user did not ask for.
    pub enabled_by_default: bool,
}

/// One annotated span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotation {
    pub layer: LayerId,
    /// Always stored in [`SchemeId::ORG`] (ADR-0004), for the same reason user
    /// bookmarks are: a layer must not rot when the user changes translation.
    pub range: VerseRange,
    pub payload: AnnotationPayload,
}

/// Layer-specific detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnnotationPayload {
    /// A prophecy, with the New Testament passages that cite it or claim its
    /// fulfilment.
    ///
    /// `fulfilled_in` is what makes the claim checkable rather than merely
    /// asserted: a highlight the user can follow to the text that grounds it.
    /// It also permits a defensible public-domain default dataset, built from
    /// New Testament citations of the Old rather than from a modern
    /// copyrighted compilation.
    MessianicProphecy {
        fulfilled_in: Vec<VerseRange>,
        note: Option<String>,
    },
    /// A subject label.
    Theme { label: String },
    /// A link to other passages.
    CrossReference { targets: Vec<VerseRange> },
    /// Attached prose, keyed by an identifier the commentary module resolves.
    Commentary { entry: String },
}

/// Why an annotation was rejected at load time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnnotationError {
    /// Annotations must be authored in `org`, per ADR-0004.
    NotCanonicalScheme { layer: LayerId, found: SchemeId },
    /// The payload does not match the layer's declared kind.
    PayloadKindMismatch { layer: LayerId, expected: LayerKind },
    /// A messianic-prophecy annotation with no fulfilment reference is an
    /// unfalsifiable assertion; the dataset must say what grounds it.
    ProphecyWithoutFulfilment { layer: LayerId, range: String },
}

impl fmt::Display for AnnotationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnnotationError::NotCanonicalScheme { layer, found } => write!(
                f,
                "layer {layer} has an annotation in scheme {found}; annotations must be authored in org"
            ),
            AnnotationError::PayloadKindMismatch { layer, expected } => {
                write!(f, "layer {layer} declares kind {expected:?} but carries a different payload")
            }
            AnnotationError::ProphecyWithoutFulfilment { layer, range } => write!(
                f,
                "layer {layer}: prophecy at {range} lists no fulfilment reference"
            ),
        }
    }
}

impl std::error::Error for AnnotationError {}

impl Annotation {
    /// Validates one annotation against its layer.
    pub fn validate(&self, layer: &AnnotationLayer) -> Result<(), AnnotationError> {
        if self.range.scheme() != SchemeId::ORG {
            return Err(AnnotationError::NotCanonicalScheme {
                layer: layer.id.clone(),
                found: self.range.scheme(),
            });
        }
        let kind_ok = matches!(
            (&self.payload, layer.kind),
            (
                AnnotationPayload::MessianicProphecy { .. },
                LayerKind::MessianicProphecy
            ) | (AnnotationPayload::Theme { .. }, LayerKind::Theme)
                | (
                    AnnotationPayload::CrossReference { .. },
                    LayerKind::CrossReference
                )
                | (AnnotationPayload::Commentary { .. }, LayerKind::Commentary)
        );
        if !kind_ok {
            return Err(AnnotationError::PayloadKindMismatch {
                layer: layer.id.clone(),
                expected: layer.kind,
            });
        }
        if let AnnotationPayload::MessianicProphecy { fulfilled_in, .. } = &self.payload {
            if fulfilled_in.is_empty() {
                return Err(AnnotationError::ProphecyWithoutFulfilment {
                    layer: layer.id.clone(),
                    range: self.range.to_string(),
                });
            }
        }
        Ok(())
    }
}

/// Which layers cover which verses of one chapter.
///
/// Built from annotations **already mapped into the display scheme**. Mapping
/// is the versification crate's job, and doing it here would drag a versifier
/// into the domain layer for no gain.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChapterOverlay {
    by_verse: BTreeMap<u16, Vec<LayerId>>,
}

impl ChapterOverlay {
    /// `chapter_verses` is the set of verse numbers actually present in the
    /// rendered chapter, so an annotation covering a verse this translation
    /// does not have contributes nothing rather than producing a dangling span.
    pub fn build<'a>(
        book: crate::book::BookId,
        chapter: u16,
        chapter_verses: &[u16],
        annotations: impl IntoIterator<Item = &'a Annotation>,
    ) -> ChapterOverlay {
        let mut by_verse: BTreeMap<u16, Vec<LayerId>> = BTreeMap::new();
        for a in annotations {
            for &v in chapter_verses {
                let r = crate::reference::VerseRef::new(book, chapter, v, a.range.scheme());
                if a.range.contains(r) {
                    let entry = by_verse.entry(v).or_default();
                    if !entry.contains(&a.layer) {
                        entry.push(a.layer.clone());
                    }
                }
            }
        }
        ChapterOverlay { by_verse }
    }

    /// Layers covering `verse`, in insertion order.
    pub fn layers_for(&self, verse: u16) -> &[LayerId] {
        self.by_verse.get(&verse).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn is_empty(&self) -> bool {
        self.by_verse.is_empty()
    }

    pub fn annotated_verses(&self) -> impl Iterator<Item = u16> + '_ {
        self.by_verse.keys().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::book::BookId;
    use crate::provenance::{License, SourceRef};
    use crate::reference::{VerseRange, VerseRef};

    fn src() -> SourceRef {
        SourceRef {
            url: "https://example.invalid/prophecies.json".into(),
            sha256: "0".repeat(64),
            publisher: "Public domain compilation".into(),
        }
    }

    fn prophecy_layer() -> AnnotationLayer {
        AnnotationLayer {
            id: LayerId::new("messianic-prophecy").unwrap(),
            kind: LayerKind::MessianicProphecy,
            name: "Messianic prophecy".into(),
            spoken_label: "messianic prophecy".into(),
            license: License::public_domain("Derived from New Testament citations", src()),
            enabled_by_default: false,
        }
    }

    fn org_range(b: BookId, c: u16, v1: u16, v2: u16) -> VerseRange {
        VerseRange::new(
            VerseRef::new(b, c, v1, SchemeId::ORG),
            VerseRef::new(b, c, v2, SchemeId::ORG),
        )
        .unwrap()
    }

    fn isaiah_53() -> Annotation {
        Annotation {
            layer: LayerId::new("messianic-prophecy").unwrap(),
            range: org_range(BookId::Isa, 53, 4, 6),
            payload: AnnotationPayload::MessianicProphecy {
                fulfilled_in: vec![org_range(BookId::Mat, 8, 17, 17)],
                note: Some("Cited at Matthew 8:17".into()),
            },
        }
    }

    #[test]
    fn layer_ids_are_filename_safe() {
        assert!(LayerId::new("messianic-prophecy").is_ok());
        assert_eq!(LayerId::new(""), Err(LayerIdError::Empty));
        assert_eq!(
            LayerId::new("Messianic Prophecy"),
            Err(LayerIdError::IllegalCharacter('M'))
        );
    }

    #[test]
    fn a_valid_prophecy_annotation_passes() {
        assert!(isaiah_53().validate(&prophecy_layer()).is_ok());
    }

    #[test]
    fn annotations_must_be_authored_in_the_canonical_scheme() {
        let mut a = isaiah_53();
        a.range = VerseRange::new(
            VerseRef::new(BookId::Isa, 53, 4, SchemeId::ENG),
            VerseRef::new(BookId::Isa, 53, 6, SchemeId::ENG),
        )
        .unwrap();
        assert!(matches!(
            a.validate(&prophecy_layer()).unwrap_err(),
            AnnotationError::NotCanonicalScheme { .. }
        ));
    }

    #[test]
    fn a_prophecy_must_say_what_fulfils_it() {
        let mut a = isaiah_53();
        a.payload = AnnotationPayload::MessianicProphecy {
            fulfilled_in: vec![],
            note: None,
        };
        assert!(matches!(
            a.validate(&prophecy_layer()).unwrap_err(),
            AnnotationError::ProphecyWithoutFulfilment { .. }
        ));
    }

    #[test]
    fn payload_must_match_the_layers_declared_kind() {
        let mut a = isaiah_53();
        a.payload = AnnotationPayload::Theme {
            label: "suffering".into(),
        };
        assert!(matches!(
            a.validate(&prophecy_layer()).unwrap_err(),
            AnnotationError::PayloadKindMismatch { .. }
        ));
    }

    #[test]
    fn overlay_covers_only_verses_the_translation_actually_has() {
        let a = isaiah_53();
        let present = [1u16, 2, 3, 4, 5, 6, 7];
        let overlay = ChapterOverlay::build(BookId::Isa, 53, &present, [&a]);
        assert_eq!(overlay.layers_for(3), &[] as &[LayerId]);
        assert_eq!(overlay.layers_for(4).len(), 1);
        assert_eq!(overlay.layers_for(6).len(), 1);
        assert_eq!(overlay.layers_for(7), &[] as &[LayerId]);
        assert_eq!(
            overlay.annotated_verses().collect::<Vec<_>>(),
            vec![4, 5, 6]
        );
    }

    #[test]
    fn an_annotation_outside_the_chapter_contributes_nothing() {
        let a = isaiah_53();
        let overlay = ChapterOverlay::build(BookId::Isa, 52, &[1, 2, 3], [&a]);
        assert!(overlay.is_empty());
    }

    #[test]
    fn overlapping_layers_stack_without_duplicating() {
        let a = isaiah_53();
        let b = Annotation {
            layer: LayerId::new("suffering-servant").unwrap(),
            range: org_range(BookId::Isa, 53, 5, 5),
            payload: AnnotationPayload::Theme {
                label: "atonement".into(),
            },
        };
        let overlay = ChapterOverlay::build(BookId::Isa, 53, &[4, 5, 6], [&a, &b, &a]);
        assert_eq!(overlay.layers_for(4).len(), 1);
        assert_eq!(overlay.layers_for(5).len(), 2);
    }

    #[test]
    fn interpretive_layers_are_off_by_default() {
        assert!(!prophecy_layer().enabled_by_default);
    }
}
