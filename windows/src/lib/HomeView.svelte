<script lang="ts">
  import { t } from './i18n.svelte';
  import ChannelLogo from './ChannelLogo.svelte';
  import { library, listLabel } from './library.svelte';
  import type { LiveItem } from './models';
  import { nav } from './nav.svelte';
  import NeonText from './NeonText.svelte';
  import { player } from './player.svelte';
  import Poster from './Poster.svelte';
  import ProgressLine from './ProgressLine.svelte';
  import { store } from './store.svelte';
  import { clock, displayName, hm, neon, progress } from './theme';

  /** Where the app opens: back to the last channel, the recent ones, the favourites, what was left half-watched, what is coming. */
  const byId = $derived(new Map(store.lineup.flatMap((row) => [row, ...(row.variants ?? [])].map((c) => [c.id, row] as const))));
  const recent = $derived([...new Set(library.recents.flatMap((id) => byId.get(id) ?? []))]);
  const last = $derived(recent[0]);
  const favorites = $derived.by(() => {
    const list = library.favoriteLists[0]?.items ?? [];
    return [...new Set(list.flatMap((id) => byId.get(id) ?? []))].slice(0, 12);
  });
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

  function watch(item: LiveItem) {
    nav.focusChannel = item.id;
    nav.section = 'live';
  }

  function watchFullscreen(item: LiveItem) {
    void store.tune(item).then(() => player.expand());
  }
</script>

<div class="home">
  {#if last}
    {@const np = store.nowNext(last.channel)}
    {@const c = neon(last.channel.name)}
    <button class="hero" style:--c={c} onclick={() => watchFullscreen(last)} title={t('Watch fullscreen')}>
      <ChannelLogo channel={last.channel} color={c} size={84} />
      <div class="hero-text">
        <span class="label mono">{t('Back to')}</span>
        <NeonText text={displayName(last.channel.name)} size={44} />
        <span class="now">{np.now ? `${hm(np.now.start)} · ${np.now.title}` : (last.channel.group ?? '')}</span>
        {#if np.now}<div class="bar"><ProgressLine value={progress(np.now, store.now)} color={c} height={3} /></div>{/if}
      </div>
      <span class="play"><span class="icon">&#xE768;</span> {t('Watch')}</span>
    </button>
  {:else}
    <div class="hello">
      <NeonText text="Welcome" size={44} />
      <p>{t('Pick a channel in Live: the last one you watch comes back here.')}</p>
      <button class="go" onclick={() => (nav.section = 'live')}>{t('Open Live')}</button>
    </div>
  {/if}

  {#snippet channelCards(title: string, items: LiveItem[])}
    <section>
      <h3 class="mono">{title}</h3>
      <div class="strip">
        {#each items as item (item.id)}
          {@const np = store.nowNext(item.channel)}
          {@const c = neon(item.channel.name)}
          <button class="card" style:--c={c} onclick={() => watch(item)} ondblclick={() => watchFullscreen(item)}>
            <ChannelLogo channel={item.channel} color={c} size={40} />
            <b>{displayName(item.channel.name)}</b>
            <small>{np.now?.title ?? item.channel.group ?? ''}</small>
            <ProgressLine value={np.now ? progress(np.now, store.now) : 0} color={c} />
          </button>
        {/each}
      </div>
    </section>
  {/snippet}

  {#if recent.length > 1}{@render channelCards(t('Recent'), recent.slice(1, 13))}{/if}
  {#if favorites.length}{@render channelCards(listLabel(library.favoriteLists[0]), favorites)}{/if}

  {#if continuing.length}
    <section>
      <h3 class="mono">{t('Continue watching')}</h3>
      <div class="strip posters">
        {#each continuing as c (c.id)}
          <div class="poster"><Poster title={c.title} subtitle={c.subtitle} url={c.poster} rating={null} progress={c.progress} onclick={c.play} /></div>
        {/each}
      </div>
    </section>
  {/if}

  {#if upcoming.length}
    <section>
      <h3 class="mono">{t('Reminders')}</h3>
      <ul class="reminders">
        {#each upcoming as r (r.channel + r.start)}
          <li>
            <span class="mono time">{hm(r.start)}</span>
            <b>{r.title}</b>
            <small>{r.channelName}</small>
          </li>
        {/each}
      </ul>
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
    gap: 28px;
    padding: 12px 28px 28px;
    scrollbar-width: thin;
    scrollbar-color: rgb(243 238 255 / 0.15) transparent;
  }
  .hero {
    display: flex;
    align-items: center;
    gap: 28px;
    padding: 26px 30px;
    border-radius: 18px;
    text-align: left;
    background: linear-gradient(110deg, color-mix(in srgb, var(--c) 22%, transparent), transparent 70%);
    outline: 1.5px solid color-mix(in srgb, var(--c) 60%, transparent);
    box-shadow: 0 0 22px color-mix(in srgb, var(--c) 25%, transparent);
    transition: box-shadow 0.2s;
  }
  .hero:hover,
  .hero:focus-visible {
    box-shadow: 0 0 34px color-mix(in srgb, var(--c) 45%, transparent);
  }
  .hero-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .label {
    font-size: 11px;
    color: var(--acid);
  }
  .now {
    font-size: 19px;
    font-weight: 600;
    color: var(--dim);
  }
  .bar {
    max-width: 480px;
  }
  .play {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 22px;
    border-radius: 999px;
    background: var(--c);
    color: var(--ink);
    font-size: 18px;
    font-weight: 700;
  }
  .hello {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 12px;
    padding: 26px 30px;
  }
  .hello p {
    margin: 0;
    color: var(--dim);
    font-size: 17px;
  }
  .go {
    padding: 9px 20px;
    border-radius: 999px;
    border: 1px solid var(--cyan);
    color: var(--cyan);
    font-weight: 600;
  }
  section {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  h3 {
    margin: 0;
    font-size: 12px;
    font-weight: 500;
    color: var(--acid);
  }
  .strip {
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: 230px;
    gap: 16px;
    overflow-x: auto;
    padding: 6px 4px 10px;
    scrollbar-width: thin;
    scrollbar-color: rgb(243 238 255 / 0.15) transparent;
  }
  .strip.posters {
    grid-auto-columns: 170px;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 14px;
    border-radius: 12px;
    text-align: left;
    outline: 1px solid rgb(243 238 255 / 0.12);
    transition: outline-color 0.15s, box-shadow 0.2s;
  }
  .card:hover,
  .card:focus-visible {
    outline: 1.5px solid var(--c);
    box-shadow: 0 0 14px color-mix(in srgb, var(--c) 35%, transparent);
  }
  .card b,
  .card small {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .card b {
    font-size: 17px;
  }
  .card small {
    font-size: 14px;
    color: var(--dim);
  }
  .reminders {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .reminders li {
    display: flex;
    align-items: baseline;
    gap: 14px;
    font-size: 16px;
  }
  .time {
    color: var(--acid);
    font-size: 13px;
  }
  .reminders small {
    color: var(--dim);
  }
</style>
