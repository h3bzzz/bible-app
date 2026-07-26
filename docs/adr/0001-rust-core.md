# ADR-0001 — The core engine is written in Rust

- **Status:** Accepted
- **Date:** 2026-07-24
- **Supersedes:** none

## Context

The core is the shared, UI-independent engine: canon model, versification, text
store, search, import, sync primitives. The workload is ~30 MB of text, SQLite
queries, an XML/USFM importer, and string normalization. There is **no CPU-bound
work**. Performance does not decide this.

Three candidates were evaluated: Rust, Zig, Go.

## Decision

**Rust.**

## Rationale

The operative reason is not the one usually given.

The usual argument is "Rust automates FFI binding generation via UniFFI, and FFI
maintenance across four platforms is the largest recurring cost." That argument
is **void under ADR-0002**: a Tauri shell talks to the core over Tauri's IPC
command layer (serde-serialized payloads over a channel), not a C ABI. There is
no Kotlin binding, no Swift binding, no header file. See ADR-0008.

The operative reason is: **ADR-0002 puts a Rust toolchain in this repository
whether or not the core is Rust.** Tauri's shell *is* Rust. Choosing Zig or Go
for the core means maintaining two systems toolchains in order to avoid writing
Rust, while still writing Rust in the shell. That is strictly worse than one
toolchain.

Secondary reasons:

- `rusqlite` with the `bundled` feature compiles the SQLite amalgamation with
  FTS5 enabled. No external toolchain, no system SQLite dependency, and the
  exact SQLite version is pinned by `Cargo.lock`.
- Mature libraries for the work this project actually does: `quick-xml`,
  `unicode-normalization`, `serde`, and CRDT crates (`automerge`, `loro`) for
  ADR-0006's later phase.
- UniFFI remains available as **insurance**, not as the plan. If Tauri mobile
  proves inadequate, a native Kotlin/Swift shell is an additive adapter over the
  existing coarse DTO boundary, not a rewrite.

## Alternatives rejected

**Zig.** The cleanest cross-compilation story of the three and the most direct
SQLite integration (`@cImport` of the amalgamation). Rejected because it is
pre-1.0 and the standard library breaks between minor releases. Against a solo
maintainer with a five-year horizon, that is a migration pass every release,
forever, in exchange for benefits (small binaries, fast compiles) this project
does not need. Additionally it would be a second toolchain alongside Tauri's
Rust.

**Go.** The most stable language of the three and the most pleasant to maintain
in the large. Rejected because this project lives at its boundaries and Go is
weakest there: `gomobile bind` is minimally maintained with a severely
restricted export surface; cgo SQLite (`mattn/go-sqlite3`) conflicts with
cross-compilation; `modernc.org/sqlite` is a machine translation of SQLite that
would sit under the entire product's data layer; TinyGo's `reflect` gaps make
the wasm target painful. Go's strengths — goroutines, GC, servers — are
strengths this project does not need. Also a second toolchain.

## Consequences

Accepted costs, stated plainly:

1. **Slowest iteration loop of the three, permanently.** Mitigated by splitting
   `core/` into five small crates so incremental recompiles stay local, by using
   `cargo check` for the inner loop, and by a CI performance budget on
   incremental build time.
2. **Borrow-checker tax with minimal payoff.** This application has one writer,
   no shared mutable state, and no concurrency beyond a background import task.
   We pay Rust's learning cost for guarantees a single-threaded design would
   have given anyway. This is a one-time cost against a multi-year benefit.
3. **`unsafe` must not spread.** Every crate under `core/` carries
   `#![forbid(unsafe_code)]`. If a future C ABI need arises, it gets its own
   crate that is allowed `unsafe` and contains nothing else.

## Binding constraint on the core

Boundary calls are **coarse**. `get_chapter(translation, book, chapter)` returns
an entire chapter as one serialized payload. There is no `get_verse`, no
`get_verse_count`, no per-verse call. One round trip per screen of content.

This rule is what makes the boundary survivable if it is ever swapped from
Tauri IPC to a real C ABI. Every chatty crossing designed in is a bug paid for
on four platforms.
