//! Versification: which chapters and verses a tradition recognises, and how to
//! translate a reference from one tradition's numbering into another's.
//!
//! ADR-0004 is the decision record. In short: every reference carries its
//! scheme, everything persisted is stored in `org`, and `org` is the pivot for
//! all conversion.
//!
//! This is the module the risk register ranks as the highest-consequence
//! correctness risk in the project: a mapping bug silently relocates a user's
//! bookmarks and notes, and there is no server to repair the damage
//! afterwards. It is therefore the most heavily tested module, with a
//! conformance suite driven by the real vendored data.

#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]

pub mod scheme;
pub mod versifier;

pub use scheme::{DefectReason, MappingDefect, PrunedMapping, Scheme, SchemeLoadError, VerseKey};
pub use versifier::{SchemeRegistry, VersificationError, Versifier};
