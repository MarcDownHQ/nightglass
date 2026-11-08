<script module lang="ts">
  import type { LiveItem, MovieItem, Programme, SeriesItem } from './models';
  import { column, type Column, type Stale } from './search';
  import { store } from './store.svelte';

  /** Folded names of everything searchable: built once, kept while the lists stay the same. */
  interface Index {
    from: unknown[];
    live: Column<LiveItem>;
    tv: Column<{ item: LiveItem; p: Programme }>;
    movies: Column<MovieItem>;
    series: Column<SeriesItem>;
  }
  let cached: Index | null = null;
  let building: Promise<Index | null> | null = null;

  function getIndex(closed: Stale): Promise<Index | null> {
    const from = [store.channels, store.lineup, store.movies, store.series, store.guide];
    if (cached && cached.from.every((x, i) => x === from[i])) return Promise.resolve(cached);
    building ??= build(from, closed).finally(() => (building = null));
    return building;
  }

  async function build(from: unknown[], closed: Stale): Promise<Index | null> {
    const lineup = store.lineup;
    const programmes: { item: LiveItem; p: Programme }[] = [];
    for (let i = 0; i < lineup.length; i++) {
      for (const p of store.programmes(lineup[i].channel)) programmes.push({ item: lineup[i], p });
      if (i % 2000 === 1999) {
        await new Promise((r) => setTimeout(r));
        if (closed()) return null;
      }
    }
    const live = await column(store.channels, (i) => i.channel.name, closed);
    const tv = live && (await column(programmes, (x) => x.p.title, closed));
    const movies = tv && (await column(store.movies, (i) => i.movie.name, closed));
    const series = movies && (await column(store.series, (i) => i.series.name, closed));
    if (!live || !tv || !movies || !series) return null;
    cached = { from, live, tv, movies, series };
    return cached;
  }
</script>

<script lang="ts">
  import { t } from './i18n.svelte';
  import { onDestroy } from 'svelte';
  import { fade } from 'svelte/transition';
  import ChannelLogo from './ChannelLogo.svelte';
  import { library } from './library.svelte';
  import { categoryLabel, fold } from './lineup';
  import { nav } from './nav.svelte';
  import { player } from './player.svelte';
  import { find, type Found } from './search';
  import Tag from './Tag.svelte';
  import { displayName, hm, qualityTag } from './theme';

  /** Ctrl+K: one field across channels, guide programmes, movies and series. Typing never waits for the search. */
  type Kind = 'Live' | 'TV' | 'Movie' | 'Series';
  /** `sub`: a second line (number, category, what is on); `badge`: the picture quality. */
  type Hit = { key: string; title: string; detail?: string; sub?: string; badge?: string | null } & (
    | { kind: 'Live'; item: LiveItem }
    | { kind: 'TV'; item: LiveItem; programme: Programme }
    | { kind: 'Movie'; item: MovieItem }
    | { kind: 'Series'; item: SeriesItem }
    | { kind: 'More'; of: Kind; count: number }
  );

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

  interface Results {
    live: Found<LiveItem>;
    tv: Found<{ item: LiveItem; p: Programme }>;
    movies: Found<MovieItem>;
    series: Found<SeriesItem>;
  }
  /** The last search that finished: shown until the next one has. */
  let results = $state.raw<Results | null>(null);
  let busy = $state(false);
  /** Each search has a number; an older one stops as soon as a newer began. */
  let run = 0;
  let closed = false;
  onDestroy(() => (closed = true));

  $effect(() => {
    const q = fold(query.trim());
    const kinds = [...expanded];
    const id = ++run;
    if (!q) {
      results = null;
      busy = false;
      return;
    }
    busy = true;
    // A short pause after the last key: typing a word runs one search, not one per letter.
    const timer = setTimeout(() => void search(q, kinds, id), 120);
    return () => clearTimeout(timer);
  });

  async function search(q: string, kinds: Kind[], id: number) {
    const stale = () => closed || id !== run;
    const ix = await getIndex(() => closed);
    if (!ix || stale()) return;
    const limit = (k: Kind) => (kinds.includes(k) ? ALL : LIMITS[k]);
    const now = store.now;
    const live = await find(ix.live, q, limit('Live'), stale);
    // Programmes on now, coming, or past and replayable.
    const tv = live && (await find(ix.tv, q, limit('TV'), stale, ({ item, p }) => p.end > now || store.canReplay(item, p)));
    const movies = tv && (await find(ix.movies, q, limit('Movie'), stale));
    const series = movies && (await find(ix.series, q, limit('Series'), stale));
    if (!live || !tv || !movies || !series) return;
    results = { live, tv, movies, series };
    busy = false;
  }

  function group<T>(kind: Kind, found: Found<T>, hit: (t: T) => Hit): Hit[] {
    const out = found.list.map(hit);
    if (found.count > found.list.length) {
      out.push({ kind: 'More', key: `more-${kind}`, of: kind, count: found.count, title: t('Show all {n}', { n: found.count }) });
    }
    return out;
  }

  const join = (...parts: (string | null | undefined | false)[]) => parts.filter(Boolean).join(' · ');
  const stars = (rating: number | null) => (rating ? `★ ${rating.toFixed(1)}` : null);

  const hits = $derived.by((): Hit[] => {
    if (!results) return [];
    const now = store.now;
    // Several sources can carry the same name: say which one.
    const many = store.sources.length > 1;
    return [
      ...group('Live', results.live, (item) => ({
        kind: 'Live',
        key: item.id,
        item,
        title: displayName(item.channel.name),
        sub: join(item.channel.number !== null && String(item.channel.number), item.channel.group && categoryLabel(item.channel.group), store.nowNext(item.channel).now?.title),
        badge: qualityTag(item.channel.name),
        detail: many ? item.source.name : undefined,
      })),
      ...group('TV', results.tv, ({ item, p }) => ({
        kind: 'TV',
        key: `${item.id}@${p.start}`,
        item,
        programme: p,
        title: p.title,
        sub: join(displayName(item.channel.name), item.channel.group && categoryLabel(item.channel.group)),
        badge: qualityTag(item.channel.name),
        detail:
          p.start <= now && p.end > now
            ? t('now')
            : p.end <= now
              ? t('replay')
              : new Date(p.start * 1000).toDateString() === new Date(now * 1000).toDateString()
                ? hm(p.start)
                : new Date(p.start * 1000).toLocaleDateString([], { weekday: 'short' }) + ' ' + hm(p.start),
      })),
      ...group('Movie', results.movies, (item) => ({
        kind: 'Movie',
        key: item.id,
        item,
        title: item.movie.name,
        sub: join(item.movie.group && categoryLabel(item.movie.group), stars(item.movie.rating)),
        detail: many ? item.source.name : undefined,
      })),
      ...group('Series', results.series, (item) => ({
        kind: 'Series',
        key: item.id,
        item,
        title: item.series.name,
        sub: join(item.series.group && categoryLabel(item.series.group), stars(item.series.rating)),
        detail: many ? item.source.name : undefined,
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
      {#if busy}<span class="spinner" role="status" aria-label={t('Searching…')}></span>{/if}
    </label>
    {#if hits.length}
      <ul bind:this={listEl} role="listbox">
        {#each hits as hit, i (hit.kind + hit.key)}
          <li role="option" aria-selected={i === index}>
            <button class:on={i === index} class:more={hit.kind === 'More'} onclick={() => open(hit)} onmousemove={() => (index = i)}>
              <span class="kind">{#if hit.kind !== 'More'}<Tag text={hit.kind === 'Live' ? t('Live') : hit.kind === 'Movie' ? t('Movie') : hit.kind === 'Series' ? t('Series') : t('TV')} color={COLORS[hit.kind]} />{/if}</span>
              {#if hit.kind === 'Live' || hit.kind === 'TV'}<ChannelLogo channel={hit.item.channel} size={34} />{/if}
              <span class="text">
                <span class="title">{hit.title}</span>
                {#if hit.sub}<small>{hit.sub}</small>{/if}
              </span>
              {#if hit.badge}<Tag text={hit.badge} color="var(--soft)" />{/if}
              {#if hit.detail}<span class="detail mono">{hit.detail}</span>{/if}
            </button>
          </li>
        {/each}
      </ul>
    {:else if query.trim() && results && !busy}
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
    background: rgb(10 12 17 / 0.6);
  }
  .palette {
    width: 760px;
    max-width: calc(100% - 32px);
    background: var(--ink);
    border: 1px solid rgb(233 236 242 / 0.1);
    border-radius: 14px;
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
    font-weight: 600;
    color: var(--text);
    background: none;
    border: 0;
    outline: none;
  }
  input::placeholder {
    color: var(--dim);
  }
  .spinner {
    flex: none;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    border: 2px solid rgb(255 255 255 / 0.15);
    border-top-color: var(--accent);
    animation: spin 0.7s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
    max-height: 480px;
    overflow-y: auto;
    border-top: 1px solid rgb(233 236 242 / 0.06);
    scrollbar-width: thin;
  }
  li button {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 18px;
    text-align: left;
  }
  li button.on {
    background: rgb(143 168 255 / 0.18);
  }
  .kind {
    flex: none;
    width: 64px;
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .title {
    font-size: 16px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  small {
    font-size: 12px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .detail {
    flex: none;
    max-width: 30%;
    font-size: 12px;
    color: var(--dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  li button.more .title {
    font-size: 14px;
    color: var(--cyan);
  }
  .none {
    margin: 0;
    padding: 14px 18px;
    border-top: 1px solid rgb(233 236 242 / 0.06);
    font-size: 15px;
    color: var(--dim);
  }
</style>
