<script>
  import { renderChapter } from './render.js';

  /**
   * @type {{ view: object|null, onNavigate?: (book: string, chapter: number) => void }}
   */
  let { view = null, onNavigate = () => {} } = $props();

  const html = $derived(renderChapter(view));

  /**
   * Footnote markers are rendered as real buttons by `render.js`; this is the
   * one place the static HTML needs behaviour attached. Delegation rather than
   * per-button listeners, because the markup is set as a block.
   * @param {MouseEvent} event
   */
  function handleClick(event) {
    const target = /** @type {HTMLElement} */ (event.target);
    const button = target.closest('.note-ref');
    if (!button) return;
    const id = button.getAttribute('data-note');
    const note = document.getElementById(`note-${id}`);
    if (note) {
      note.scrollIntoView({ behavior: 'smooth', block: 'center' });
      // Move focus, not just the viewport: a sighted keyboard user and a
      // screen-reader user must both end up at the note.
      note.setAttribute('tabindex', '-1');
      note.focus();
    }
  }
</script>

<!--
  The rendered chapter is set as a block because all of its structure and every
  accessibility decision is computed in `render.js`, where it is unit-tested.
  Rebuilding that structure with Svelte control flow would move the rules into
  a component where nothing checks them.

  The content is not user input: it comes from the Rust core, and `render.js`
  escapes every text node and attribute value (see render.test.js).
-->
<!-- eslint-disable-next-line svelte/no-at-html-tags -->
<div class="chapter-host" onclick={handleClick} role="presentation">
  {@html html}
</div>

{#if view?.nav}
  <nav class="chapter-nav" aria-label="Chapter navigation">
    {#if view.nav.previous}
      <button
        type="button"
        onclick={() => onNavigate(view.nav.previous.book, view.nav.previous.chapter)}
      >
        <span aria-hidden="true">←</span>
        {view.nav.previous.label}
      </button>
    {:else}
      <span></span>
    {/if}

    {#if view.nav.next}
      <button
        type="button"
        onclick={() => onNavigate(view.nav.next.book, view.nav.next.chapter)}
      >
        {view.nav.next.label}
        <span aria-hidden="true">→</span>
      </button>
    {/if}
  </nav>
{/if}

<style>
  .chapter-nav {
    display: flex;
    justify-content: space-between;
    gap: 1rem;
    max-inline-size: var(--measure);
    margin-inline: auto;
    padding-inline: 1.25rem;
    padding-block-end: 3rem;
  }

  .chapter-nav button {
    font-family: var(--font-ui);
    font-size: 0.9rem;
    color: var(--accent);
    background: var(--paper-raised);
    border: 1px solid var(--rule);
    border-radius: 6px;
    padding: 0.5rem 0.9rem;
    cursor: pointer;
  }

  .chapter-nav button:hover {
    border-color: var(--accent);
  }
</style>
