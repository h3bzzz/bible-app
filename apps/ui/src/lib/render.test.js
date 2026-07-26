/**
 * Tests for the chapter renderer.
 *
 * Run with `node --test apps/ui/src/lib/`.
 *
 * These are the accessibility floor from ADR-0002 and ADR-0009, expressed as
 * assertions. A rule stated only in a document is a rule that drifts.
 */

import test from 'node:test';
import assert from 'node:assert/strict';
import { renderChapter, escapeHtml } from './render.js';

/** A minimal but realistic ChapterView, shaped exactly like `get_chapter` emits. */
function psalm23View(overrides = {}) {
  return {
    translation: {
      id: 'eng-web',
      abbrev: 'WEB',
      name: 'World English Bible',
      language: 'en',
      direction: 'ltr',
      attribution: 'The World English Bible is in the public domain.',
    },
    book: { id: 'PSA', name: 'Psalms' },
    chapter: 23,
    title: 'Psalms 23',
    verses: [1, 2],
    nav: { previous: null, next: null },
    notes: [],
    blocks: [
      {
        kind: 'paragraph',
        style: 'd',
        inlines: [{ kind: 'text', text: 'A Psalm by David.' }],
      },
      {
        kind: 'paragraph',
        style: 'q1',
        inlines: [
          {
            kind: 'verseStart',
            start: 1,
            end: 1,
            number: '1',
            label: 'Psalms 23 verse 1',
            layers: [],
            spokenLayers: [],
          },
          { kind: 'text', text: 'Yahweh is my shepherd;' },
        ],
      },
      {
        kind: 'paragraph',
        style: 'q2',
        inlines: [{ kind: 'text', text: 'I shall lack nothing.' }],
      },
    ],
    ...overrides,
  };
}

test('emits lang and dir from the translation, never inferred', () => {
  const html = renderChapter(psalm23View());
  assert.match(html, /lang="en"/);
  assert.match(html, /dir="ltr"/);
});

test('a right-to-left translation gets dir=rtl', () => {
  const view = psalm23View();
  view.translation.language = 'hbo';
  view.translation.direction = 'rtl';
  const html = renderChapter(view);
  assert.match(html, /lang="hbo"/);
  assert.match(html, /dir="rtl"/);
});

test('verse numbers are hidden from assistive technology', () => {
  const html = renderChapter(psalm23View());
  assert.match(
    html,
    /<sup class="verse-number" aria-hidden="true">1<\/sup>/,
    'the printed number must be aria-hidden',
  );
});

test('each verse carries a spoken reference before its text', () => {
  const html = renderChapter(psalm23View());
  const srIndex = html.indexOf('Psalms 23 verse 1');
  const numIndex = html.indexOf('verse-number');
  const textIndex = html.indexOf('Yahweh is my shepherd');
  assert.ok(srIndex > -1, 'the spoken label must be present');
  assert.ok(srIndex < numIndex, 'the label precedes the printed number');
  assert.ok(numIndex < textIndex, 'both precede the verse text');
});

test('a bridged verse announces the range', () => {
  const view = psalm23View();
  view.blocks[1].inlines[0] = {
    kind: 'verseStart',
    start: 19,
    end: 27,
    number: '19-27',
    label: 'Sirach 26 verses 19-27',
    layers: [],
    spokenLayers: [],
  };
  const html = renderChapter(view);
  assert.match(html, /Sirach 26 verses 19-27/);
  assert.match(html, />19-27<\/sup>/);
});

test('poetry indentation is a class, never whitespace entities', () => {
  const html = renderChapter(psalm23View());
  assert.match(html, /class="blk blk-q1"/);
  assert.match(html, /class="blk blk-q2"/);
  assert.doesNotMatch(html, /&nbsp;/, 'indentation must not be spoken');
  assert.doesNotMatch(html, /&emsp;/);
});

test('headings are real heading elements in document order', () => {
  const view = psalm23View();
  view.blocks.unshift({
    kind: 'heading',
    level: 2,
    style: 'ms1',
    inlines: [{ kind: 'text', text: 'BOOK 1' }],
  });
  const html = renderChapter(view);
  assert.match(html, /<h2 class="blk blk-ms1">BOOK 1<\/h2>/);
  // The chapter title is the h1, so sections start at h2 and never skip.
  assert.match(html, /<h1 class="chapter-title">Psalms 23<\/h1>/);
});

test('heading levels are clamped into a valid range', () => {
  const view = psalm23View();
  view.blocks.unshift({ kind: 'heading', level: 99, style: 's1', inlines: [] });
  const html = renderChapter(view);
  assert.match(html, /<h6/);
  assert.doesNotMatch(html, /<h99/);
});

test('words of Jesus are announced, not signalled by colour alone', () => {
  const view = psalm23View();
  view.blocks[1].inlines.push({
    kind: 'span',
    style: 'wj',
    inner: [{ kind: 'text', text: 'Come and see.' }],
  });
  const html = renderChapter(view);
  assert.match(html, /class="wj"/, 'a class for the visual treatment');
  assert.match(html, /words of Jesus begin/, 'and an announcement for the rest');
  assert.match(html, /words of Jesus end/);
});

test('annotation layers add both a class and an announcement', () => {
  const view = psalm23View();
  view.blocks[1].inlines[0] = {
    kind: 'verseStart',
    start: 4,
    end: 4,
    number: '4',
    label: 'Isaiah 53 verse 4',
    layers: ['messianic-prophecy'],
    spokenLayers: ['messianic prophecy'],
  };
  const html = renderChapter(view);
  assert.match(html, /layer-messianic-prophecy/, 'class for the theme to colour');
  assert.match(html, /messianic prophecy/, 'and a spoken cue, so colour is not the only signal');
});

test('a verse with no layers gets no layer markup', () => {
  const html = renderChapter(psalm23View());
  assert.doesNotMatch(html, /layer-/);
});

test('stanza breaks are semantic elements, not margin hacks', () => {
  const view = psalm23View();
  view.blocks.push({ kind: 'blank' });
  const html = renderChapter(view);
  assert.match(html, /class="stanza-break"/);
  assert.doesNotMatch(html, /<br\s*\/?>/);
});

test('note references are focusable buttons with labels', () => {
  const view = psalm23View();
  view.blocks[1].inlines.push({ kind: 'note', id: 0, caller: '+' });
  view.notes = [
    { id: 0, kind: 'footnote', caller: '+', inlines: [], text: 'TR adds a clause.' },
  ];
  const html = renderChapter(view);
  assert.match(html, /<button type="button" class="note-ref"/, 'must be keyboard operable');
  assert.match(html, /aria-label="Footnote 1"/);
  assert.match(html, /<li id="note-0"/);
  assert.match(html, /TR adds a clause\./);
});

test('Strong’s numbers travel as data attributes', () => {
  const view = psalm23View();
  view.blocks[1].inlines.push({
    kind: 'span',
    style: 'w',
    strong: 'H3068',
    inner: [{ kind: 'text', text: 'Yahweh' }],
  });
  const html = renderChapter(view);
  assert.match(html, /data-strong="H3068"/);
});

test('attribution is always surfaced', () => {
  const html = renderChapter(psalm23View());
  assert.match(html, /class="attribution"/);
  assert.match(html, /public domain/);
});

test('text is escaped', () => {
  const view = psalm23View();
  view.blocks[1].inlines[1] = {
    kind: 'text',
    text: '<script>alert("xss")</script> & more',
  };
  const html = renderChapter(view);
  assert.doesNotMatch(html, /<script>/, 'markup in content must not reach the DOM');
  assert.match(html, /&lt;script&gt;/);
  assert.match(html, /&amp; more/);
});

test('attribute values are escaped too', () => {
  const view = psalm23View();
  view.title = 'Psalms "23"';
  view.translation.abbrev = '<WEB>';
  const html = renderChapter(view);
  assert.doesNotMatch(html, /aria-label="Psalms "23""/);
  assert.match(html, /&quot;23&quot;/);
  assert.match(html, /&lt;WEB&gt;/);
});

test('escapeHtml handles every dangerous character', () => {
  assert.equal(escapeHtml(`<>&"'`), '&lt;&gt;&amp;&quot;&#39;');
  assert.equal(escapeHtml('plain'), 'plain');
});

test('unknown inline and block kinds are dropped, not stringified', () => {
  const view = psalm23View();
  view.blocks.push({ kind: 'somethingNew', inlines: [] });
  view.blocks[1].inlines.push({ kind: 'alsoNew' });
  const html = renderChapter(view);
  assert.doesNotMatch(html, /\[object Object\]/);
  assert.doesNotMatch(html, /somethingNew/);
});

test('an empty view renders nothing rather than throwing', () => {
  assert.equal(renderChapter(null), '');
  assert.equal(renderChapter(undefined), '');
});

test('a chapter with no blocks still renders its frame', () => {
  const view = psalm23View({ blocks: [] });
  const html = renderChapter(view);
  assert.match(html, /<article class="chapter"/);
  assert.match(html, /<h1 class="chapter-title">/);
});

// ── Annotation layer tinting (ADR-0009) ─────────────────────────────────

/** Isaiah 53 with verse 5 in the messianic-prophecy layer, spanning blocks. */
function annotatedView() {
  return psalm23View({
    title: 'Isaiah 53',
    blocks: [
      {
        kind: 'paragraph',
        style: 'q1',
        inlines: [
          {
            kind: 'verseStart',
            start: 5,
            end: 5,
            number: '5',
            label: 'Isaiah 53 verse 5',
            layers: ['messianic-prophecy'],
            spokenLayers: ['messianic prophecy'],
          },
          { kind: 'text', text: 'But he was pierced for our transgressions.' },
        ],
      },
      {
        kind: 'paragraph',
        style: 'q2',
        inlines: [
          { kind: 'text', text: 'The punishment was upon him.' },
          { kind: 'verseEnd', start: 5, end: 5 },
          { kind: 'text', text: 'After the verse.' },
        ],
      },
    ],
  });
}

test('text inside an annotated verse carries the layer class', () => {
  const html = renderChapter(annotatedView());
  assert.match(
    html,
    /<span class="lt layer-messianic-prophecy">But he was pierced for our transgressions\.<\/span>/,
  );
});

test('layer tinting continues across block boundaries within the verse', () => {
  const html = renderChapter(annotatedView());
  assert.match(
    html,
    /<span class="lt layer-messianic-prophecy">The punishment was upon him\.<\/span>/,
  );
});

test('text after the verse end is not tinted', () => {
  const html = renderChapter(annotatedView());
  assert.match(html, />After the verse\./);
  assert.doesNotMatch(html, /<span class="lt[^>]*>After the verse\./);
});

test('words of Jesus are not layer-tinted inside an annotated verse', () => {
  const view = annotatedView();
  view.blocks[0].inlines.push({
    kind: 'span',
    style: 'wj',
    inner: [{ kind: 'text', text: 'I will.' }],
  });
  const html = renderChapter(view);
  // The wj span keeps its own text untinted; the translation's markup wins.
  assert.match(html, /<span class="wj">.*I will\..*<\/span>/);
  assert.doesNotMatch(html, /<span class="lt[^"]*">I will\./);
});

test('unannotated verses emit no tint spans at all', () => {
  const html = renderChapter(psalm23View());
  assert.doesNotMatch(html, /class="lt/);
});
