<script lang="ts">
  import { t } from './i18n.svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { epgMatch } from './dialogs.svelte';
  import { library } from './library.svelte';
  import { fold } from './lineup';
  import { player } from './player.svelte';
  import { store } from './store.svelte';
  import { displayName } from './theme';
  import VirtualList from './VirtualList.svelte';

  /** A channel the guide doesn't match: pick its guide channel among the ones the source's XMLTV describes. */
  let channels = $state.raw<[string, string][]>([]);
  let loading = $state(false);
  let query = $state('');
  let input: HTMLInputElement | undefined = $state();

  const item = $derived(epgMatch.item);
  const current = $derived(item ? (library.epgOverrides[item.id] ?? null) : null);

  $effect(() => {
    const it = item;
    if (!it) return;
    query = displayName(it.channel.name);
    loading = true;
    invoke<[string, string][]>('guide_channels', { id: it.source.id })
      .then((list) => epgMatch.item === it && (channels = list))
      .catch(() => (channels = []))
      .finally(() => (loading = false));
    queueMicrotask(() => input?.select());
  });

  const hits = $derived.by(() => {
    const words = fold(query).split(/\s+/).filter(Boolean);
    if (!words.length) return channels;
    return channels.filter(([id, name]) => {
      const text = fold(`${name} ${id}`);
      return words.every((w) => text.includes(w));
    });
  });

  function choose(id: string | null) {
    if (!item) return;
    library.setEpgOverride(item.id, id);
    store.reloadGuide(item.source);
    player.notify(id ? t('Guide matched: {id}', { id }) : t('Back to the playlist’s guide id'));
    epgMatch.item = null;
  }

  function onkeydown(e: KeyboardEvent) {
    if (item && e.key === 'Escape') {
      epgMatch.item = null;
      e.preventDefault();
      e.stopPropagation();
    }
  }
</script>

<svelte:window onkeydowncapture={onkeydown} />

{#if item}
  <div class="scrim" role="presentation" onmousedown={(e) => e.target === e.currentTarget && (epgMatch.item = null)}>
    <div class="dialog" role="dialog" aria-label={t('Match the guide…')}>
      <header>
        <h2>{t('Guide for {name}', { name: displayName(item.channel.name) })}</h2>
        <button class="icon close" title={t('Close (Esc)')} onclick={() => (epgMatch.item = null)}>&#xE8BB;</button>
      </header>
      <p class="hint">
        {t('Playlist id:')} <span class="mono">{item.channel.epgId ?? t('none')}</span>
        {#if current}{t('· matched by hand:')} <span class="mono">{current}</span>
          <button class="link" onclick={() => choose(null)}>{t('Undo')}</button>{/if}
      </p>
      <input bind:this={input} bind:value={query} placeholder={t('Search the guide’s channels')} spellcheck="false" />
      <div class="list">
        {#if loading}
          <p class="note">{t('Loading…')}</p>
        {:else if !channels.length}
          <p class="note">{t('This source has no guide yet, or it has not loaded. Add an XMLTV URL to the source, or wait for it to load.')}</p>
        {:else if !hits.length}
          <p class="note">{t('No guide channel matches.')}</p>
        {:else}
          <VirtualList items={hits} rowHeight={44} key={([id]) => id} row={hit} />
        {/if}
      </div>
    </div>
  </div>
{/if}

{#snippet hit([id, name]: [string, string])}
  <button class="hit" class:on={id === current} onclick={() => choose(id)}>
    <b>{name}</b>
    {#if id !== name}<span class="mono">{id}</span>{/if}
  </button>
{/snippet}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 105;
    display: grid;
    place-items: center;
    background: rgb(5 1 15 / 0.6);
  }
  .dialog {
    width: min(640px, calc(100vw - 80px));
    height: min(620px, calc(100vh - 120px));
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 22px 24px;
    border-radius: 14px;
    background: var(--ink);
    border: 1px solid rgb(5 217 232 / 0.4);
    box-shadow: 0 0 24px rgb(5 217 232 / 0.2);
  }
  header {
    display: flex;
    align-items: center;
  }
  h2 {
    flex: 1;
    margin: 0;
    font-size: 22px;
  }
  .close {
    padding: 8px;
    font-size: 12px;
    color: var(--dim);
  }
  .hint {
    margin: 0;
    font-size: 14px;
    color: var(--dim);
  }
  .link {
    color: var(--cyan);
  }
  input {
    padding: 10px 12px;
    border-radius: 8px;
    border: 1px solid rgb(243 238 255 / 0.2);
    background: var(--night);
    color: var(--text);
    font: inherit;
    font-size: 16px;
  }
  input:focus {
    outline: none;
    border-color: var(--cyan);
  }
  .list {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .note {
    margin: 0;
    padding: 12px 4px;
    color: var(--dim);
  }
  /* Name and id on one baseline, the pair centred in the row; the text starts where the field's does. */
  .hit {
    width: 100%;
    height: 100%;
    display: grid;
    grid-auto-flow: column;
    justify-content: start;
    align-content: center;
    align-items: baseline;
    gap: 12px;
    padding: 0 13px;
    border-radius: 6px;
    text-align: left;
  }
  .hit:hover {
    background: rgb(243 238 255 / 0.06);
  }
  .hit.on b {
    color: var(--cyan);
  }
  .hit b {
    font-size: 16px;
    font-weight: 600;
  }
  .hit span {
    font-size: 12px;
    color: var(--dim);
  }
</style>
