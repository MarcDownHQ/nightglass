<script lang="ts">
  import { t } from './i18n.svelte';
  import { fade } from 'svelte/transition';
  import { contextMenu } from './menuState.svelte';
  import HudPanel from './HudPanel.svelte';
  import { episodeKey, library } from './library.svelte';
  import type { Episode, SeriesItem } from './models';
  import { nav } from './nav.svelte';
  import NeonText from './NeonText.svelte';
  import { player } from './player.svelte';
  import { store } from './store.svelte';

  let { item, close }: { item: SeriesItem; close: () => void } = $props();

  let episodes = $state.raw<Episode[]>([]);
  let season = $state(1);
  let loading = $state(true);
  let error = $state<string | null>(null);

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
      .finally(() => (loading = false));
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

<div class="detail" transition:fade={{ duration: 200 }}>
  {#if item.series.poster}<img class="blur" src={item.series.poster} alt="" referrerpolicy="no-referrer" />{/if}
  <div class="body">
    <div class="cover">
      {#if item.series.poster}<img src={item.series.poster} alt="" referrerpolicy="no-referrer" />{/if}
    </div>
    <div class="info">
      <button class="back" onclick={close}><span class="icon">&#xE76B;</span> {t('Back')}</button>
      <NeonText text={item.series.name} size={48} lines={2} />
      {#if item.series.plot}<p class="plot">{item.series.plot}</p>{/if}
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
    color: var(--magenta);
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
