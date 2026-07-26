//! The application layer: use cases and the boundary contract.
//!
//! Everything a shell can call lives here. Shells (Tauri desktop, Tauri
//! mobile, a wasm web build) contain wiring and no logic — §5's "no business
//! logic in UI components" is enforced by there being nothing in a shell for
//! logic to live in.

#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]

pub mod catalog;
pub mod get_chapter;
pub mod layers;
pub mod view;

pub use catalog::{list_translations, table_of_contents};
pub use layers::{
    activate, activate_for_translation, builtin_layers, layer_views, LayerError, LoadedLayer,
};
pub use get_chapter::{chapters_of, get_chapter, view_of, ActiveLayers, AppError};
pub use view::{
    BlockView, BookView, ChapterCursor, ChapterView, InlineView, NavView, NoteView, TocBookView,
    TocView, TranslationView,
};
