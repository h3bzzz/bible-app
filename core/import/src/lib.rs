//! Importing scripture from publisher source formats into [`bible_domain`]
//! types.
//!
//! USFM is the primary path, because it is what the publishers actually
//! distribute: eBible.org ships the World English Bible as USFM, and requirement
//! §1 — that the project remain buildable if every third-party service
//! disappears — means importing from a vendored archive of the real source
//! format rather than calling an API.
//!
//! This crate is build-time weight. It is depended on by `tools/build-corpus`,
//! not by the application, so a reader binary does not link a parser it will
//! never run.

#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]

pub mod tokenizer;
pub mod usfm;

pub use tokenizer::{tokenize, Marker, Token};
pub use usfm::{parse_book, BookMeta, ImportError, ImportedBook};
