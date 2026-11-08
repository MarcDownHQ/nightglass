<script lang="ts">
  import { t } from './i18n.svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import Menu from './Menu.svelte';
  import { nav, SECTIONS } from './nav.svelte';
  import { player } from './player.svelte';
  import { store } from './store.svelte';
  import { updater } from './updater.svelte';
  import { prompt } from './dialogs.svelte';
  import { library } from './library.svelte';
  import { multiview } from './tilesState.svelte';

  const win = getCurrentWindow();
  const ALL = '';

  const title = $derived(multiview.open ? t('Multi-view') : t(SECTIONS.find((s) => s.id === nav.section)?.title ?? ''));
  const current = $derived(store.visibleSources.length === 1 ? store.visibleSources[0].name : t('All sources'));
  const choices = $derived([
    { id: ALL, name: t('All sources'), selected: store.visibleSources.length !== 1 },
    ...store.sources.map((s) => ({ id: s.id, name: s.name, selected: store.activeSource === s.id })),
  ]);

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

<header data-tauri-drag-region>
  <h1 data-tauri-drag-region>{title}</h1>
  {#if store.loading.size}<span class="loading">{t('Loading…')}</span>{/if}
  <div class="spacer" data-tauri-drag-region></div>
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
    <button class="pill square" class:open={library.unlocked} onclick={lock} title={library.unlocked ? t('Lock the PIN categories again') : t('Show the categories behind the PIN')}>
      <span class="icon">{library.unlocked ? '' : ''}</span>
    </button>
  {/if}
  {#if store.sources.length > 1}
    <Menu label={t('Source')} items={choices} onpick={(id) => store.setActive(id || null)} title={t('Choose the source to show')}>
      {#snippet trigger()}
        <span class="pill"><span class="dot"></span><span class="name">{current}</span><span class="icon chevron">&#xE70D;</span></span>
      {/snippet}
    </Menu>
  {/if}
  <button class="pill search" onclick={() => (nav.searching = true)}>
    <span class="icon">&#xE721;</span><span class="hint">{t('Channels, programmes, movies, series')}</span><kbd>Ctrl K</kbd>
  </button>
  <div class="controls">
    <button class="icon" aria-label={t('Minimize')} onclick={() => win.minimize()}>&#xE921;</button>
    <button class="icon" aria-label={t('Maximize')} onclick={() => win.toggleMaximize()}>&#xE922;</button>
    <button class="icon close" aria-label={t('Close')} onclick={() => win.close()}>&#xE8BB;</button>
  </div>
</header>

<style>
  header {
    flex: none;
    height: 60px;
    display: flex;
    align-items: center;
    gap: 10px;
    padding-left: 28px;
    border-bottom: 1px solid var(--border);
  }
  h1 {
    margin: 0;
    font-size: 20px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }
  .spacer {
    flex: 1;
    align-self: stretch;
  }
  .loading {
    font-size: 12px;
    color: var(--muted);
    animation: pulse 1.2s ease-in-out infinite alternate;
  }
  @keyframes pulse {
    to {
      opacity: 0.4;
    }
  }
  .pill {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 36px;
    box-sizing: border-box;
    padding: 0 12px;
    border-radius: 10px;
    background: var(--raised);
    color: var(--soft);
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
    transition: background 0.15s, color 0.15s;
  }
  .pill:hover {
    background: var(--hover);
    color: #fff;
  }
  .pill .icon {
    font-size: 14px;
  }
  .square {
    width: 36px;
    padding: 0;
    justify-content: center;
  }
  .square.open,
  .update {
    color: var(--accent);
  }
  .search {
    width: 340px;
    color: var(--muted);
  }
  .search .hint {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    text-align: left;
  }
  kbd {
    font: inherit;
    font-size: 11px;
    padding: 2px 6px;
    border-radius: 6px;
    background: var(--hover);
  }
  .name {
    max-width: 200px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #5bd69b;
  }
  .chevron {
    font-size: 10px;
    color: var(--muted);
  }
  .controls {
    display: flex;
    align-self: stretch;
    margin-left: 8px;
  }
  .controls button {
    width: 46px;
    font-size: 10px;
    color: var(--muted);
    transition: background 0.15s, color 0.15s;
  }
  .controls button:hover {
    background: rgb(255 255 255 / 0.08);
    color: var(--text);
  }
  .controls .close:hover {
    background: #c42b1c;
    color: #fff;
  }
</style>
