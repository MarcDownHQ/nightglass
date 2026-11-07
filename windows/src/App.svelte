<script lang="ts">
  import { t } from './lib/i18n.svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { onMount } from 'svelte';
  import { runAutomation } from './lib/automation';
  import { startGamepad } from './lib/gamepad';
  import ContextMenu from './lib/ContextMenu.svelte';
  import EpgMatchDialog from './lib/EpgMatchDialog.svelte';
  import GuideView from './lib/GuideView.svelte';
  import HelpDialog from './lib/HelpDialog.svelte';
  import HomeView from './lib/HomeView.svelte';
  import { panels } from './lib/dialogs.svelte';
  import { options } from './lib/options.svelte';
  import SettingsDialog from './lib/SettingsDialog.svelte';
  import LiveView from './lib/LiveView.svelte';
  import MoviesView from './lib/MoviesView.svelte';
  import { multiview } from './lib/tilesState.svelte';
  import MultiView from './lib/MultiView.svelte';
  import ReminderBanner from './lib/ReminderBanner.svelte';
  import { library } from './lib/library.svelte';
  import { nav, SECTIONS, typing } from './lib/nav.svelte';
  import NeonText from './lib/NeonText.svelte';
  import OrganizeDialog from './lib/OrganizeDialog.svelte';
  import PromptDialog from './lib/PromptDialog.svelte';
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
    else if (e.ctrlKey && (e.key === '+' || e.key === '=')) options.stepZoom(0.1);
    else if (e.ctrlKey && e.key === '-') options.stepZoom(-0.1);
    else if (e.ctrlKey && e.key === '0') options.set('zoom', 1);
    else if (e.key === 'F1' || (e.key === '?' && !typing(e) && !nav.searching)) panels.help = !panels.help;
    else if (e.ctrlKey || e.altKey || e.metaKey || nav.searching || typing(e) || e.defaultPrevented) return;
    else if (e.key === '+' || e.key === '=') player.setVolume(library.volume + 5);
    else if (e.key === '-') player.setVolume(library.volume - 5);
    else if (e.key.toLowerCase() === 'm') player.toggleMute();
    else return;
    e.preventDefault();
  }

  onMount(async () => {
    void updater.check();
    void options.applyZoom();
    startGamepad();
    if (options.startFullscreen) void win.setFullscreen(true);
    nav.section = options.startScreen === 'home' ? 'home' : 'live';
    await store.init();
    ready = true;
    if (options.startScreen === 'last' && library.recents[0]) nav.focusChannel = library.recents[0];
    await runAutomation();
  });
</script>

<svelte:window bind:innerWidth={W} bind:innerHeight={H} {onkeydown} />

<div
  class="backdrop"
  style:clip-path={multiview.open
    ? backdropClip(W, H, ...multiview.rects.map((r, i) => (multiview.tiles[i].item ? r : null)))
    : backdropClip(W, H, hole.rect)}
></div>

<!-- Stays mounted under the fullscreen player, so the screens keep their category and scroll. -->
<div class="app" class:hidden={player.expanded}>
  <TitleBar />
  {#if multiview.open}
    <MultiView />
  {:else if nav.section === 'sources'}
    <SourcesView />
  {:else if ready && !store.sources.length}
    <div class="empty">
      <NeonText text="No source yet" size={44} />
      <p>{t('Add an M3U playlist, an Xtream account or a Stalker portal.')}</p>
      <button class="primary" onclick={() => (nav.section = 'sources')}>{t('Add a source')}</button>
    </div>
  {:else if ready}
    {#if nav.section === 'home'}
      <HomeView />
    {:else if nav.section === 'live'}
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

<ReminderBanner />
{#if panels.settings}<SettingsDialog />{/if}
{#if panels.help}<HelpDialog />{/if}
<ContextMenu />
<OrganizeDialog />
<EpgMatchDialog />
<PromptDialog />

{#if player.osd}
  <div class="toast" role="status">{player.osd}</div>
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
  .toast {
    position: fixed;
    left: 50%;
    bottom: 40px;
    z-index: 90;
    transform: translateX(-50%);
    padding: 9px 18px;
    border-radius: 999px;
    background: rgb(5 1 15 / 0.85);
    border: 1px solid rgb(5 217 232 / 0.4);
    font-size: 15px;
    font-weight: 500;
    pointer-events: none;
  }
  .primary:hover {
    box-shadow: 0 0 24px rgb(255 42 109 / 0.85);
  }
</style>
