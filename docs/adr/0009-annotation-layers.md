# ADR-0009 — Interpretive markup is an annotation layer, not a text style

- **Status:** Accepted
- **Date:** 2026-07-24
- **Prompted by:** the requirement that Old Testament messianic prophecies be
  highlighted in their own colour, as words of Jesus are highlighted in red.

## Context

The request reads as symmetrical with red-letter text. It is not, and building
it that way would be a mistake that could not be undone later.

**Words of Jesus are part of the translation.** USFM marks them `\wj`; the
marker is authored by the translation committee, travels in the source file, and
is present in 2,290 places in the vendored WEB corpus. It lands in
`CharStyle::WordsOfJesus` and is stored in the chapter document because the
publisher put it there.

**Messianic prophecy is not part of any translation.** There is no USFM marker
for it. No edition ships it. Which Old Testament passages count as messianic
prophecy is an editorial and theological judgement — one that Jewish and
Christian readers answer differently, that Christian traditions answer
differently from one another, and that is published as a separate work with its
own author and its own copyright.

Storing it as a character style in `chapter_doc` would therefore:

1. **Bake one tradition's interpretation into the corpus**, where the user can
   neither disable it nor disagree with it.
2. **Destroy it on re-import.** The text comes from USFM; anything not in the
   USFM is lost the next time the importer runs.
3. **Mislicense it.** A prophecy list is not public domain merely because the
   scripture it points at is.
4. **Make it unattributable.** The user cannot see whose judgement they are
   reading.

## Decision

Interpretive markup is an **annotation layer**: an independently licensed
dataset of verse ranges, stored separately from the text, resolved over it at
render time, and switchable by the user.

This is also the extension point the five-year feature list needs. Commentaries,
thematic tagging, cross-reference sets and study notes are all the same shape —
"someone's assertion about a span of verses." Defining the mechanism once here
is why none of them will need a schema change later.

Concretely:

- `AnnotationLayer` — id, kind, display name, **spoken label**, licence,
  default-enabled flag.
- `Annotation` — layer id, a `VerseRange`, and a kind-specific payload.
- `ChapterOverlay` — built at render time by intersecting a chapter's actual
  verses with the active layers' ranges.

## Consequences and rules

**Ranges are stored in `org`,** exactly as user bookmarks are (ADR-0004),
enforced by validation. A prophecy list keyed to English versification would
mis-highlight in a translation using Hebrew numbering — Joel 2:28 and Joel 3:1
are the same verse, and a layer that did not know it would silently highlight
the wrong text.

**A messianic-prophecy annotation must name what fulfils it.** The payload
carries `fulfilled_in: Vec<VerseRange>`, and validation rejects an empty list.
This turns a bare assertion into a claim the reader can follow to the New
Testament text that grounds it, and it makes a defensible public-domain default
dataset possible: one built from **New Testament citations of the Old** — a
matter of textual record — rather than from a modern copyrighted compilation.

**Interpretive layers default to off.** `enabled_by_default` is false for
`MessianicProphecy`. The application should not assert a contested reading the
user did not ask for. The user turns it on; it is then persistent.

**Colour is never the only cue.** WCAG 1.4.1 — and this applies to red-letter
text just as much as to prophecy highlighting, so ADR-0002's accessibility floor
is extended:

- every layer carries a `spoken_label`, which is *content* and lives in the
  domain, while the colour is *presentation* and lives in the theme;
- an annotated span is announced on entry and exit by a screen reader;
- a non-colour cue (underline style, marginal marker) accompanies the colour;
- the two default layers must remain distinguishable in both light and dark
  themes and under the common colour-vision deficiencies. Red for words of
  Jesus and red-adjacent for prophecy would fail this; the prophecy layer's
  default is therefore chosen away from red.

**Layers are licensed separately and surfaced in the UI**, through the same
`License` type and the same `build-corpus` bundling gate as translations.

## Scope

The mechanism, the domain types and their validation land now, because
retrofitting them is precisely what ADR-0003 and ADR-0004 exist to prevent.

**The prophecy dataset itself is slice 3**, after the reader works and search
exists. Sourcing it is a research task with a licensing dimension, not a coding
task, and it must not block a chapter appearing on screen.

## Alternatives rejected

**A `CharStyle::Prophecy` variant.** Simplest to render and wrong for all four
reasons above. Rejected.

**Hard-coding a prophecy list in the binary.** Not user-switchable, not
attributable, not replaceable, and it makes the application the author of a
theological claim. Rejected.

**Deriving prophecy spans from a cross-reference dataset at runtime.** Attractive
because cross-references are widely available and public domain, but the
inference "the New Testament cites this verse" is not the same claim as "this
verse is a messianic prophecy," and conflating them would produce both false
positives and false negatives. The datasets stay distinct; a cross-reference
layer may *inform* the prophecy layer's construction, offline, with a human
deciding.
