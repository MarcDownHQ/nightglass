<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { onMount } from 'svelte';
  import { runAutomation } from './lib/automation';
  import GuideView from './lib/GuideView.svelte';
  import LiveView from './lib/LiveView.svelte';
  import MoviesView from './lib/MoviesView.svelte';
  import { nav, SECTIONS } from './lib/nav.svelte';
  import NeonText from './lib/NeonText.svelte';
  import { player } from './lib/player.svelte';
  import PlayerScreen from './lib/PlayerScreen.svelte';
  import SearchPalette from './lib/SearchPalette.svelte';
  import SeriesView from './lib/SeriesView.svelte';
  import SourcesView from './lib/SourcesView.svelte';
  import { store } from './lib/store.svelte';
  import TitleBar from './lib/TitleBar.svelte';
  import { updater } from './lib/updater.svelte';
  import { backdropClip, hole } from './lib/video.svelte';

  const win = getCurrentWindow();
  let W = $state(innerWidth);
  let H = $state(innerHeight);
  let ready = $state(false);

  async function onkeydown(e: KeyboardEvent) {
    const n = Number(e.key);
    if (e.ctrlKey && e.key === '.') player.stop();
    else if (e.ctrlKey && n >= 1 && n <= SECTIONS.length) {
      nav.searching = false;
      nav.section = SECTIONS[n - 1].id;
      void player.collapse();
    } else if (e.ctrlKey && e.key.toLowerCase() === 'k') nav.searching = !nav.searching;
    else if (e.key === 'F11') await win.setFullscreen(!(await win.isFullscreen()));
    else return;
    e.preventDefault();
  }

  onMount(async () => {
    void updater.check();
    await store.init();
    ready = true;
    await runAutomation();
  });
</script>

<svelte:window bind:innerWidth={W} bind:innerHeight={H} {onkeydown} />

<div class="backdrop" style:clip-path={backdropClip(W, H, hole.rect)}></div>

<!-- Stays mounted under the fullscreen player, so the screens keep their category and scroll. -->
<div class="app" class:hidden={player.expanded}>
  <TitleBar />
  {#if nav.section === 'sources'}
    <SourcesView />
  {:else if ready && !store.sources.length}
    <div class="empty">
      <NeonText text="No source yet" size={44} />
      <p>Add an M3U playlist, an Xtream account or a Stalker portal.</p>
      <button class="primary" onclick={() => (nav.section = 'sources')}>Add a source</button>
    </div>
  {:else if ready}
    {#if nav.section === 'live'}
      <LiveView />
    {:else if nav.section === 'guide'}
      <GuideView />
    {:else if nav.section === 'movies'}
      <MoviesView />
    {:else}
      <SeriesView />
    {/if}
  {/if}
</div>

{#if player.expanded}
  <PlayerScreen />
{/if}

{#if nav.searching}
  <SearchPalette />
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: linear-gradient(to bottom, var(--ink), var(--night));
  }
  .app {
    position: relative;
    height: 100vh;
    display: flex;
    flex-direction: column;
  }
  .app.hidden {
    visibility: hidden;
  }
  .empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 18px;
    padding-bottom: 64px;
  }
  .empty p {
    margin: 0;
    font-size: 16px;
    color: var(--dim);
  }
  .primary {
    margin-top: 8px;
    padding: 10px 22px;
    border-radius: 999px;
    background: var(--magenta);
    color: var(--ink);
    font-size: 18px;
    font-weight: 700;
    box-shadow: 0 0 14px rgb(255 42 109 / 0.6);
  }
  .primary:hover {
    box-shadow: 0 0 24px rgb(255 42 109 / 0.85);
  }
</style>
