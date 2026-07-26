<script>
  /**
   * The table of contents: every book, expanding to a grid of its chapters.
   *
   * Books are disclosure buttons (`aria-expanded`), not links to a second
   * page — a reader hunting for 2 Kings 6 should never lose their place in
   * the book list while doing it. A single-chapter book (Obadiah, Jude…)
   * skips the disclosure and opens directly.
   *
   * @type {{ toc: object, onSelect?: (book: string, chapter: number) => void }}
   */
  let { toc, onSelect = () => {} } = $props();

  /** @type {string|null} the USFM code of the expanded book */
  let open = $state(null);

  function toggle(id) {
    open = open === id ? null : id;
  }
</script>

<section class="toc" aria-labelledby="toc-heading">
  <h1 id="toc-heading">Contents</h1>
  <p class="toc-translation">
    <span lang={toc.translation.language} dir={toc.translation.direction}>
      {toc.translation.name}
    </span>
  </p>

  <ul class="books">
    {#each toc.books as b (b.id)}
      <li>
        {#if b.chapters.length === 1}
          <button type="button" class="book" onclick={() => onSelect(b.id, b.chapters[0])}>
            {b.name}
          </button>
        {:else}
          <button
            type="button"
            class="book"
            aria-expanded={open === b.id}
            onclick={() => toggle(b.id)}
          >
            {b.name}
            <span class="count" aria-hidden="true">{b.chapters.length}</span>
          </button>
          {#if open === b.id}
            <ol class="chapters" aria-label="Chapters of {b.name}">
              {#each b.chapters as c (c)}
                <li>
                  <button type="button" class="chapter-btn" onclick={() => onSelect(b.id, c)}>
                    <span class="sr-only">{b.name} chapter</span>{c}
                  </button>
                </li>
              {/each}
            </ol>
          {/if}
        {/if}
      </li>
    {/each}
  </ul>

  <footer class="attribution">{toc.translation.attribution}</footer>
</section>

<style>
  .toc {
    max-inline-size: var(--measure);
    margin-inline: auto;
    padding-inline: 1.25rem;
    padding-block: 2rem 4rem;
  }

  h1 {
    font-size: 1.6rem;
    font-weight: 600;
    margin-block-end: 0.25rem;
  }

  .toc-translation {
    font-family: var(--font-ui);
    font-size: 0.85rem;
    color: var(--ink-muted);
    margin-block: 0 1.5rem;
    padding-block-end: 0.75rem;
    border-block-end: 1px solid var(--rule);
  }

  .books {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .book {
    inline-size: 100%;
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 1rem;
    font-family: var(--font-body);
    font-size: 1.05rem;
    color: var(--ink);
    background: none;
    border: 0;
    border-block-end: 1px solid var(--rule);
    padding: 0.55rem 0.25rem;
    cursor: pointer;
    text-align: start;
  }

  .book:hover {
    color: var(--accent);
  }

  .book[aria-expanded='true'] {
    color: var(--accent);
  }

  .count {
    font-family: var(--font-ui);
    font-size: 0.7rem;
    color: var(--ink-muted);
  }

  .chapters {
    list-style: none;
    margin: 0;
    padding: 0.75rem 0.25rem 1rem;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(2.75rem, 1fr));
    gap: 0.4rem;
    border-block-end: 1px solid var(--rule);
  }

  .chapter-btn {
    font-family: var(--font-ui);
    font-size: 0.9rem;
    color: var(--ink);
    background: var(--paper-raised);
    border: 1px solid var(--rule);
    border-radius: 6px;
    padding-block: 0.45rem;
    inline-size: 100%;
    cursor: pointer;
  }

  .chapter-btn:hover {
    border-color: var(--accent);
    color: var(--accent);
  }
</style>
