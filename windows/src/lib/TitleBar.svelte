<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { nav, type Section } from './nav.svelte';
  import NeonText from './NeonText.svelte';
  import { store } from './store.svelte';

  const win = getCurrentWindow();
</script>

{#snippet tab(section: Section, title: string)}
  <button class="nav" class:on={nav.section === section} onclick={() => (nav.section = section)}>{title}</button>
{/snippet}

<header data-tauri-drag-region>
  <div class="brand"><NeonText text="NIGHTGLASS" size={26} /></div>
  {@render tab('live', 'Live')}
  <div class="spacer" data-tauri-drag-region></div>
  {#if store.loading.size}<span class="loading mono">Loading…</span>{/if}
  {@render tab('sources', 'Sources')}
  <div class="controls">
    <button class="icon" aria-label="Minimize" onclick={() => win.minimize()}>&#xE921;</button>
    <button class="icon" aria-label="Maximize" onclick={() => win.toggleMaximize()}>&#xE922;</button>
    <button class="icon close" aria-label="Close" onclick={() => win.close()}>&#xE8BB;</button>
  </div>
</header>

<style>
  header {
    flex: none;
    height: 64px;
    display: flex;
    align-items: center;
    gap: 16px;
    padding-left: 28px;
  }
  /* Clicks fall through to the drag region. */
  .brand {
    pointer-events: none;
    margin-right: 12px;
  }
  .spacer {
    flex: 1;
    align-self: stretch;
  }
  .nav {
    padding: 6px 12px;
    font-size: 18px;
    font-weight: 600;
    color: var(--dim);
    transition: color 0.15s, text-shadow 0.3s;
  }
  .nav:hover {
    color: var(--text);
  }
  .nav.on {
    color: #fff;
    text-shadow:
      0 0 2px var(--magenta),
      0 0 10px var(--magenta);
  }
  .loading {
    font-size: 11px;
    color: var(--acid);
    animation: pulse 1.2s ease-in-out infinite alternate;
  }
  @keyframes pulse {
    to {
      opacity: 0.35;
    }
  }
  .controls {
    display: flex;
    align-self: stretch;
    margin-left: 8px;
  }
  .controls button {
    width: 46px;
    font-size: 10px;
    color: var(--dim);
    transition: background 0.15s, color 0.15s;
  }
  .controls button:hover {
    background: rgb(243 238 255 / 0.08);
    color: var(--text);
  }
  .controls .close:hover {
    background: var(--magenta);
    color: #fff;
  }
</style>
