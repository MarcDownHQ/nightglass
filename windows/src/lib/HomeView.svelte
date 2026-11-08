<script lang="ts">
  import { t } from './i18n.svelte';
  import ChannelLogo from './ChannelLogo.svelte';
  import { library, listLabel } from './library.svelte';
  import type { LiveItem } from './models';
  import { nav } from './nav.svelte';
  import { player } from './player.svelte';
  import Poster from './Poster.svelte';
  import ProgressLine from './ProgressLine.svelte';
  import { store } from './store.svelte';
  import { clock, displayName, hm, progress } from './theme';

  /** Where the app opens: back to the last channel, what was left half-watched, the favourites, the recent ones, what is coming. */
  const byId = $derived(new Map(store.lineup.flatMap((row) => [row, ...(row.variants ?? [])].map((c) => [c.id, row] as const))));
  const recent = $derived([...new Set(library.recents.flatMap((id) => byId.get(id) ?? []))]);
  const last = $derived(recent[0]);
  const firstList = $derived(library.favoriteLists[0]);
  const favorites = $derived([...new Set((firstList?.items ?? []).flatMap((id) => byId.get(id) ?? []))].slice(0, 10));
  const movies = $derived(new Map(store.movies.map((m) => [m.id, m])));
  const shows = $derived(new Map(store.series.map((s) => [s.id, s])));
  const continuing = $derived(
    library.continuing.slice(0, 10).flatMap(([id, p]) => {
      const r = p.resume;
      if (r.kind === 'movie') {
        const m = movies.get(id);
        return m ? [{ id, title: m.movie.name, subtitle: t('{time} left', { time: clock(p.duration - p.position) }), poster: m.movie.poster, progress: p.position / p.duration, play: () => store.playMovie(m) }] : [];
      }
      const s = shows.get(r.series);
      return s
        ? [{ id, title: s.series.name, subtitle: `S${r.episode.season}E${r.episode.number} · ${r.episode.title}`, poster: s.series.poster, progress: p.position / p.duration, play: () => void store.playEpisode(s, r.episode) }]
        : [];
    }),
  );
  const upcoming = $derived(library.reminders.filter((r) => r.end > store.now).slice(0, 6));
  const greeting = $derived.by(() => {
    const h = new Date(store.now * 1000).getHours();
    return h >= 5 && h < 18 ? t('Hello') : t('Good evening');
  });
  const today = $derived(new Date(store.now * 1000).toLocaleDateString([], { weekday: 'long', day: 'numeric', month: 'long' }));

  /** In Live, with its preview: the channel plays there. */
  function watch(item: LiveItem) {
    nav.focusChannel = item.id;
    nav.section = 'live';
  }

  function watchFullscreen(item: LiveItem) {
    nav.focusChannel = item.id;
    nav.section = 'live';
    void store.tune(item).then(() => player.expand());
  }
</script>

{#snippet tile(item: LiveItem, removable: boolean)}
  {@const np = store.nowNext(item.channel)}
  <div class="tile">
    <div class="tile-head">
      <ChannelLogo channel={item.channel} size={40} />
      <button class="tile-name" onclick={() => watch(item)} ondblclick={() => watchFullscreen(item)}>{displayName(item.channel.name)}</button>
      {#if removable && firstList}
        <button class="star" title={t('Remove from favorites')} aria-label={t('Remove from favorites')} onclick={() => library.toggleFavorite(firstList.id, item.id)}>&#xE735;</button>
      {/if}
    </div>
    <small>{np.now?.title ?? item.channel.group ?? ''}</small>
    <ProgressLine value={np.now ? progress(np.now, store.now) : 0} />
  </div>
{/snippet}

<div class="home">
  <header>
    <h1>{greeting}</h1>
    <span class="today">{today}</span>
  </header>

  {#if last || continuing.length}
    <section>
      <h2>{t('Pick up where you left off')}</h2>
      <div class="resume" style:grid-template-columns={last && continuing.length ? 'minmax(0, 1.6fr) repeat(2, minmax(0, 0.55fr))' : last ? 'minmax(0, 760px)' : 'repeat(4, minmax(0, 1fr))'}>
        {#if last}
          {@const np = store.nowNext(last.channel)}
          <button class="hero" onclick={() => watch(last)} ondblclick={() => watchFullscreen(last)} title={t('Watch')}>
            <span class="hero-top">
              <ChannelLogo channel={last.channel} size={72} />
              <span class="live-badge"><span></span>{t('LIVE')}</span>
            </span>
            <span class="hero-bottom">
              <span class="hero-text">
                <span class="meta">{last.channel.number ? `${last.channel.number} · ` : ''}{displayName(last.channel.name)} · {t('last channel watched')}</span>
                <b>{np.now?.title ?? displayName(last.channel.name)}</b>
                {#if np.now}<span class="bar"><ProgressLine value={progress(np.now, store.now)} /></span>{/if}
              </span>
              <span class="play"><span class="icon">&#xE768;</span>{t('Watch')}</span>
            </span>
          </button>
        {/if}
        {#each continuing.slice(0, last ? 2 : 4) as c (c.id)}
          <div class="poster"><Poster title={c.title} subtitle={c.subtitle} url={c.poster} rating={null} progress={c.progress} onclick={c.play} /></div>
        {/each}
      </div>
    </section>
  {:else}
    <section class="hello">
      <h2>{t('Welcome')}</h2>
      <p>{t('Pick a channel in Live: the last one you watch comes back here.')}</p>
      <button class="primary" onclick={() => (nav.section = 'live')}>{t('Open Live')}</button>
    </section>
  {/if}

  {#if favorites.length}
    <section>
      <div class="section-head">
        <h2>{firstList ? listLabel(firstList) : ''}</h2>
        <span class="count mono">{favorites.length}</span>
      </div>
      <div class="tiles">
        {#each favorites as item (item.id)}{@render tile(item, true)}{/each}
      </div>
    </section>
  {:else if firstList}
    <section>
      <h2>{listLabel(firstList)}</h2>
      <p class="hint">{t('No favorite channel yet. In Live, click the star of a channel to find it here.')}</p>
    </section>
  {/if}

  {#if recent.length > 1}
    <section>
      <h2>{t('Recent')}</h2>
      <div class="tiles">
        {#each recent.slice(1, 6) as item (item.id)}{@render tile(item, false)}{/each}
      </div>
    </section>
  {/if}

  {#if continuing.length > (last ? 2 : 4)}
    <section>
      <h2>{t('Continue watching')}</h2>
      <div class="posters">
        {#each continuing.slice(last ? 2 : 4) as c (c.id)}
          <div class="poster small"><Poster title={c.title} subtitle={c.subtitle} url={c.poster} rating={null} progress={c.progress} onclick={c.play} /></div>
        {/each}
      </div>
    </section>
  {/if}

  {#if upcoming.length}
    <section class="panel">
      <h2>{t('Reminders')}</h2>
      {#each upcoming as r (r.channel + r.start)}
        <div class="reminder">
          <span class="mono time">{hm(r.start)}</span>
          <span class="what"><b>{r.title}</b><small>{r.channelName}</small></span>
          <button class="chip" onclick={() => library.toggleReminder(r)}>{t('Cancel')}</button>
        </div>
      {/each}
    </section>
  {/if}
</div>

<style>
  .home {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 30px;
    padding: 26px 36px 36px;
  }
  header {
    display: flex;
    align-items: baseline;
    gap: 16px;
  }
  h1 {
    margin: 0;
    font-size: 26px;
    font-weight: 700;
    letter-spacing: -0.015em;
  }
  .today {
    font-size: 14px;
    color: var(--muted);
    display: inline-block;
  }
  .today::first-letter {
    text-transform: uppercase;
  }
  section {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  h2 {
    margin: 0;
    font-size: 17px;
    font-weight: 600;
  }
  .section-head {
    display: flex;
    align-items: baseline;
    gap: 10px;
  }
  .count {
    font-size: 13px;
    color: var(--muted);
  }
  .resume {
    display: grid;
    grid-template-columns: minmax(0, 1.6fr) repeat(2, minmax(0, 0.55fr));
    gap: 16px;
    align-items: stretch;
  }
  .hero {
    display: flex;
    flex-direction: column;
    min-height: 260px;
    border-radius: 14px;
    overflow: hidden;
    background: var(--surface);
    border: 1px solid var(--border);
    text-align: left;
  }
  .hero:hover {
    background: #151922;
  }
  .hero-top {
    position: relative;
    flex: 1;
    display: grid;
    place-items: center;
    background: #1a2030;
  }
  .live-badge {
    position: absolute;
    top: 12px;
    left: 12px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 24px;
    padding: 0 9px;
    border-radius: 7px;
    background: rgb(10 12 17 / 0.75);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.05em;
    color: #ff9a8d;
  }
  .live-badge span {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--live);
  }
  .hero-bottom {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 14px 16px;
  }
  .hero-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .meta {
    font-size: 12px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .hero-text b {
    font-size: 16px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .bar {
    margin-top: 6px;
    max-width: 320px;
  }
  .play {
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 38px;
    padding: 0 14px;
    border-radius: 10px;
    background: var(--accent);
    color: var(--bg);
    font-size: 13px;
    font-weight: 600;
  }
  .play .icon {
    font-size: 12px;
  }
  .poster {
    min-width: 0;
  }
  .posters {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 16px;
  }
  .tiles {
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: 14px;
  }
  .tile {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px;
    border-radius: 14px;
    background: var(--surface);
    border: 1px solid var(--border);
    min-width: 0;
  }
  .tile:hover {
    background: #151922;
  }
  .tile-head {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .tile-name {
    flex: 1;
    min-width: 0;
    font-size: 14px;
    font-weight: 600;
    text-align: left;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tile-name:hover {
    color: var(--accent);
  }
  .tile small {
    min-height: 1lh;
    font-size: 13px;
    color: var(--soft);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .star {
    flex: none;
    width: 30px;
    height: 30px;
    border-radius: 8px;
    font-family: var(--icons);
    font-size: 14px;
    color: var(--fav);
  }
  .star:hover {
    background: var(--hover);
  }
  .hint {
    margin: 0;
    font-size: 14px;
    color: var(--muted);
  }
  .panel {
    gap: 4px;
    padding: 16px;
    border-radius: 14px;
    background: var(--surface);
    border: 1px solid var(--border);
  }
  .panel h2 {
    margin-bottom: 8px;
  }
  .reminder {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 6px 0;
  }
  .time {
    flex: none;
    width: 52px;
    font-size: 13px;
    font-weight: 600;
    color: var(--accent);
  }
  .what {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .what b {
    font-size: 14px;
    font-weight: 600;
  }
  .what small {
    font-size: 12px;
    color: var(--muted);
  }
  .chip {
    height: 32px;
    padding: 0 12px;
    border-radius: 9px;
    background: var(--raised);
    color: var(--soft);
    font-size: 13px;
    font-weight: 600;
  }
  .chip:hover {
    background: var(--hover);
    color: #fff;
  }
  .hello p {
    margin: 0;
    font-size: 15px;
    color: var(--muted);
  }
  .primary {
    align-self: flex-start;
    height: 40px;
    padding: 0 18px;
    border-radius: 10px;
    background: var(--accent);
    color: var(--bg);
    font-size: 14px;
    font-weight: 600;
  }
</style>
