//! Boundary DTOs.
//!
//! These are the *only* types that cross into the UI. They are deliberately
//! separate from the domain model for the same reason the wire format is: the
//! shape the renderer consumes is a published contract, and a domain refactor
//! must not silently change it (ADR-0001).
//!
//! Everything the renderer needs to produce accessible output is computed
//! here, not left to the UI. Spoken verse labels in particular are *content*
//! and belong on this side of the boundary — a frontend that has to build them
//! by string-concatenating a book name is a frontend that will get it wrong in
//! the second language.

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChapterView {
    pub translation: TranslationView,
    pub book: BookView,
    pub chapter: u16,
    /// Spoken and displayed title, e.g. `"Psalms 23"`.
    pub title: String,
    pub blocks: Vec<BlockView>,
    pub notes: Vec<NoteView>,
    /// Verse numbers present, ascending.
    pub verses: Vec<u16>,
    pub nav: NavView,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranslationView {
    pub id: String,
    pub abbrev: String,
    pub name: String,
    /// BCP-47, emitted as the `lang` attribute.
    pub language: String,
    /// `"ltr"` or `"rtl"`, emitted as the `dir` attribute. Never inferred in
    /// the UI.
    pub direction: String,
    /// Must be surfaced wherever this translation is displayed (§4.3).
    pub attribution: String,
}

/// One annotation layer, as the layer picker sees it (ADR-0009).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LayerView {
    pub id: String,
    pub name: String,
    pub spoken_label: String,
    pub enabled_by_default: bool,
    /// Surfaced wherever the layer is offered, like translation attribution.
    pub attribution: String,
}

/// The table of contents for one translation: every book present and the
/// chapters each one has. One coarse call feeds the whole contents screen,
/// for the same reason `get_chapter` feeds the whole reading screen.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TocView {
    pub translation: TranslationView,
    /// Canonical order — the order they are shelved, not alphabetical.
    pub books: Vec<TocBookView>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TocBookView {
    /// USFM code, e.g. `"GEN"`.
    pub id: String,
    pub name: String,
    /// Chapter numbers present, ascending.
    pub chapters: Vec<u16>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BookView {
    /// USFM code, e.g. `"PSA"`.
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NavView {
    pub previous: Option<ChapterCursor>,
    pub next: Option<ChapterCursor>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChapterCursor {
    pub book: String,
    pub chapter: u16,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum BlockView {
    /// `style` is a stable token the stylesheet keys on: `p`, `q1`, `d`, …
    Paragraph {
        style: String,
        inlines: Vec<InlineView>,
    },
    /// `level` maps to `<h2>`/`<h3>`; real headings in document order.
    Heading {
        level: u8,
        style: String,
        inlines: Vec<InlineView>,
    },
    /// A stanza break. Semantic, not decorative.
    Blank,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum InlineView {
    Text {
        text: String,
    },
    /// Opens a verse. `label` is the full spoken reference for the
    /// visually-hidden span that precedes the number, so a screen reader says
    /// "Psalms 23 verse 1" instead of running the digits into the text.
    VerseStart {
        start: u16,
        end: u16,
        /// What is printed, e.g. `"1"` or `"19-27"`.
        number: String,
        label: String,
        /// Annotation layers covering this verse (ADR-0009), by id. The theme
        /// maps these to colours; `spokenLabels` carries the non-colour cue.
        layers: Vec<String>,
        spoken_layers: Vec<String>,
    },
    VerseEnd {
        start: u16,
        end: u16,
    },
    Span {
        /// `wj`, `w`, `nd`, `add`, …
        style: String,
        /// Present only on wordlist entries that carry one.
        #[serde(skip_serializing_if = "Option::is_none")]
        strong: Option<String>,
        inner: Vec<InlineView>,
    },
    Note {
        id: u32,
        caller: String,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NoteView {
    pub id: u32,
    /// `"footnote"` or `"crossReference"`.
    pub kind: String,
    pub caller: String,
    pub inlines: Vec<InlineView>,
    /// Flattened text, for the `aria-label` of the note marker.
    pub text: String,
}
