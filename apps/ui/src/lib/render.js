/**
 * Renders a ChapterView (the boundary DTO from `bible-app-core`) to HTML.
 *
 * All rendering logic lives here rather than in the Svelte component so that
 * the accessibility rules in ADR-0002 and ADR-0009 are unit-testable. A rule
 * that only exists inside a component is a rule nobody checks.
 *
 * The invariants this module exists to enforce:
 *
 *  - Verse numbers are `aria-hidden`, with an adjacent visually-hidden span
 *    carrying the full spoken reference. A screen reader must say
 *    "Psalms 23 verse 1. Yahweh is my shepherd", never "23 1 Yahweh".
 *  - Poetry indentation is a CSS class, never `&nbsp;` or `&emsp;`. Whitespace
 *    used as indentation gets spoken.
 *  - Headings are real `<h2>`/`<h3>` in document order, never styled divs.
 *  - `lang` and `dir` come from the translation and are never inferred.
 *  - Colour is never the only cue: words of Jesus and annotation layers each
 *    carry a visually-hidden announcement as well as a class.
 */

/** Characters that must not reach the DOM as markup. */
const ESCAPES = {
  '&': '&amp;',
  '<': '&lt;',
  '>': '&gt;',
  '"': '&quot;',
  "'": '&#39;',
};

/**
 * Escapes text for HTML.
 *
 * Scripture text contains `<`, `>` and `&` rarely but genuinely (translator
 * brackets, ampersands in book titles), and footnotes contain them more often.
 * @param {string} s
 * @returns {string}
 */
export function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, (c) => ESCAPES[c]);
}

/**
 * Visually hidden but announced. Paired with `.sr-only` in the stylesheet.
 * @param {string} text
 */
function srOnly(text) {
  return `<span class="sr-only">${escapeHtml(text)}</span>`;
}

/**
 * Rendering context threaded through one chapter pass.
 *
 * `layers` is the set of annotation-layer ids covering the verse currently
 * being emitted (ADR-0009). It is *state*, not a per-inline property,
 * because a verse's text is a flat run of siblings — often bare text nodes —
 * and a verse can span several blocks of poetry. Tracking it here lets every
 * text node inside an annotated verse carry the layer class, which is what
 * makes the highlight cover the words rather than just the verse number.
 *
 * `inWj` suppresses layer tinting inside words of Jesus: what the
 * translation itself marks outranks an interpretive overlay.
 * @returns {{ layers: string[], inWj: boolean }}
 */
function freshContext() {
  return { layers: [], inWj: false };
}

/**
 * @param {object} inline
 * @param {{ layers: string[], inWj: boolean }} ctx
 * @returns {string}
 */
function renderInline(inline, ctx) {
  switch (inline.kind) {
    case 'text': {
      const text = escapeHtml(inline.text);
      if (ctx.layers.length && !ctx.inWj) {
        const cls = ctx.layers.map((id) => `layer-${escapeHtml(id)}`).join(' ');
        return `<span class="lt ${cls}">${text}</span>`;
      }
      return text;
    }

    case 'verseStart': {
      // The spoken label precedes the number so the reference is announced
      // before the text it introduces. The printed number is hidden from
      // assistive technology to avoid it being read twice.
      const layerCue = inline.spokenLayers?.length
        ? srOnly(`, ${inline.spokenLayers.join(', ')}, `)
        : '';
      const layerClasses = (inline.layers ?? [])
        .map((id) => `layer-${escapeHtml(id)}`)
        .join(' ');
      const cls = ['verse', layerClasses].filter(Boolean).join(' ');
      ctx.layers = inline.layers ?? [];
      return (
        `<span class="${cls}" data-verse="${inline.start}">` +
        srOnly(inline.label) +
        layerCue +
        `<sup class="verse-number" aria-hidden="true">${escapeHtml(inline.number)}</sup>` +
        `</span>`
      );
    }

    case 'verseEnd':
      // Zero-width. Emitted as a marker so verse-scoped selection and
      // highlighting have an anchor, but it contributes nothing visually and
      // nothing to the accessibility tree.
      ctx.layers = [];
      return `<span class="verse-end" data-verse="${inline.end}" aria-hidden="true"></span>`;

    case 'span': {
      const strong = inline.strong
        ? ` data-strong="${escapeHtml(inline.strong)}"`
        : '';
      if (inline.style === 'wj') {
        // Words of Jesus. Red by convention, but the convention is not the
        // cue: the announcement is, and the stylesheet adds a non-colour
        // treatment as well. Layer tinting is suppressed inside: the
        // translation's own markup outranks an interpretive overlay.
        const wasInWj = ctx.inWj;
        ctx.inWj = true;
        const inner = (inline.inner ?? []).map((i) => renderInline(i, ctx)).join('');
        ctx.inWj = wasInWj;
        return (
          `<span class="wj"${strong}>` +
          srOnly('words of Jesus begin. ') +
          inner +
          srOnly(' words of Jesus end. ') +
          `</span>`
        );
      }
      const inner = (inline.inner ?? []).map((i) => renderInline(i, ctx)).join('');
      return `<span class="ch-${escapeHtml(inline.style)}"${strong}>${inner}</span>`;
    }

    case 'note':
      // A button, not a superscript glyph: it is interactive, so it must be
      // focusable and operable by keyboard.
      return (
        `<button type="button" class="note-ref" data-note="${inline.id}" ` +
        `aria-label="Footnote ${inline.id + 1}">` +
        `<span aria-hidden="true">${escapeHtml(inline.caller === '+' ? '*' : inline.caller)}</span>` +
        `</button>`
      );

    default:
      // An unrecognised inline kind is dropped rather than rendered as
      // `[object Object]`. The Rust side reports unknown markers; this is the
      // last line of defence.
      return '';
  }
}

/**
 * @param {object} block
 * @param {{ layers: string[], inWj: boolean }} ctx shared across the
 *   chapter's blocks: a verse — and therefore its annotation layers — can
 *   span several blocks of poetry.
 * @returns {string}
 */
function renderBlock(block, ctx) {
  switch (block.kind) {
    case 'paragraph': {
      const inner = (block.inlines ?? []).map((i) => renderInline(i, ctx)).join('');
      // The style token becomes a class. Poetry indentation is
      // `padding-inline-start` in the stylesheet, so it works in RTL and is
      // never spoken.
      return `<p class="blk blk-${escapeHtml(block.style)}">${inner}</p>`;
    }

    case 'heading': {
      const level = Math.min(Math.max(block.level ?? 2, 2), 6);
      const inner = (block.inlines ?? []).map((i) => renderInline(i, ctx)).join('');
      return `<h${level} class="blk blk-${escapeHtml(block.style)}">${inner}</h${level}>`;
    }

    case 'blank':
      // A stanza break is semantic, not decorative, so it is a real element
      // with a class rather than a `<br>` or a margin hack.
      return `<div class="stanza-break" role="separator" aria-hidden="true"></div>`;

    default:
      return '';
  }
}

/**
 * Renders the notes section.
 * @param {object[]} notes
 */
function renderNotes(notes) {
  if (!notes?.length) return '';
  // Notes are not verse text; they never inherit the chapter's layer state.
  const items = notes
    .map(
      (n) =>
        `<li id="note-${n.id}" class="note note-${escapeHtml(n.kind)}">` +
        `${escapeHtml(n.text)}</li>`,
    )
    .join('');
  return (
    `<section class="notes" aria-labelledby="notes-heading">` +
    `<h2 id="notes-heading">Notes</h2>` +
    `<ol>${items}</ol>` +
    `</section>`
  );
}

/**
 * Renders a whole chapter.
 *
 * @param {object} view a ChapterView from `get_chapter`
 * @returns {string} HTML
 */
export function renderChapter(view) {
  if (!view) return '';

  const ctx = freshContext();
  const blocks = (view.blocks ?? []).map((b) => renderBlock(b, ctx)).join('');

  return (
    `<article class="chapter" lang="${escapeHtml(view.translation.language)}" ` +
    `dir="${escapeHtml(view.translation.direction)}" ` +
    `aria-label="${escapeHtml(view.title)}, ${escapeHtml(view.translation.abbrev)}">` +
    `<h1 class="chapter-title">${escapeHtml(view.title)}</h1>` +
    `<div class="chapter-body">${blocks}</div>` +
    renderNotes(view.notes) +
    // §4.3: attribution is surfaced wherever the translation is displayed.
    `<footer class="attribution">${escapeHtml(view.translation.attribution)}</footer>` +
    `</article>`
  );
}
