<script lang="ts">
  import { t } from './i18n.svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import Menu from './Menu.svelte';
  import { nav, SECTIONS, type Section } from './nav.svelte';
  import NeonText from './NeonText.svelte';
  import { player } from './player.svelte';
  import { store } from './store.svelte';
  import { updater } from './updater.svelte';
  import { panels, prompt } from './dialogs.svelte';
  import { library } from './library.svelte';
  import { multiview } from './tilesState.svelte';

  const win = getCurrentWindow();
  const ALL = '';

  const current = $derived(store.visibleSources.length === 1 ? store.visibleSources[0].name : t('All sources'));
  const choices = $derived([
    { id: ALL, name: t('All sources'), selected: store.visibleSources.length !== 1 },
    ...store.sources.map((s) => ({ id: s.id, name: s.name, selected: store.activeSource === s.id })),
  ]);

  function show(section: Section) {
    if (multiview.open) void multiview.close();
    nav.section = section;
    void player.collapse();
  }

  async function lock() {
    if (library.unlocked) {
      library.unlocked = false;
      return;
    }
    const pin = await prompt.ask(t('Parental PIN'), '', { secret: true });
    if (pin === null) return;
    if (await library.checkPin(pin)) library.unlocked = true;
    else player.notify(t('Wrong PIN'));
  }
</script>

{#snippet tab(section: Section, title: string)}
  <button class="nav" class:on={nav.section === section} onclick={() => show(section)}>{title}</button>
{/snippet}

<header data-tauri-drag-region>
  <div class="brand"><NeonText text="NIGHTGLASS" size={26} /></div>
  {#each SECTIONS.slice(0, 5) as s (s.id)}{@render tab(s.id, t(s.title))}{/each}
  <div class="spacer" data-tauri-drag-region></div>
  {#if store.loading.size}<span class="loading mono">{t('Loading…')}</span>{/if}
  {#if updater.available}
    <button
      class="pill update"
      title={updater.error ?? t('Install version {version}; Nightglass restarts', { version: updater.available.version })}
      onclick={() => updater.install()}
    >
      <span class="icon">&#xE896;</span>
      {updater.progress === null ? t('Update {version}', { version: updater.available.version }) : t('Updating {percent}%', { percent: Math.round(updater.progress * 100) })}
    </button>
  {/if}
  {#if library.pinHash && library.lockedGroups.size}
    <button class="pill lock" class:open={library.unlocked} onclick={lock} title={library.unlocked ? t('Lock the PIN categories again') : t('Show the categories behind the PIN')}>
      <span class="icon">{library.unlocked ? '\uE785' : '\uE72E'}</span>
    </button>
  {/if}
  {#if store.sources.length > 1}
    <div class="picker-menu">
    <Menu label={t('Source')} items={choices} onpick={(id) => store.setActive(id || null)} title={t('Choose the source to show')}>
      {#snippet trigger()}
        <span class="pill picker"><span class="dot"></span><span class="name">{current}</span><span class="icon chevron">&#xE70D;</span></span>
      {/snippet}
    </Menu>
    </div>
  {/if}
  <button class="pill" onclick={() => (nav.searching = true)}>
    <span class="icon">&#xE721;</span> {t('Search')} <span class="dim mono">Ctrl K</span>
  </button>
  {@render tab('sources', t('Sources'))}
  <button class="icon gear" title={t('Settings')} onclick={() => (panels.settings = true)}>&#xE713;</button>
  <div class="controls">
    <button class="icon" aria-label={t('Minimize')} onclick={() => win.minimize()}>&#xE921;</button>
    <button class="icon" aria-label={t('Maximize')} onclick={() => win.toggleMaximize()}>&#xE922;</button>
    <button class="icon close" aria-label={t('Close')} onclick={() => win.close()}>&#xE8BB;</button>
  </div>
</header>

<style>
  header {
    /* Odd heights (65 here, 35 for the controls): the capitals, an odd number of pixels tall, then sit
       exactly in the middle instead of half a pixel off. */
    flex: none;
    height: 65px;
    display: flex;
    align-items: center;
    gap: 4px;
    padding-left: 28px;
  }
  /* Clicks fall through to the drag region. */
  .brand {
    pointer-events: none;
    margin-right: 24px;
  }
  .spacer {
    flex: 1;
    align-self: stretch;
  }
  .nav {
    height: 35px;
    box-sizing: border-box;
    padding: 0 12px;
    font-size: 18px;
    font-weight: 600;
    color: var(--dim);
    white-space: nowrap;
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
    margin-right: 12px;
    font-size: 11px;
    color: var(--acid);
    animation: pulse 1.2s ease-in-out infinite alternate;
  }
  @keyframes pulse {
    to {
      opacity: 0.35;
    }
  }
  .pill {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-right: 12px;
    height: 35px;
    box-sizing: border-box;
    padding: 0 14px;
    border-radius: 999px;
    border: 1px solid rgb(243 238 255 / 0.15);
    font-size: 15px;
    font-weight: 500;
    white-space: nowrap;
    transition: border-color 0.15s;
  }
  .pill:hover {
    border-color: rgb(243 238 255 / 0.35);
  }
  .pill .icon {
    font-size: 13px;
  }
  .pill .dim {
    font-size: 11px;
    color: var(--dim);
  }
  .lock {
    width: 35px;
    padding: 0;
    justify-content: center;
    color: var(--dim);
  }
  .lock.open {
    color: var(--acid);
    border-color: rgb(249 240 2 / 0.5);
  }
  .update {
    color: var(--acid);
    border-color: rgb(249 240 2 / 0.5);
  }
  .update:hover {
    border-color: var(--acid);
  }
  /* The pill's margin sits outside the menu, so the list lines up with the pill's right edge. */
  .picker-menu {
    margin-right: 12px;
  }
  .picker {
    margin-right: 0;
    gap: 6px;
    border-color: rgb(255 42 109 / 0.5);
  }
  .picker:hover {
    border-color: var(--magenta);
  }
  .picker .name {
    max-width: 220px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--magenta);
    box-shadow: 0 0 4px var(--magenta);
  }
  .chevron {
    font-size: 9px;
    color: var(--dim);
  }
  .gear {
    margin-left: 6px;
    width: 35px;
    height: 35px;
    font-size: 16px;
    color: var(--dim);
  }
  .gear:hover {
    color: var(--text);
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
