<script lang="ts">
  import { nav } from './nav.svelte';
  import PosterBrowser from './PosterBrowser.svelte';
  import SeriesDetail from './SeriesDetail.svelte';
  import { store } from './store.svelte';
</script>

<!-- The grid stays mounted under the page, so Back returns to the same category and scroll. -->
<div class="series" class:covered={nav.openSeries}>
  <PosterBrowser
    items={store.series}
    info={(i) => i.series}
    empty="No series. Add an Xtream account that has some."
    onpick={(i) => (nav.openSeries = i)}
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
