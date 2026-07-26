<script>
  /**
   * The first screen: choose which translation to read.
   *
   * Attribution is shown on the card, not hidden behind a detail view —
   * §4.3 requires it wherever a translation is displayed, and the picker
   * displays translations.
   *
   * @type {{ translations: object[], onSelect?: (id: string) => void }}
   */
  let { translations = [], onSelect = () => {} } = $props();
</script>

<section class="picker" aria-labelledby="picker-heading">
  <h1 id="picker-heading">Choose a translation</h1>

  {#if translations.length === 0}
    <p class="empty" role="alert">
      The corpus contains no translations. Run: cargo run -p build-corpus
    </p>
  {:else}
    <ul class="translations">
      {#each translations as t (t.id)}
        <li>
          <button type="button" class="translation" onclick={() => onSelect(t.id)}>
            <span class="abbrev" aria-hidden="true">{t.abbrev}</span>
            <span class="name" lang={t.language} dir={t.direction}>{t.name}</span>
            <span class="attribution">{t.attribution}</span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .picker {
    max-inline-size: var(--measure);
    margin-inline: auto;
    padding-inline: 1.25rem;
    padding-block: 3rem 4rem;
  }

  h1 {
    font-size: 1.6rem;
    font-weight: 600;
    margin-block-end: 1.5rem;
    padding-block-end: 0.5rem;
    border-block-end: 1px solid var(--rule);
  }

  .empty {
    font-family: var(--font-ui);
    color: var(--ink-muted);
  }

  .translations {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 0.75rem;
  }

  .translation {
    inline-size: 100%;
    display: grid;
    gap: 0.25rem;
    text-align: start;
    font-family: var(--font-ui);
    color: var(--ink);
    background: var(--paper-raised);
    border: 1px solid var(--rule);
    border-radius: 8px;
    padding: 0.9rem 1.1rem;
    cursor: pointer;
  }

  .translation:hover {
    border-color: var(--accent);
  }

  .abbrev {
    font-size: 0.75rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--accent);
  }

  .name {
    font-family: var(--font-body);
    font-size: 1.15rem;
  }

  .attribution {
    font-size: 0.75rem;
    color: var(--ink-muted);
  }
</style>
