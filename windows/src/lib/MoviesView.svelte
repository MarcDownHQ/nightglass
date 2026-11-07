<script lang="ts">
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

  function menu(e: MouseEvent, i: MovieItem) {
    const resume = { kind: 'movie' as const, id: i.id };
    const started = library.startAt(i.id) > 0;
    contextMenu.open(e, [
      { label: started ? 'Resume' : 'Play', icon: '', action: () => store.playMovie(i) },
      ...(started ? [{ label: 'Play from the start', icon: '', action: () => store.playMovie(i, true) }] : []),
      library.progress[i.id]?.watched
        ? { label: 'Mark as unwatched', icon: '', action: () => library.setWatched(i.id, resume, false) }
        : { label: 'Mark as watched', icon: '', action: () => library.setWatched(i.id, resume, true) },
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
          subtitle: `${clock(p.duration - p.position)} left`,
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
  empty="No movies. M3U playlists don't carry any; add an Xtream account."
  onpick={(i) => store.playMovie(i)}
  {menu}
  {status}
  {continuing}
/>
