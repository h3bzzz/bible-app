<script>
  import { invoke } from '@tauri-apps/api/core';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import Chapter from './lib/Chapter.svelte';
  import Toc from './lib/Toc.svelte';
  import TranslationPicker from './lib/TranslationPicker.svelte';
  import './lib/theme.css';

  // The startup flow: pick a translation (remembered across launches), then
  // open at the last-read position, or failing that the first page of the
  // corpus — Genesis 1 in any full bible. The "first page" is not a constant
  // here: it is books[0].chapters[0] of the table of contents, so a corpus
  // that starts elsewhere (a NT-only translation) still opens at its own
  // first page.

  // The window is undecorated (the native titlebar cannot follow the app
  // theme), so the app bar doubles as the titlebar: it carries the drag
  // region and the minimize/maximize/close controls. Null outside Tauri —
  // in a plain browser tab the controls simply don't render.
  //
  // A mobile Tauri build has `__TAURI_INTERNALS__` too, so this handle is
  // non-null on a phone. That is fine for `getCurrentWindow`, but the window
  // *chrome* — drag region, minimize/maximize/close — is meaningless there:
  // the OS owns the title bar and there is nothing to minimize. So the chrome
  // is gated on `isDesktop` below, resolved once from the backend, not on
  // `appWindow` alone.
  const appWindow = '__TAURI_INTERNALS__' in window ? getCurrentWindow() : null;

  // True only on a desktop build. Drives whether the window chrome renders and
  // whether the desktop-only commands (`desktop_env`, `toggle_maximize`) may
  // be invoked — on mobile they are not even registered. Resolved in `start`.
  let isDesktop = $state(false);

  /**
   * Hyprland (and tiling compositors generally) have no minimize; showing a
   * button that can never work is worse than not showing it. Resolved once
   * at startup.
   */
  let canMinimize = $state(true);

  // The bar steps out of the way while reading: scrolling down hides it,
  // and scrolling up, returning to the top, or touching the top edge of the
  // window brings it back. It never hides outside the reading screen — the
  // other screens are navigation, where the bar is the way out.
  let barHidden = $state(false);
  let lastY = 0;

  function onScroll() {
    const y = window.scrollY;
    if (screen === 'read') {
      if (y < 48) barHidden = false;
      else if (y > lastY + 4) barHidden = true;
      else if (y < lastY - 4) barHidden = false;
    }
    lastY = y;
  }

  function onMouseMove(e) {
    if (barHidden && e.clientY <= 16) barHidden = false;
  }

  const LAST_TRANSLATION = 'bible.translation';
  const THEME = 'bible.theme';
  const positionKey = (tid) => `bible.position.${tid}`;

  /**
   * 'light' or 'dark'. Starts from the saved choice, else the system
   * preference. The value is stamped on the root element as `data-theme`,
   * which the stylesheet treats as overriding the media query in both
   * directions.
   * @type {'light'|'dark'}
   */
  let theme = $state(
    localStorage.getItem(THEME) ??
      (window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'),
  );

  $effect(() => {
    document.documentElement.dataset.theme = theme;
  });

  function toggleTheme() {
    theme = theme === 'dark' ? 'light' : 'dark';
    localStorage.setItem(THEME, theme);
  }

  // Annotation layers (ADR-0009). Interpretive layers default to off — the
  // application does not assert a contested reading the reader did not ask
  // for — but the choice, once made, is remembered.
  const LAYERS = 'bible.layers';

  /** @type {object[]} every layer compiled into the binary */
  let availableLayers = $state([]);
  /** @type {string[]} the ids currently switched on */
  let enabledLayers = $state([]);

  const prophecyOn = $derived(enabledLayers.includes('messianic-prophecy'));

  function toggleLayer(id) {
    enabledLayers = enabledLayers.includes(id)
      ? enabledLayers.filter((l) => l !== id)
      : [...enabledLayers, id];
    localStorage.setItem(LAYERS, JSON.stringify(enabledLayers));
    if (screen === 'read' && view) load(view.book.id, view.chapter);
  }

  const MEASURE = 'bible.measure';
  /** The three column widths, widest first; labels are what the button shows. */
  const MEASURES = [
    { id: 'wide', label: 'Wide' },
    { id: 'medium', label: 'Medium' },
    { id: 'narrow', label: 'Narrow' },
  ];

  /** @type {'wide'|'medium'|'narrow'} */
  let measure = $state(
    MEASURES.some((m) => m.id === localStorage.getItem(MEASURE))
      ? localStorage.getItem(MEASURE)
      : 'wide',
  );

  $effect(() => {
    document.documentElement.dataset.measure = measure;
  });

  function cycleMeasure() {
    const i = MEASURES.findIndex((m) => m.id === measure);
    measure = MEASURES[(i + 1) % MEASURES.length].id;
    localStorage.setItem(MEASURE, measure);
  }

  const measureLabel = $derived(MEASURES.find((m) => m.id === measure)?.label ?? 'Wide');

  const FONT_SIZE = 'bible.fontSize';
  const FONT_MIN = 70;
  const FONT_MAX = 200;
  const FONT_STEP = 10;

  /**
   * Text size as a percentage of the user's browser/system default. Applied
   * to the root element, and everything below is `rem`-based by design — so
   * the whole page scales together, and the reading measure (also in rem)
   * keeps its line length in characters rather than growing longer lines.
   */
  let fontSize = $state(Number(localStorage.getItem(FONT_SIZE)) || 100);

  $effect(() => {
    document.documentElement.style.fontSize = `${fontSize}%`;
  });

  function adjustFont(delta) {
    fontSize = Math.min(FONT_MAX, Math.max(FONT_MIN, fontSize + delta));
    localStorage.setItem(FONT_SIZE, String(fontSize));
  }

  /** @type {'loading'|'pick'|'contents'|'read'|'error'} */
  let screen = $state('loading');
  let translations = $state([]);
  /** @type {string|null} the selected translation id */
  let translationId = $state(null);
  let toc = $state(null);
  let view = $state(null);
  let error = $state(null);

  const current = $derived(translations.find((t) => t.id === translationId) ?? null);

  function fail(e) {
    error = String(e);
    screen = 'error';
  }

  async function start() {
    try {
      if (appWindow) {
        isDesktop = await invoke('is_desktop');
      }
      if (isDesktop) {
        const env = await invoke('desktop_env');
        canMinimize = !String(env).includes('Hyprland');
      }
      availableLayers = await invoke('list_layers');
      try {
        const saved = JSON.parse(localStorage.getItem(LAYERS) ?? 'null');
        enabledLayers = Array.isArray(saved)
          ? saved.filter((id) => availableLayers.some((l) => l.id === id))
          : availableLayers.filter((l) => l.enabledByDefault).map((l) => l.id);
      } catch {
        enabledLayers = [];
      }
      translations = await invoke('list_translations');
      const saved = localStorage.getItem(LAST_TRANSLATION);
      if (translations.some((t) => t.id === saved)) {
        await select(saved);
      } else {
        screen = 'pick';
      }
    } catch (e) {
      fail(e);
    }
  }

  async function select(id) {
    try {
      translationId = id;
      localStorage.setItem(LAST_TRANSLATION, id);
      toc = await invoke('get_toc', { translation: id });
      const target = savedPosition(id) ?? firstPage();
      await load(target.book, target.chapter);
    } catch (e) {
      fail(e);
    }
  }

  /** The first page of this corpus: the first chapter of the first book. */
  function firstPage() {
    const first = toc?.books?.[0];
    return first
      ? { book: first.id, chapter: first.chapters[0] }
      : { book: 'GEN', chapter: 1 };
  }

  /**
   * The last-read position, but only if it still exists in this corpus — a
   * rebuilt corpus that dropped a book must not strand the reader on an
   * error screen at startup.
   */
  function savedPosition(id) {
    try {
      const raw = localStorage.getItem(positionKey(id));
      if (!raw) return null;
      const p = JSON.parse(raw);
      const book = toc?.books?.find((b) => b.id === p.book);
      return book && book.chapters.includes(p.chapter) ? p : null;
    } catch {
      return null;
    }
  }

  async function load(book, chapter) {
    try {
      view = await invoke('get_chapter', {
        translation: translationId,
        book,
        chapter,
        // `$state.snapshot`, not the raw value: `enabledLayers` is a Svelte
        // proxy, and what crosses the Tauri IPC boundary must be a plain
        // array or it can serialise to nothing — silently un-highlighting
        // every layer.
        layers: $state.snapshot(enabledLayers),
      });
      localStorage.setItem(positionKey(translationId), JSON.stringify({ book, chapter }));
      error = null;
      screen = 'read';
      window.scrollTo({ top: 0 });
    } catch (e) {
      fail(e);
    }
  }

  $effect(() => {
    start();
  });

  // Leaving the reading screen always restores the bar.
  $effect(() => {
    if (screen !== 'read') barHidden = false;
  });
</script>

<svelte:head>
  <title>{view && screen === 'read' ? `${view.title} — ${view.translation.abbrev}` : 'Bible'}</title>
</svelte:head>

<svelte:window onscroll={onScroll} onmousemove={onMouseMove} />

<a class="skip-link" href="#main">Skip to text</a>

<header class="app-bar" class:bar-hidden={barHidden} data-tauri-drag-region>
  <div class="bar-group" data-tauri-drag-region>
    {#if screen === 'read' || screen === 'contents'}
      <button
        type="button"
        class="bar-btn"
        aria-expanded={screen === 'contents'}
        onclick={() => (screen = screen === 'contents' ? 'read' : 'contents')}
      >
        Contents
      </button>
    {/if}
  </div>
  <span class="bar-title" aria-hidden="true" data-tauri-drag-region>
    {screen === 'read' && view ? view.title : ''}
  </span>
  <div class="bar-group" data-tauri-drag-region>
    {#if (screen === 'read' || screen === 'contents') && current}
      <!-- "Translations", not the bare abbreviation: "WEB" alone reads as
           the platform, not as the World English Bible. -->
      <button
        type="button"
        class="bar-btn"
        onclick={() => (screen = 'pick')}
        aria-label="Switch translation, currently {current.name}"
      >
        Translations · {current.abbrev}
      </button>
    {/if}
    {#if availableLayers.some((l) => l.id === 'messianic-prophecy')}
      <button
        type="button"
        class="bar-btn"
        class:bar-btn-on={prophecyOn}
        aria-pressed={prophecyOn}
        onclick={() => toggleLayer('messianic-prophecy')}
        title="Highlight Old Testament messianic prophecies"
      >
        Prophecy
      </button>
    {/if}
    <button
      type="button"
      class="bar-btn"
      onclick={cycleMeasure}
      aria-label="Column width: {measureLabel}. Cycle to next width"
    >
      {measureLabel}
    </button>
    <button
      type="button"
      class="bar-btn"
      onclick={() => adjustFont(-FONT_STEP)}
      disabled={fontSize <= FONT_MIN}
      aria-label="Decrease text size"
    >
      <span aria-hidden="true">A−</span>
    </button>
    <button
      type="button"
      class="bar-btn"
      onclick={() => adjustFont(FONT_STEP)}
      disabled={fontSize >= FONT_MAX}
      aria-label="Increase text size"
    >
      <span aria-hidden="true">A+</span>
    </button>
    <button
      type="button"
      class="bar-btn"
      onclick={toggleTheme}
      aria-label="Switch to {theme === 'dark' ? 'light' : 'dark'} theme"
    >
      <span aria-hidden="true">{theme === 'dark' ? '☀' : '☾'}</span>
    </button>
    {#if appWindow && isDesktop}
      <span class="win-controls">
        {#if canMinimize}
          <button
            type="button"
            class="win-btn"
            onclick={() => appWindow.minimize()}
            aria-label="Minimize window"
          >
            <span aria-hidden="true">─</span>
          </button>
        {/if}
        <button
          type="button"
          class="win-btn"
          onclick={() => invoke('toggle_maximize')}
          aria-label="Maximize or restore window"
        >
          <span aria-hidden="true">▢</span>
        </button>
        <button
          type="button"
          class="win-btn win-close"
          onclick={() => appWindow.close()}
          aria-label="Close window"
        >
          <span aria-hidden="true">✕</span>
        </button>
      </span>
    {/if}
  </div>
</header>

<main id="main">
  {#if screen === 'error'}
    <p class="error" role="alert">{error}</p>
  {:else if screen === 'loading'}
    <p class="loading" aria-live="polite">Loading…</p>
  {:else if screen === 'pick'}
    <TranslationPicker {translations} onSelect={select} />
  {:else if screen === 'contents'}
    <Toc {toc} onSelect={load} />
  {:else if view}
    <Chapter {view} onNavigate={load} />
  {/if}
</main>

<style>
  .skip-link {
    position: absolute;
    inset-inline-start: -9999px;
    padding: 0.6rem 1rem;
    background: var(--paper-raised);
    color: var(--ink);
    z-index: 10;
  }
  .skip-link:focus {
    inset-inline-start: 0.5rem;
    inset-block-start: 0.5rem;
  }

  .app-bar {
    position: sticky;
    inset-block-start: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.5rem 0.75rem;
    background: var(--paper);
    border-block-end: 1px solid var(--rule);
    z-index: 5;
  }

  .bar-group {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-shrink: 0;
    /* Both groups reserve space even when empty, so the title stays centred
       and the bar does not jump between screens. */
    min-inline-size: 2.5rem;
  }

  .bar-title {
    font-family: var(--font-ui);
    font-size: 0.85rem;
    color: var(--ink-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .bar-btn {
    font-family: var(--font-ui);
    font-size: 0.85rem;
    color: var(--accent);
    background: var(--paper-raised);
    border: 1px solid var(--rule);
    border-radius: 6px;
    padding: 0.35rem 0.75rem;
    cursor: pointer;
    flex-shrink: 0;
  }

  .bar-btn:hover {
    border-color: var(--accent);
  }

  /* An active layer's button carries the layer's own colour, so the control
     and the highlight it produces are visibly the same thing. */
  .bar-btn-on {
    color: var(--prophecy);
    border-color: var(--prophecy);
    background: var(--prophecy-wash);
  }

  .bar-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .bar-btn:disabled:hover {
    border-color: var(--rule);
  }

  .app-bar.bar-hidden {
    transform: translateY(-100%);
  }

  .app-bar {
    transition: transform 0.25s ease;
  }

  .win-controls {
    display: flex;
    align-items: stretch;
    margin-inline-start: 0.5rem;
    padding-inline-start: 0.5rem;
    border-inline-start: 1px solid var(--rule);
  }

  .win-btn {
    font-family: var(--font-ui);
    font-size: 0.8rem;
    line-height: 1;
    color: var(--ink-muted);
    background: none;
    border: 0;
    border-radius: 6px;
    padding: 0.45rem 0.6rem;
    cursor: pointer;
  }

  .win-btn:hover {
    background: var(--paper-raised);
    color: var(--ink);
  }

  /* Close is the one destructive control; its hover says so. */
  .win-close:hover {
    background: #a3301f;
    color: #fff;
  }

  .error,
  .loading {
    max-inline-size: var(--measure);
    margin-inline: auto;
    padding: 2rem 1.25rem;
    font-family: var(--font-ui);
  }
  .error {
    color: #a3301f;
  }
</style>
