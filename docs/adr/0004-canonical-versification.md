# ADR-0004 — All persisted references are stored in the `org` versification scheme

- **Status:** Accepted
- **Date:** 2026-07-24

## Context

Chapter and verse numbering is not universal. Psalm titles shift numbering
between the Hebrew and English traditions; Malachi has three chapters in the
Hebrew tradition and four in the English; deuterocanonical books exist in some
canons and not others; some verses are merged or split between traditions.

A `struct { book, chapter, verse }` with no scheme attached is a latent
data-corruption bug across every feature that stores a reference — bookmarks,
notes, highlights, reading plans, cross-references, and sync.

## Decision

1. **`VerseRef` always carries its `SchemeId`.** There is no constructor that
   omits it.
2. **Every reference persisted in `user.db` is stored in `org`** (the
   Paratext/SIL *Original* versification), with a SQL `CHECK` constraint
   enforcing it. The translation the user was reading is recorded separately as
   provenance only.
3. **`org` is the pivot.** Converting between two non-original schemes is
   `a → org → b`. The scheme data files are structured this way already: each
   scheme's `mappedVerses` maps *that scheme's* references *to* `org`.
4. Display converts `org → translation.scheme` at render time.

## Rationale

Storing references in the scheme of whatever translation was open means a
bookmark's meaning depends on an application setting at the time it was created.
Change your default translation and your highlights move. Add a translation in
2031 using a scheme nobody anticipated and you need a data migration over user
content.

Under this decision, a new scheme costs one data file and zero migrations.

## Data source

**Copenhagen Alliance `versification-specification` JSON**, vendored at
`data/versification/`, rather than raw Paratext `.vrs` text files.

This is an amendment to the original design, which specified `.vrs` parsing. The
`.vrs` files are not published at a stable, fetchable URL; the Copenhagen
Alliance repository publishes the same Paratext-derived data as versioned,
machine-readable JSON with the four fields we need — `maxVerses`,
`mappedVerses`, `excludedVerses`, `partialVerses`. Six schemes are vendored:
`org`, `eng`, `lxx`, `vul`, `rso`, `rsc`.

## Corrections forced by the real data

Three claims in the original design document were checked against
`data/versification/*.json` and found wrong. They are corrected here so they are
not repeated.

1. **Psalm 23 does not shift.** Both schemes give it six verses; its `\d`
   superscription is unnumbered in each. The real Psalms example is **Psalm
   51** — 21 verses in `org`, 19 in `eng`, because the Hebrew superscription
   occupies verses 1–2. Psalm 23 is retained as a *rendering* fixture (`\d`
   titles, `\q1`/`\q2` poetry) but not as a versification fixture.
2. **The Genesis mapping direction was backwards.** It is `eng` GEN 31:55 →
   `org` GEN 32:1, not the reverse: English Genesis 31 has 55 verses, Hebrew
   Genesis 31 has 54.
3. **Romans 16:25–27 is not a versification difference at all.** `eng` and `org`
   agree on all 27 verses. It is a question of manuscript placement, not verse
   numbering, and encoding it as a mapping would be wrong. The conformance
   suite now asserts the *absence* of a mapping, so a future "fix" cannot
   invent one.

## Two structural findings

**Verse 0 is real.** The Russian schemes number Hebrew superscriptions as verse
0 (`PSA 141:0`). Any `verse >= 1` assumption silently drops them.

**Aliasing is real, and the inverse mapping must account for it.** `eng`
addresses the Letter of Jeremiah both as its own book `LJE 1` and as `BAR 6`,
mapping the latter onto the former. So `org LJE 1:7` has *two* valid English
addresses. The inverse of `to_org` is therefore defined properly — `r` is an
image of `o` exactly when `r`'s own forward mapping lands on `o` — rather than
by reading a precomputed reverse index. Assuming the identity is always valid
breaks Genesis; assuming it never is breaks the Letter of Jeremiah.

## Defects versus prunings

The loader distinguishes two things that a single "errors" list would conflate:

- **Defects** — malformed data. Exactly two exist upstream: `vul`'s
  `DAG 3:52-23` (an inverted range, almost certainly a typo for `3:52-53`) and
  `rso`'s `PSA 89:2-6 → PSA 90:1-6` (five source verses, six targets, which no
  shift, merge or split reconciles). Both are dropped rather than guessed at.
  **`org` and `eng`, the two schemes v1 ships, have zero defects.**
- **Prunings** — well-formed mappings pointing at a representation `org` does
  not use. Several schemes redirect `S3Y` (Song of the Three) onto `DAG 3`
  (Greek Daniel), but `org` lists no `DAG` at all. Followed literally, the Song
  of the Three would have no canonical address and so could never be
  bookmarked. These mappings are dropped and the positional identity stands —
  `org` gives `S3Y` 67 verses.

Keeping them separate matters: burying two real typos under a hundred routine
normalisations makes the defect list useless.

## Conformance suite

The versification crate is the one place with a trait (`Versifier`), justified
by a named second consumer: the fixture-driven double used by the conformance
suite. Cases, all verified against the vendored data:

| Case | Assertion |
|---|---|
| Psalm 51 superscription | `eng` 51:1 ↔ `org` 51:3; `eng` 51:19 ↔ `org` 51:21 |
| Psalm 23 | no shift; six verses in both — guards correction 1 |
| Genesis chapter boundary | `eng` GEN 31:55 ↔ `org` GEN 32:1; whole of GEN 32 shifts by one |
| Malachi chapter split | `eng` MAL 4:1 ↔ `org` MAL 3:19; `eng` has 4 chapters, `org` 3 |
| Joel chapter split | `eng` JOL 2:28 ↔ `org` JOL 3:1; `eng` has 3 chapters, `org` 4 |
| 3 John | 15 verses in both; no mapping — guards against inventing one |
| Romans 16 | no mapping — guards correction 3 |
| Letter of Jeremiah | `eng` BAR 6:7 ↔ `org` LJE 1:7 (cross-book); both English addresses resolve |
| Song of the Three | stays addressable in `org` after pruning |
| Verse zero | `rsc` PSA 141:0 exists and maps |
| Absence | a verse beyond the target scheme maps to `[]`, not an error |
| **Round-trip property** | every verse of every scheme survives `scheme → org → scheme`; >100,000 references exercised |
| **Containment property** | no mapping ever yields a position the target scheme lacks; >100,000 checked |

## Consequences

1. Every read path that displays a reference pays one mapping lookup. Measured
   at microseconds against an in-memory index; not a performance concern.
2. A reference that does not exist in the target scheme maps to **empty**, not to
   an error and not to a nearby guess. Callers must handle absence. This is
   correct: 3 John 15 genuinely does not exist in schemes that merge it into 14.
3. Splits map one reference to **several**. `Versifier::map` returns `Vec`, and
   callers that assume a single result are wrong by construction rather than
   wrong at runtime.
