<script lang="ts">
  import { MAC, t } from './i18n.svelte';
  import { nav, type Section } from './nav.svelte';
  import { panels } from './dialogs.svelte';
  import { player } from './player.svelte';
  import { multiview } from './tilesState.svelte';

  /** Segoe Fluent Icons glyph per screen. */
  const MAIN: { id: Section; title: string; glyph: string }[] = [
    { id: 'home', title: 'Home', glyph: '' },
    { id: 'live', title: 'Live', glyph: '' },
    { id: 'guide', title: 'Guide', glyph: '' },
    { id: 'movies', title: 'Movies', glyph: '' },
    { id: 'series', title: 'Series', glyph: '' },
  ];

  function show(section: Section) {
    if (multiview.open) void multiview.close();
    nav.section = section;
    void player.collapse();
  }
</script>

{#snippet item(id: Section, title: string, glyph: string)}
  <button class="item" class:on={nav.section === id && !multiview.open} aria-current={nav.section === id ? 'page' : undefined} onclick={() => show(id)}>
    <span class="icon">{glyph}</span>{t(title)}
  </button>
{/snippet}

<nav aria-label={t('Main navigation')} class:mac={MAC} data-tauri-drag-region>
  <div class="brand" aria-hidden="true" data-tauri-drag-region>
    <svg width="22" height="22" viewBox="0 0 24 24"><path d="M20 14.5A8.5 8.5 0 1 1 9.5 4a7 7 0 0 0 10.5 10.5z" fill="var(--accent)" /></svg>
  </div>
  {#each MAIN as s (s.id)}{@render item(s.id, s.title, s.glyph)}{/each}
  <div class="spacer" data-tauri-drag-region></div>
  {@render item('sources', 'Sources', '')}
  <button class="item" onclick={() => (panels.settings = true)}><span class="icon">&#xE713;</span>{t('Settings')}</button>
</nav>

<style>
  nav {
    flex: none;
    width: 84px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 14px 0 16px;
    border-right: 1px solid var(--border);
  }
  /* Below the traffic lights. */
  nav.mac {
    padding-top: 40px;
  }
  .brand {
    width: 40px;
    height: 40px;
    margin-bottom: 14px;
    border-radius: 12px;
    background: var(--raised);
    display: grid;
    place-items: center;
  }
  .spacer {
    flex: 1;
    align-self: stretch;
  }
  .item {
    width: 68px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 5px;
    padding: 10px 0 9px;
    border-radius: 12px;
    color: var(--muted);
    font-size: 11px;
    font-weight: 600;
    transition: background 0.15s, color 0.15s;
  }
  .item .icon {
    font-size: 20px;
  }
  .item:hover {
    background: var(--raised);
    color: var(--text);
  }
  .item.on {
    background: var(--raised);
    color: var(--text);
  }
  .item.on .icon {
    color: var(--accent);
  }
</style>
