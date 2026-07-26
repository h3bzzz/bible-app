//! Persistence.
//!
//! Two databases, deliberately (ADR-0005):
//!
//! - **`corpus.db`** — read-only at runtime, produced by `tools/build-corpus`,
//!   replaceable wholesale. Text, versification tables, annotation layers.
//! - **`user.db`** — read-write and precious. Notes, highlights, bookmarks,
//!   layer preferences, and the sync oplog.
//!
//! Mixing them would mean a translation update could endanger a user's notes
//! and a notes restore could corrupt the corpus. They are attached separately
//! at runtime.
//!
//! This is the only crate permitted to contain SQL.

#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]

pub mod corpus;
pub mod migrations;
pub mod wire;

pub use corpus::{CorpusDb, StorageError};
pub use migrations::{current_version, migrate, Database, Migration, MigrationError};
pub use wire::{CodecError, DOC_FORMAT};
