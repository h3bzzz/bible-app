# ADR-0003 — The chapter is the storage unit; verses are milestones inside it

- **Status:** Accepted
- **Date:** 2026-07-24

## Context

Verse text is structured, not a string. It carries paragraph and poetry line
breaks, section headings, words-of-Jesus marking, footnotes, inline
cross-references, italics for translator-supplied words, small-caps divine
names, and — in the WEB edition this project ships — per-word Strong's numbers.

If the storage schema holds plain strings, that information is destroyed
permanently at import time. Re-importing later cannot recover it if the source
archive is gone, and re-importing is expensive besides.

The obvious schema is one row per verse. It is wrong.

## Decision

**One stored document per chapter.** `ChapterDoc` is a flat sequence of `Block`s;
each `Block` holds a tree of `Inline`s; **verse boundaries are zero-width
milestones in the inline stream**, not containers.

A separate flattened `verse_plain` projection is derived at import time for
full-text search and for cheap verse-level lookups (cross-reference previews).
It is a *derived index*, never the source of truth.

## Rationale — from the actual corpus, not from theory

Paragraphs, poetry stanzas, and section headings **span verse boundaries**.
Two examples taken verbatim from the vendored WEB USFM:

**Psalm 23:4** begins in one poetry line and continues across two more:

```
\q1
\v 4 Even though I walk through the valley of the shadow of death,
\q2   I will fear no evil, for you are with me.
\q1 Your rod and your staff,
\q2   they comfort me.
```

Verse 4 opens inside a `\q1` block and its text continues through a `\q2`, a
second `\q1`, and a second `\q2`. There is no verse-shaped container to put this
in.

**John 1:21** starts a new paragraph in the middle of the verse:

```
\v 21 They asked him, "What then? Are you Elijah?"
\p He said, "I am not."
```

A per-verse schema must either duplicate the block wrapper on every verse
fragment (producing wrong rendering — a stanza break where there is none) or
drop it (producing prose where there is poetry). Both are data loss. USX models
this the same way for the same reason.

## Consequences

1. **Reading one verse requires decoding its chapter.** Accepted: chapters are
   small (Psalm 23 is under 4 KB encoded) and the reading UI displays whole
   chapters anyway. The `verse_plain` projection covers the cases that genuinely
   need a single verse without its context.
2. **Rendering is a stateful walk**, not a loop over rows. The renderer tracks
   the current verse as it encounters `VerseStart`/`VerseEnd` milestones. This is
   the cost of representing overlapping hierarchies in a flat sequence, and it
   is paid once in one module.
3. **Highlights anchor to (verse, char offset) into the flattened projection**,
   not into the document tree. Tree paths would break on any re-import; character
   offsets into normalized verse text survive.
4. `chapter_doc.doc_format` is an integer version on the encoding. A change to
   `ChapterDoc`'s shape bumps it and triggers re-import, rather than silently
   mis-decoding.

## Encoding

CBOR, stored as a `BLOB`. Chosen over JSON: roughly 40% smaller across the
corpus, faster to decode, and the shape is owned by our own types rather than by
anything that needs to be human-readable. The corpus database is a build
artifact; it is not edited by hand.

## Forward hooks accepted now

`CharStyle::W { strong, lemma }` exists from the first commit. This is not
speculative: the vendored WEB edition carries `\w word|strong="H3068"\w*` on
nearly every word, so the hook is exercised by real data on day one. Discarding
it at import would make any future interlinear or Strong's feature require
re-importing the entire corpus.
