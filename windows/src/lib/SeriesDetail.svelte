<script lang="ts">
  import { ANDROID, t } from './i18n.svelte';
  import { onMount, tick } from 'svelte';
  import { fade } from 'svelte/transition';
  import { contextMenu } from './menuState.svelte';
  import HudPanel from './HudPanel.svelte';
  import { episodeKey, library } from './library.svelte';
  import type { Episode, Info, SeriesItem } from './models';
  import { nav } from './nav.svelte';
  import NeonText from './NeonText.svelte';
  import { player } from './player.svelte';
  import { store } from './store.svelte';
  import { metaParts, openTrailer } from './titleInfo';

  let { item, close }: { item: SeriesItem; close: () => void } = $props();

  let episodes = $state.raw<Episode[]>([]);
  let season = $state(1);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let info = $state.raw<Info | null>(null);
  const plot = $derived(info?.plot ?? item.series.plot);
  const meta = $derived(metaParts(info));

  const seasons = $derived([...new Set(episodes.map((e) => e.season))].sort((a, b) => a - b));
  const list = $derived(episodes.filter((e) => e.season === season));

  $effect(() => {
    const current = item;
    loading = true;
    error = null;
    store
      .episodes(current)
      .then((list) => {
        if (current !== item) return;
        episodes = list;
        season = list.length ? Math.min(...list.map((e) => e.season)) : 1;
      })
      .catch((e) => (error = String(e)))
      .finally(() => {
        loading = false;
        // TV: with the episodes in, the remote starts on the first one, unless it has already moved.
        void tick().then(() => {
          if (!ANDROID) return;
          const now = document.activeElement;
          if (!now || now === document.body || now.classList.contains('back')) document.querySelector<HTMLElement>('.detail .episode .main')?.focus();
        });
      });
  });

  onMount(() => {
    if (ANDROID) document.querySelector<HTMLElement>('.detail .back')?.focus();
  });

  $effect(() => {
    const current = item;
    info = null;
    store
      .seriesInfo(current)
      .then((i) => {
        if (current === item) info = i;
      })
      .catch((e) => console.warn(`series info ${current.series.name}: ${e}`));
  });

  function toggleWatched(e: Episode) {
    const key = episodeKey(item.id, e);
    library.setWatched(key, { kind: "episode", series: item.id, episode: e }, !library.progress[key]?.watched);
  }

  function menu(ev: MouseEvent, e: Episode) {
    const key = episodeKey(item.id, e);
    const started = library.startAt(key) > 0;
    contextMenu.open(ev, [
      { label: started ? t('Resume') : t('Play'), icon: "", action: () => void store.playEpisode(item, e, episodes) },
      ...(started ? [{ label: t('Play from the start'), icon: "", action: () => void store.playEpisode(item, e, episodes, true) }] : []),
      library.progress[key]?.watched
        ? { label: t('Mark as unwatched'), icon: "", action: () => toggleWatched(e) }
        : { label: t('Mark as watched'), icon: "", action: () => toggleWatched(e) },
      { label: t('Mark the season as watched'), icon: "", action: () => markSeason(e.season) },
    ]);
  }

  function markSeason(s: number) {
    for (const e of episodes.filter((x) => x.season === s)) {
      library.setWatched(episodeKey(item.id, e), { kind: "episode", series: item.id, episode: e }, true);
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && !e.defaultPrevented && !player.expanded && !nav.searching) {
      e.preventDefault();
      close();
    }
  }
</script>

<svelte:window {onkeydown} />

<div class="detail" class:tv={ANDROID} transition:fade={{ duration: 200 }}>
  {#if item.series.poster}<img class="blur" src={item.series.poster} alt="" referrerpolicy="no-referrer" />{/if}
  <div class="body">
    <div class="cover">
      {#if item.series.poster}<img src={item.series.poster} alt="" referrerpolicy="no-referrer" />{/if}
    </div>
    <div class="info">
      <button class="back" onclick={close}><span class="icon">&#xE76B;</span> {t('Back')}</button>
      <NeonText text={item.series.name} size={48} lines={2} />
      <div class="meta">
        {#if info?.rating ?? item.series.rating}<span class="mono">★ {(info?.rating ?? item.series.rating ?? 0).toFixed(1)}</span>{/if}
        {#each meta as part (part)}<span>{part}</span>{/each}
        {#if info?.trailer}<button class="trailer" onclick={() => info?.trailer && void openTrailer(info.trailer).catch((e) => player.notify(String(e)))}>{t('Trailer')}</button>{/if}
      </div>
      {#if plot}<p class="plot">{plot}</p>{/if}
      {#if info?.cast}<p class="credit"><b>{t('Cast')}</b> {info.cast}</p>{/if}
      {#if seasons.length > 1}
        <div class="seasons">
          {#each seasons as s (s)}
            <button class:on={s === season} onclick={() => (season = s)}>{t('Season {n}', { n: s })}</button>
          {/each}
        </div>
      {/if}
      <HudPanel caption={t('Episodes')} trailing={loading ? '' : String(list.length)} class="episodes">
        <div class="list">
          {#if loading}
            <p class="note mono">{t('Loading…')}</p>
          {:else if error}
            <p class="note mono error">{error}</p>
          {:else if !episodes.length}
            <p class="note">{t('No episodes.')}</p>
          {/if}
          {#each list as e (e.id)}
            {@const p = library.progress[episodeKey(item.id, e)]}
            <div class="episode" class:watched={p?.watched}>
              <button class="main" onclick={() => store.playEpisode(item, e, episodes)} oncontextmenu={(ev) => menu(ev, e)}>
                <span class="num mono">{e.number}</span>
                <span class="text">
                  <b>{e.title}</b>
                  {#if e.plot}<small>{e.plot}</small>{/if}
                  {#if p && !p.watched && p.position > 30}<span class="bar"><span style:width="{(p.position / p.duration) * 100}%"></span></span>{/if}
                </span>
                <span class="icon play">&#xE768;</span>
              </button>
              <button class="icon seen" title={p?.watched ? t('Mark as unwatched') : t('Mark as watched')} onclick={() => toggleWatched(e)}>&#xE73E;</button>
            </div>
          {/each}
        </div>
      </HudPanel>
    </div>
  </div>
</div>

<style>
  .detail {
    position: absolute;
    inset: 61px 0 0 85px;
    overflow: hidden;
  }
  .detail.tv {
    inset: 0 0 0 85px;
  }
  .blur {
    position: absolute;
    inset: -60px;
    width: calc(100% + 120px);
    height: calc(100% + 120px);
    object-fit: cover;
    filter: blur(40px);
    opacity: 0.45;
  }
  .detail::before {
    content: '';
    position: absolute;
    inset: 0;
    z-index: 1;
    background: rgb(10 12 17 / 0.55);
  }
  .body {
    position: relative;
    z-index: 2;
    height: 100%;
    box-sizing: border-box;
    display: flex;
    align-items: flex-start;
    gap: 40px;
    padding: 24px 40px 32px;
  }
  .cover {
    flex: none;
    width: 260px;
    aspect-ratio: 2 / 3;
    border-radius: 8px;
    overflow: hidden;
    background: var(--night);
  }
  .cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .info {
    flex: 1;
    min-width: 0;
    max-height: 100%;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .back {
    align-self: flex-start;
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 16px;
    font-weight: 600;
    color: var(--cyan);
  }
  .back .icon {
    font-size: 12px;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 16px;
    font-size: 15px;
    color: var(--dim);
  }
  .trailer {
    height: 32px;
    padding: 0 14px;
    border-radius: 6px;
    border: 1.5px solid rgb(233 236 242 / 0.25);
    font-size: 15px;
    font-weight: 600;
    color: var(--text);
  }
  .trailer:hover,
  .trailer:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .credit {
    margin: 0;
    max-width: 700px;
    font-size: 14px;
    line-height: 1.4;
    color: var(--dim);
  }
  .credit b {
    margin-right: 8px;
    color: var(--text);
  }
  .plot {
    margin: 0;
    max-width: 700px;
    font-size: 16px;
    line-height: 1.45;
    color: var(--dim);
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 4;
    line-clamp: 4;
    overflow: hidden;
  }
  .seasons {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .seasons button {
    height: 30px;
    box-sizing: border-box;
    padding: 0 12px;
    line-height: 17px;
    border-radius: 5px;
    border: 1.5px solid rgb(233 236 242 / 0.2);
    font-size: 16px;
    font-weight: 600;
    color: var(--dim);
  }
  .seasons button:hover {
    color: var(--text);
  }
  .seasons button.on {
    color: #fff;
    border-color: var(--magenta);
  }
  .info :global(.episodes) {
    flex: 0 1 auto;
    max-width: 900px;
  }
  .list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    scrollbar-width: thin;
    scrollbar-color: rgb(233 236 242 / 0.15) transparent;
  }
  .note {
    margin: 0;
    padding: 16px;
    font-size: 13px;
    color: var(--dim);
  }
  .note.error {
    color: var(--live);
  }
  .episode {
    display: flex;
    align-items: center;
  }
  .episode .main {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 10px 16px;
    text-align: left;
  }
  .episode:hover,
  .episode:focus-within {
    background: rgb(233 236 242 / 0.05);
  }
  .episode.watched .text {
    opacity: 0.55;
  }
  .seen {
    flex: none;
    margin-right: 10px;
    padding: 6px;
    border-radius: 50%;
    font-size: 13px;
    color: rgb(233 236 242 / 0.25);
  }
  .seen:hover {
    color: var(--text);
  }
  .watched .seen {
    color: var(--acid);
  }
  .bar {
    display: block;
    height: 3px;
    margin-top: 4px;
    max-width: 320px;
    background: rgb(233 236 242 / 0.12);
    border-radius: 2px;
  }
  .bar span {
    display: block;
    height: 100%;
    border-radius: 2px;
    background: var(--magenta);
  }
  .num {
    flex: none;
    width: 24px;
    font-size: 12px;
    color: var(--dim);
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .text b,
  .text small {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .text b {
    font-size: 17px;
    font-weight: 600;
  }
  .text small {
    font-size: 14px;
    color: var(--dim);
  }
  .play {
    color: var(--magenta);
    font-size: 14px;
  }
</style>
