<script lang="ts">
  import { fade } from 'svelte/transition';
  import type { LiveItem, MovieItem, SeriesItem } from './models';
  import { nav } from './nav.svelte';
  import { player } from './player.svelte';
  import { store } from './store.svelte';
  import Tag from './Tag.svelte';
  import { displayName } from './theme';

  /** Ctrl+K: one field across channels, movies and series. */
  type Hit =
    | { kind: 'Live'; item: LiveItem; title: string }
    | { kind: 'Movie'; item: MovieItem; title: string }
    | { kind: 'Series'; item: SeriesItem; title: string };

  const COLORS = { Live: 'var(--magenta)', Movie: 'var(--cyan)', Series: 'var(--acid)' };

  let query = $state('');
  let index = $state(0);
  let input: HTMLInputElement;
  let listEl: HTMLElement | undefined = $state();

  /** Case and accents don't matter, like Finder's search: "ete" finds "Été". */
  const folded = new Map<string, string>();
  function fold(s: string): string {
    let f = folded.get(s);
    if (f === undefined) {
      f = s.normalize('NFD').replace(/\p{Diacritic}/gu, '').toLowerCase();
      folded.set(s, f);
    }
    return f;
  }

  function first<T>(items: T[], name: (i: T) => string, q: string, limit: number): T[] {
    const out: T[] = [];
    for (const i of items) {
      if (fold(name(i)).includes(q) && out.push(i) >= limit) break;
    }
    return out;
  }

  const hits = $derived.by((): Hit[] => {
    const q = fold(query.trim());
    if (!q) return [];
    return [
      ...first(store.channels, (i) => i.channel.name, q, 12).map((item) => ({ kind: 'Live' as const, item, title: displayName(item.channel.name) })),
      ...first(store.movies, (i) => i.movie.name, q, 10).map((item) => ({ kind: 'Movie' as const, item, title: item.movie.name })),
      ...first(store.series, (i) => i.series.name, q, 10).map((item) => ({ kind: 'Series' as const, item, title: item.series.name })),
    ];
  });

  $effect(() => {
    void query;
    index = 0;
  });

  $effect(() => {
    listEl?.children[index]?.scrollIntoView({ block: 'nearest' });
  });

  $effect(() => input.focus());

  function open(hit: Hit) {
    nav.searching = false;
    switch (hit.kind) {
      case 'Live':
        void player.collapse();
        nav.focusChannel = hit.item.id;
        nav.section = 'live';
        break;
      case 'Movie':
        store.playMovie(hit.item);
        break;
      case 'Series':
        void player.collapse();
        nav.openSeries = hit.item;
        nav.section = 'series';
        break;
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') nav.searching = false;
    else if (e.key === 'ArrowDown') index = Math.min(index + 1, Math.max(hits.length - 1, 0));
    else if (e.key === 'ArrowUp') index = Math.max(index - 1, 0);
    else if (e.key === 'Enter' && hits[index]) open(hits[index]);
    else return;
    e.preventDefault();
    e.stopPropagation();
  }
</script>

<div class="scrim" transition:fade={{ duration: 150 }} onmousedown={(e) => e.target === e.currentTarget && (nav.searching = false)} role="presentation">
  <div class="palette" role="dialog" aria-label="Search">
    <label class="field">
      <span class="icon">&#xE721;</span>
      <input bind:this={input} bind:value={query} {onkeydown} placeholder="Channels, movies, series" spellcheck="false" />
    </label>
    {#if hits.length}
      <ul bind:this={listEl} role="listbox">
        {#each hits as hit, i (hit.kind + hit.item.id)}
          <li role="option" aria-selected={i === index}>
            <button class:on={i === index} onclick={() => open(hit)} onmousemove={() => (index = i)}>
              <span class="kind"><Tag text={hit.kind} color={COLORS[hit.kind]} /></span>
              <span class="title">{hit.title}</span>
            </button>
          </li>
        {/each}
      </ul>
    {:else if query.trim()}
      <p class="none">Nothing matches.</p>
    {/if}
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 20;
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 120px;
    background: rgb(5 1 15 / 0.6);
  }
  .palette {
    width: 680px;
    max-width: calc(100% - 32px);
    background: var(--ink);
    border: 1px solid rgb(5 217 232 / 0.25);
    border-radius: 14px;
    box-shadow: 0 0 40px rgb(255 42 109 / 0.2);
    overflow: hidden;
  }
  .field {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 18px;
  }
  .field .icon {
    color: var(--cyan);
    font-size: 18px;
  }
  input {
    flex: 1;
    min-width: 0;
    font: inherit;
    font-size: 20px;
    font-weight: 500;
    color: var(--text);
    background: none;
    border: 0;
    outline: none;
  }
  input::placeholder {
    color: var(--dim);
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
    max-height: 420px;
    overflow-y: auto;
    border-top: 1px solid rgb(5 217 232 / 0.14);
    scrollbar-width: thin;
  }
  li button {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 9px 18px;
    text-align: left;
  }
  li button.on {
    background: rgb(255 42 109 / 0.18);
  }
  .kind {
    flex: none;
    width: 70px;
  }
  .title {
    font-size: 17px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .none {
    margin: 0;
    padding: 14px 18px;
    border-top: 1px solid rgb(5 217 232 / 0.14);
    font-size: 15px;
    color: var(--dim);
  }
</style>
