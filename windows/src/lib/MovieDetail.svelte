<script lang="ts">
  import { onMount } from 'svelte';
  import { fade } from 'svelte/transition';
  import { ANDROID, t } from './i18n.svelte';
  import { library } from './library.svelte';
  import type { Info, MovieItem } from './models';
  import { nav } from './nav.svelte';
  import NeonText from './NeonText.svelte';
  import { player } from './player.svelte';
  import { store } from './store.svelte';
  import { metaParts, openTrailer } from './titleInfo';

  let { item, close }: { item: MovieItem; close: () => void } = $props();

  let info = $state.raw<Info | null>(null);
  let loading = $state(true);
  let play = $state<HTMLButtonElement>();

  $effect(() => {
    const current = item;
    loading = true;
    info = null;
    store
      .movieInfo(current)
      .then((i) => {
        if (current === item) info = i;
      })
      .catch((e) => console.warn(`movie info ${current.movie.name}: ${e}`))
      .finally(() => (loading = false));
  });

  const started = $derived(library.startAt(item.id) > 0);
  const saved = $derived(library.myList.has(item.id));
  const plot = $derived(info?.plot ?? item.movie.plot);
  const rating = $derived(info?.rating ?? item.movie.rating);
  const meta = $derived(metaParts(info));
  const backdrop = $derived(info?.backdrop ?? item.movie.poster);

  onMount(() => play?.focus());

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && !e.defaultPrevented && !player.expanded && !nav.searching) {
      e.preventDefault();
      close();
    }
  }
</script>

<svelte:window {onkeydown} />

<div class="detail" class:tv={ANDROID} transition:fade={{ duration: 200 }}>
  {#if backdrop}<img class="blur" src={backdrop} alt="" referrerpolicy="no-referrer" />{/if}
  <div class="body">
    <div class="cover">{#if item.movie.poster}<img src={item.movie.poster} alt="" referrerpolicy="no-referrer" />{/if}</div>
    <div class="info">
      <button class="back" onclick={close}><span class="icon">&#xE76B;</span> {t('Back')}</button>
      <NeonText text={item.movie.name} size={44} lines={2} />
      <div class="meta">
        {#if rating && rating > 0}<span class="mono">★ {rating.toFixed(1)}</span>{/if}
        {#each meta as part (part)}<span>{part}</span>{/each}
        {#if library.progress[item.id]?.watched}<span>{t('Watched')}</span>{/if}
      </div>
      <div class="actions">
        <button class="act accent" bind:this={play} onclick={() => store.playMovie(item)}><span class="icon">&#xE768;</span>{started ? t('Resume') : t('Play')}</button>
        {#if started}<button class="act" onclick={() => store.playMovie(item, true)}>{t('Play from the start')}</button>{/if}
        {#if info?.trailer}<button class="act" onclick={() => info?.trailer && void openTrailer(info.trailer).catch((e) => player.notify(String(e)))}>{t('Trailer')}</button>{/if}
        <button class="act" class:on={saved} aria-pressed={saved} onclick={() => library.toggleHidden(library.myList, item.id)}>{t('My list')}</button>
      </div>
      {#if plot}<p class="plot">{plot}</p>{:else if loading}<p class="plot dim mono">{t('Loading…')}</p>{/if}
      {#if info?.director}<p class="credit"><b>{t('Director')}</b> {info.director}</p>{/if}
      {#if info?.cast}<p class="credit"><b>{t('Cast')}</b> {info.cast}</p>{/if}
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
    width: 240px;
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
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 14px;
    scrollbar-width: none;
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
    gap: 6px 16px;
    font-size: 15px;
    color: var(--dim);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
  }
  .act {
    height: 40px;
    box-sizing: border-box;
    padding: 0 18px;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    border-radius: 8px;
    border: 1.5px solid rgb(233 236 242 / 0.25);
    font-size: 16px;
    font-weight: 600;
    color: var(--text);
  }
  .act.accent {
    background: var(--accent);
    border-color: var(--accent);
    color: #0a0c11;
  }
  .act.on {
    border-color: var(--magenta);
  }
  .act:hover,
  .act:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .plot {
    margin: 0;
    max-width: 760px;
    font-size: 16px;
    line-height: 1.5;
    color: var(--dim);
  }
  .plot.dim {
    font-size: 13px;
  }
  .credit {
    margin: 0;
    max-width: 760px;
    font-size: 14px;
    line-height: 1.4;
    color: var(--dim);
  }
  .credit b {
    margin-right: 8px;
    color: var(--text);
  }
</style>
