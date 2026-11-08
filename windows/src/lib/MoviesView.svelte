<script lang="ts">
  import { t } from './i18n.svelte';
  import { contextMenu } from './menuState.svelte';
  import { library } from './library.svelte';
  import type { MovieItem } from './models';
  import PosterBrowser, { type ContinueCard } from './PosterBrowser.svelte';
  import { store } from './store.svelte';
  import { clock } from './theme';

  const byId = $derived(new Map(store.movies.map((m) => [m.id, m])));

  function status(i: MovieItem) {
    const p = library.progress[i.id];
    return { progress: p && !p.watched && p.duration ? p.position / p.duration : 0, watched: !!p?.watched };
  }

  function toggleMine(i: MovieItem) {
    library.toggleHidden(library.myList, i.id);
  }

  function menu(e: MouseEvent, i: MovieItem) {
    const resume = { kind: 'movie' as const, id: i.id };
    const started = library.startAt(i.id) > 0;
    contextMenu.open(e, [
      { label: started ? t('Resume') : t('Play'), icon: '', action: () => store.playMovie(i) },
      ...(started ? [{ label: t('Play from the start'), icon: '', action: () => store.playMovie(i, true) }] : []),
      library.progress[i.id]?.watched
        ? { label: t('Mark as unwatched'), icon: '', action: () => library.setWatched(i.id, resume, false) }
        : { label: t('Mark as watched'), icon: '', action: () => library.setWatched(i.id, resume, true) },
      { label: library.myList.has(i.id) ? t('Remove from my list') : t('Add to my list'), icon: '', action: () => toggleMine(i) },
    ]);
  }

  const continuing = $derived(
    library.continuing.flatMap(([id, p]): ContinueCard[] => {
      const m = p.resume.kind === 'movie' ? byId.get(id) : undefined;
      if (!m) return [];
      return [
        {
          id,
          title: m.movie.name,
          subtitle: t('{time} left', { time: clock(p.duration - p.position) }),
          poster: m.movie.poster,
          progress: p.position / p.duration,
          play: () => store.playMovie(m),
          menu: (e) => menu(e, m),
        },
      ];
    }),
  );
</script>

<PosterBrowser
  items={store.movies}
  info={(i) => i.movie}
  empty={t('No movies. M3U playlists don’t carry any; add an Xtream account.')}
  onpick={(i) => store.playMovie(i)}
  {menu}
  {status}
  {continuing}
  {details}
/>

{#snippet details(i: MovieItem)}
  {@const p = library.progress[i.id]}
  {@const started = library.startAt(i.id) > 0}
  {@const saved = library.myList.has(i.id)}
  <div class="detail">
    <div class="cover">{#if i.movie.poster}<img src={i.movie.poster} alt="" referrerpolicy="no-referrer" />{:else}<span>{i.movie.name}</span>{/if}</div>
    <h2>{i.movie.name}</h2>
    <div class="meta">
      {#if i.movie.rating && i.movie.rating > 0}<span class="mono">★ {i.movie.rating.toFixed(1)}</span>{/if}
      {#if i.movie.group}<span>{i.movie.group}</span>{/if}
      {#if p?.watched}<span>{t('Watched')}</span>{/if}
    </div>
    <div class="actions">
      <button class="btn accent" onclick={() => store.playMovie(i)}><span class="icon">&#xE768;</span>{started ? t('Resume') : t('Play')}</button>
      <button class="btn" class:on={saved} aria-pressed={saved} onclick={() => toggleMine(i)}><span class="icon">{saved ? '' : ''}</span>{t('My list')}</button>
    </div>
    {#if started && p}
      <div class="left"><span class="bar"><span style:width="{(p.position / p.duration) * 100}%"></span></span>{t('{time} left', { time: clock(p.duration - p.position) })}</div>
      <button class="link" onclick={() => store.playMovie(i, true)}>{t('Play from the start')}</button>
    {/if}
    {#if i.movie.plot}<p class="plot">{i.movie.plot}</p>{/if}
  </div>
{/snippet}

<style>
  .detail {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .cover {
    position: relative;
    aspect-ratio: 2 / 3;
    max-height: 360px;
    border-radius: 14px;
    overflow: hidden;
    background: var(--raised);
    display: grid;
    place-items: center;
  }
  .cover img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .cover span {
    padding: 18px;
    font-size: 24px;
    font-weight: 700;
    text-align: center;
  }
  h2 {
    margin: 0;
    font-size: 22px;
    font-weight: 700;
    letter-spacing: -0.01em;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 12px;
    font-size: 13px;
    color: var(--muted);
  }
  .actions {
    display: flex;
    gap: 10px;
  }
  .btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    height: 42px;
    padding: 0 16px;
    border-radius: 10px;
    background: var(--raised);
    font-size: 14px;
    font-weight: 500;
  }
  .btn:hover {
    background: var(--hover);
  }
  .btn .icon {
    font-size: 13px;
  }
  .btn.accent {
    flex: 1;
    background: var(--accent);
    color: var(--bg);
    font-weight: 600;
  }
  .btn.accent:hover {
    background: var(--accent-hover);
  }
  .btn.on .icon {
    color: var(--accent);
  }
  .left {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 13px;
    color: var(--soft);
  }
  .bar {
    flex: 1;
    height: 4px;
    border-radius: 2px;
    background: #222836;
    overflow: hidden;
  }
  .bar span {
    display: block;
    height: 100%;
    background: var(--accent);
  }
  .link {
    align-self: flex-start;
    font-size: 13px;
    color: var(--accent);
  }
  .plot {
    margin: 0;
    font-size: 13px;
    line-height: 1.55;
    color: var(--muted);
  }
</style>
