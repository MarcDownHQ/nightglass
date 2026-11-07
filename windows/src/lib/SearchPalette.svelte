<script lang="ts">
  import { t } from './i18n.svelte';
  import { fade } from 'svelte/transition';
  import { library } from './library.svelte';
  import type { LiveItem, MovieItem, Programme, SeriesItem } from './models';
  import { nav } from './nav.svelte';
  import { player } from './player.svelte';
  import { store } from './store.svelte';
  import Tag from './Tag.svelte';
  import { displayName, hm } from './theme';

  /** Ctrl+K: one field across channels, guide programmes, movies and series. */
  type Kind = 'Live' | 'TV' | 'Movie' | 'Series';
  type Hit =
    | { kind: 'Live'; key: string; item: LiveItem; title: string; detail?: string }
    | { kind: 'TV'; key: string; item: LiveItem; programme: Programme; title: string; detail: string }
    | { kind: 'Movie'; key: string; item: MovieItem; title: string; detail?: string }
    | { kind: 'Series'; key: string; item: SeriesItem; title: string; detail?: string }
    | { kind: 'More'; key: string; of: Kind; count: number; title: string; detail?: string };

  const COLORS: Record<Kind | 'More', string> = {
    Live: 'var(--magenta)',
    TV: 'var(--cyan)',
    Movie: 'var(--cyan)',
    Series: 'var(--acid)',
    More: 'var(--dim)',
  };
  const LIMITS: Record<Kind, number> = { Live: 12, TV: 8, Movie: 10, Series: 10 };
  /** "Show all": a long list still stays bounded. */
  const ALL = 400;

  let query = $state('');
  let index = $state(0);
  /** The kinds the user asked to see in full. */
  let expanded = $state<Kind[]>([]);
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

  /** The matches, up to the kind's limit, and how many there are in all. */
  function matches<T>(kind: Kind, items: Iterable<T>, name: (i: T) => string, q: string): { list: T[]; count: number } {
    const limit = expanded.includes(kind) ? ALL : LIMITS[kind];
    const list: T[] = [];
    let count = 0;
    for (const i of items) {
      if (!fold(name(i)).includes(q)) continue;
      count++;
      if (list.length < limit) list.push(i);
    }
    return { list, count };
  }

  /** Programmes on now, coming, or past and replayable. */
  function* programmes(): Generator<{ item: LiveItem; p: Programme }> {
    const now = store.now;
    for (const item of store.lineup) {
      for (const p of store.programmes(item.channel)) {
        if (p.end > now || store.canReplay(item, p)) yield { item, p };
      }
    }
  }

  function group<T>(kind: Kind, found: { list: T[]; count: number }, hit: (t: T) => Hit): Hit[] {
    const out = found.list.map(hit);
    if (found.count > found.list.length) {
      out.push({ kind: 'More', key: `more-${kind}`, of: kind, count: found.count, title: t('Show all {n}', { n: found.count }) });
    }
    return out;
  }

  const hits = $derived.by((): Hit[] => {
    const q = fold(query.trim());
    if (!q) return [];
    const now = store.now;
    return [
      ...group('Live', matches('Live', store.channels, (i) => i.channel.name, q), (item) => ({
        kind: 'Live',
        key: item.id,
        item,
        title: displayName(item.channel.name),
      })),
      ...group('TV', matches('TV', programmes(), (x) => x.p.title, q), ({ item, p }) => ({
        kind: 'TV',
        key: `${item.id}@${p.start}`,
        item,
        programme: p,
        title: p.title,
        detail: `${p.start <= now && p.end > now ? t('now') : p.end <= now ? t('replay') : new Date(p.start * 1000).toDateString() === new Date(now * 1000).toDateString() ? hm(p.start) : new Date(p.start * 1000).toLocaleDateString([], { weekday: 'short' }) + ' ' + hm(p.start)} · ${displayName(item.channel.name)}`,
      })),
      ...group('Movie', matches('Movie', store.movies, (i) => i.movie.name, q), (item) => ({ kind: 'Movie', key: item.id, item, title: item.movie.name })),
      ...group('Series', matches('Series', store.series, (i) => i.series.name, q), (item) => ({
        kind: 'Series',
        key: item.id,
        item,
        title: item.series.name,
      })),
    ];
  });

  $effect(() => {
    void query;
    index = 0;
    expanded = [];
  });

  $effect(() => {
    listEl?.children[index]?.scrollIntoView({ block: 'nearest' });
  });

  $effect(() => input.focus());

  function open(hit: Hit) {
    if (hit.kind === 'More') {
      expanded = [...expanded, hit.of];
      return;
    }
    nav.searching = false;
    switch (hit.kind) {
      case 'Live':
        void player.collapse();
        nav.focusChannel = hit.item.id;
        nav.section = 'live';
        break;
      case 'TV': {
        const p = hit.programme;
        if (p.end <= store.now) void store.playReplay(hit.item, p);
        else if (p.start > store.now) {
          library.toggleReminder({ channel: hit.item.id, channelName: displayName(hit.item.channel.name), title: p.title, start: p.start, end: p.end });
          player.notify(library.hasReminder(hit.item.id, p.start) ? t('Reminder set for {time}', { time: hm(p.start) }) : t('Reminder cancelled'));
        } else {
          void player.collapse();
          nav.focusChannel = hit.item.id;
          nav.section = 'live';
        }
        break;
      }
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
  <div class="palette" role="dialog" aria-label={t('Search')}>
    <label class="field">
      <span class="icon">&#xE721;</span>
      <input bind:this={input} bind:value={query} {onkeydown} placeholder={t('Channels, programmes, movies, series')} spellcheck="false" />
    </label>
    {#if hits.length}
      <ul bind:this={listEl} role="listbox">
        {#each hits as hit, i (hit.kind + hit.key)}
          <li role="option" aria-selected={i === index}>
            <button class:on={i === index} class:more={hit.kind === 'More'} onclick={() => open(hit)} onmousemove={() => (index = i)}>
              <span class="kind">{#if hit.kind !== 'More'}<Tag text={hit.kind === 'Live' ? t('Live') : hit.kind === 'Movie' ? t('Movie') : hit.kind === 'Series' ? t('Series') : t('TV')} color={COLORS[hit.kind]} />{/if}</span>
              <span class="title">{hit.title}</span>
              {#if hit.detail}<span class="detail mono">{hit.detail}</span>{/if}
            </button>
          </li>
        {/each}
      </ul>
    {:else if query.trim()}
      <p class="none">{t('Nothing matches.')}</p>
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
  .detail {
    flex: none;
    max-width: 45%;
    font-size: 12px;
    color: var(--dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  li button.more .title {
    font-size: 14px;
    font-weight: 500;
    color: var(--cyan);
  }
  .title {
    flex: 1;
  }
  .none {
    margin: 0;
    padding: 14px 18px;
    border-top: 1px solid rgb(5 217 232 / 0.14);
    font-size: 15px;
    color: var(--dim);
  }
</style>
