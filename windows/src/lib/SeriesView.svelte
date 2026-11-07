<script lang="ts">
  import { t } from './i18n.svelte';
  import { contextMenu } from './menuState.svelte';
  import { episodeKey, library } from './library.svelte';
  import type { Episode, SeriesItem } from './models';
  import { nav } from './nav.svelte';
  import PosterBrowser, { type ContinueCard } from './PosterBrowser.svelte';
  import SeriesDetail from './SeriesDetail.svelte';
  import { store } from './store.svelte';

  const byId = $derived(new Map(store.series.map((s) => [s.id, s])));

  function menu(e: MouseEvent, item: SeriesItem, episode: Episode) {
    const key = episodeKey(item.id, episode);
    contextMenu.open(e, [
      { label: t('Resume'), icon: '', action: () => void store.playEpisode(item, episode) },
      { label: t('Play from the start'), icon: '', action: () => void store.playEpisode(item, episode, undefined, true) },
      {
        label: t('Mark as watched'),
        icon: '',
        action: () => library.setWatched(key, { kind: 'episode', series: item.id, episode }, true),
      },
      { label: t('Open the series'), icon: '', action: () => (nav.openSeries = item) },
    ]);
  }

  const continuing = $derived(
    library.continuing.flatMap(([id, p]): ContinueCard[] => {
      const r = p.resume;
      const item = r.kind === 'episode' ? byId.get(r.series) : undefined;
      if (!item || r.kind !== 'episode') return [];
      return [
        {
          id,
          title: item.series.name,
          subtitle: `S${r.episode.season}E${r.episode.number} · ${r.episode.title}`,
          poster: item.series.poster,
          progress: p.position / p.duration,
          play: () => void store.playEpisode(item, r.episode),
          menu: (e) => menu(e, item, r.episode),
        },
      ];
    }),
  );
</script>

<!-- The grid stays mounted under the page, so Back returns to the same category and scroll. -->
<div class="series" class:covered={nav.openSeries}>
  <PosterBrowser
    items={store.series}
    info={(i) => i.series}
    empty={t('No series. Add an Xtream account that has some.')}
    onpick={(i) => {
      nav.openSeries = i;
    }}
    {continuing}
  />
</div>
{#if nav.openSeries}
  <SeriesDetail item={nav.openSeries} close={() => (nav.openSeries = null)} />
{/if}

<style>
  .series {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .covered {
    visibility: hidden;
  }
</style>
