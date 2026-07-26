# ADR-0002 — UI is a Tauri 2 shell hosting a single Svelte 5 frontend

- **Status:** Accepted
- **Date:** 2026-07-24

## Context

The product renders multilingual scripture. Accessibility is a stated hard
requirement, not a nice-to-have: screen readers, dynamic type, contrast, and
keyboard navigation must work on every target. Right-to-left Hebrew and Arabic
must lay out correctly. There is one maintainer and six targets.

## Decision

**Tauri 2 shell + one Svelte 5 frontend, shared by every target.**

## Rationale

The hardest requirements — bidirectional text (UAX#9), complex-script shaping,
user font scaling, and screen-reader semantics — are satisfied **by not writing
code**. A webview supplies all four, hardened over decades. Every alternative
asks us to re-implement some part of that.

Long-form text rendering (justification, footnote markers, hanging punctuation,
poetry indentation) is the single thing browsers are best in the world at, and
it is the entire visual surface of this product.

Svelte 5 over SolidJS is a narrow call, decided on ecosystem size, documentation
quality for a solo maintainer, and `svelte-check` typing. Compiled output with
no VDOM runtime suits a document reader. Either would have been defensible; this
ADR exists so the question is not reopened.

## Alternatives rejected

**Gio (pure-Go GPU UI).** Genuinely one codebase to all six targets and very
lightweight. Rejected on accessibility: support is thin to absent. For an
application whose purpose is rendering multilingual scripture accessibly, that
is disqualifying regardless of binary size. It would also require owning UAX#9
bidi and HarfBuzz shaping ourselves.

**Native UI per platform (Compose / SwiftUI / GTK / web).** Highest quality
ceiling and the best accessibility available. Rejected as roughly 4× the UI work
for one maintainer. Not survivable.

**Compose Multiplatform / Flutter with a Kotlin or Dart core.** Rejected on the
merits, not on brief-conformance. Both delete the FFI boundary entirely and both
have real accessibility. Compose loses on second-class Linux desktop support;
Flutter loses on Dart's ecosystem for USFM/Unicode work and on a ~15 MB binary
floor. Both drag a managed runtime and a Skia renderer into a product whose
selling point is being small, offline, and buildable forever. Neither is a
mistake; Tauri is better *for this product*.

## Consequences

1. **Four different webview engines.** WebKitGTK (Linux), WebView2 (Windows),
   WKWebView (macOS/iOS), Android WebView. They will disagree.
   **Mitigation:** the CSS support floor below; no experimental CSS; per-platform
   visual-regression goldens in CI; a standing ~15% UI time budget for this.

2. **Linux accessibility is the weak link.** WebKitGTK's AT-SPI bridge is the
   least-exercised of the four.
   **Mitigation:** semantic HTML and ARIA landmarks from the first component —
   free if done from the start, expensive to retrofit. Manual Orca pass per
   release. The README states the reference accessibility platforms honestly
   (Windows, macOS, Android) rather than implying parity.

3. **iOS App Store policy risk** for webview-shell applications.
   **Mitigation:** the application bundles all content and executes no remotely
   loaded code, which is the line Apple actually polices. Noted in the risk
   register. Contingency is a native SwiftUI shell over UniFFI (ADR-0008).

4. **`contenteditable` is a known misery zone.** v1 notes are a plain
   `<textarea>`. Rich text is a later, deliberate decision.

## CSS support floor

Binding on all UI code. A feature outside this list requires amending this ADR.

- Layout: flexbox, CSS grid, logical properties (`margin-inline-start`, never
  `margin-left`) — logical properties are what make RTL free.
- Custom properties for all theming. No hard-coded colors in components.
- `prefers-color-scheme`, `prefers-reduced-motion`, `prefers-contrast`.
- `rem` units throughout, rooted on the user's font size. No `px` font sizes.
- **Not permitted:** container queries, `:has()` at load-bearing positions,
  subgrid, `text-wrap: balance` as anything but progressive enhancement.

## Accessibility floor

Binding on all UI code.

- Verse numbers are `<sup aria-hidden="true">` with an adjacent visually-hidden
  `<span>` giving the full spoken reference. A screen reader must not read
  "twenty-three one Yahweh is my shepherd" as one run-on.
- Every chapter root carries `lang` and `dir` derived from the translation's
  BCP-47 tag. Never assumed.
- Poetry indentation is CSS `padding-inline-start`, never `&nbsp;` or `&emsp;`.
  Indentation must not be spoken.
- Section headings are real `<h2>`/`<h3>` elements in document order.
- All interactive elements reachable and operable by keyboard, with a visible
  focus ring that is not removed.
