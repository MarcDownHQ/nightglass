<script lang="ts">
  import PosterBrowser from './PosterBrowser.svelte';
  import { store } from './store.svelte';
</script>

<PosterBrowser
  items={store.movies}
  info={(i) => i.movie}
  empty="No movies. M3U playlists don't carry any; add an Xtream account."
  onpick={(i) => store.playMovie(i)}
/>
