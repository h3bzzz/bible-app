//! The domain layer: canon, references, versification identity, structured
//! text, translations, provenance, and annotation layers.
//!
//! This crate has **no dependencies**. Not "few" — none. Serialization lives at
//! the boundary in `bible-app-core`; persistence lives in `bible-storage`.
//! The architectural rule "no SQL outside infrastructure" is therefore enforced
//! by the compiler rather than by code review: this crate cannot contain SQL
//! because it cannot see a database.

#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]

pub mod annotation;
pub mod book;
pub mod provenance;
pub mod reference;
pub mod text;
pub mod translation;

pub use annotation::{
    Annotation, AnnotationLayer, AnnotationPayload, ChapterOverlay, LayerId, LayerKind,
};
pub use book::{BookId, Canon};
pub use provenance::{BundleRefusal, License, SourceRef};
pub use reference::{RangeError, SchemeId, Subverse, VerseRange, VerseRef};
pub use text::{
    Block, ChapterDoc, CharStyle, HeadingStyle, Inline, Note, NoteId, NoteKind, ParaStyle,
    StrongsLang, StrongsRef, VerseSpan,
};
pub use translation::{LangTag, TextDirection, Translation, TranslationId};
