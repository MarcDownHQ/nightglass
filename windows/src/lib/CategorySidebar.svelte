<script lang="ts">
  import { t } from './i18n.svelte';
  import { fold } from './lineup';

  export interface SideEntry {
    id: string;
    label: string;
    count: number;
    /** Segoe Fluent glyph shown before the label. */
    glyph?: string;
    tint?: string;
  }

  /** The Live screen's left column: lists, pinned categories, then every category, filterable. */
  let {
    lists,
    pinned,
    categories,
    hidden,
    selected,
    onpick,
    onmenu,
    onnewlist,
  }: {
    lists: SideEntry[];
    pinned: SideEntry[];
    categories: SideEntry[];
    hidden?: SideEntry;
    selected: string;
    onpick: (id: string) => void;
    onmenu: (e: MouseEvent, entry: SideEntry) => void;
    onnewlist: () => void;
  } = $props();

  let query = $state('');
  const q = $derived(fold(query.trim()));
  const shown = $derived(q ? categories.filter((c) => fold(c.label).includes(q)) : categories);
  let listEl: HTMLElement;

  // The category picked from the keyboard (← →) or elsewhere scrolls into view.
  $effect(() => {
    void selected;
    listEl?.querySelector('[aria-current="true"]')?.scrollIntoView({ block: 'nearest' });
  });
</script>

{#snippet entry(e: SideEntry)}
  <button class="entry" class:on={e.id === selected} aria-current={e.id === selected} onclick={() => onpick(e.id)} oncontextmenu={(ev) => onmenu(ev, e)}>
    {#if e.glyph}<span class="icon" style:color={e.tint}>{e.glyph}</span>{/if}
    <span class="label">{e.label}</span>
    <span class="count">{e.count}</span>
  </button>
{/snippet}

<section class="side" aria-label={t('Categories')}>
  <label class="filter">
    <span class="icon">&#xE721;</span>
    <input bind:value={query} placeholder={t('Find a category')} spellcheck="false" aria-label={t('Find a category')} onkeydown={(e) => e.key === 'Escape' && (query = '')} />
  </label>
  <div class="scroll" bind:this={listEl}>
    {#if !q}
      <div class="heading">
        <span>{t('My lists')}</span>
        <button class="icon add" title={t('New favorites list…')} aria-label={t('New favorites list…')} onclick={onnewlist}>&#xE710;</button>
      </div>
      {#each lists as e (e.id)}{@render entry(e)}{/each}
      {#if pinned.length}
        <div class="heading"><span>{t('Pinned')}</span></div>
        {#each pinned as e (e.id)}{@render entry(e)}{/each}
      {/if}
    {/if}
    <div class="heading"><span>{q ? t('Results') : t('Categories')}</span><span class="total">{shown.length}</span></div>
    {#each shown as e (e.id)}{@render entry(e)}{/each}
    {#if q && !shown.length}<p class="none">{t('Nothing matches.')}</p>{/if}
    {#if hidden && !q}
      <div class="heading"></div>
      {@render entry(hidden)}
    {/if}
  </div>
</section>

<style>
  .side {
    width: 272px;
    flex: none;
    display: flex;
    flex-direction: column;
    min-height: 0;
    border-right: 1px solid var(--border);
  }
  .filter {
    flex: none;
    display: flex;
    align-items: center;
    gap: 8px;
    height: 38px;
    margin: 16px 16px 8px;
    padding: 0 12px;
    border-radius: 10px;
    background: var(--surface);
    border: 1px solid var(--border);
  }
  .filter .icon {
    font-size: 13px;
    color: var(--muted);
  }
  input {
    flex: 1;
    min-width: 0;
    height: 100%;
    font: inherit;
    font-size: 13px;
    color: var(--text);
    background: none;
    border: 0;
    outline: none;
  }
  input::placeholder {
    color: var(--muted);
  }
  .scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 0 10px 16px;
  }
  .heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    min-height: 12px;
    padding: 14px 12px 6px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--muted);
  }
  .total {
    font-variant-numeric: tabular-nums;
    letter-spacing: 0;
  }
  .add {
    width: 22px;
    height: 22px;
    margin: -4px -4px -4px 0;
    border-radius: 6px;
    font-size: 11px;
    color: var(--muted);
  }
  .add:hover {
    background: var(--raised);
    color: var(--text);
  }
  .entry {
    width: 100%;
    height: 36px;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 12px;
    border-radius: 9px;
    color: var(--soft);
    font-size: 14px;
    text-align: left;
  }
  .entry:hover {
    background: var(--raised);
    color: var(--text);
  }
  .entry.on {
    background: #1b2130;
    color: #fff;
  }
  .entry .icon {
    flex: none;
    width: 16px;
    font-size: 14px;
    color: var(--muted);
  }
  .label {
    flex: 1;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .count {
    flex: none;
    font-size: 12px;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  .entry.on .count {
    color: var(--accent);
  }
  .none {
    margin: 6px 12px;
    font-size: 13px;
    color: var(--muted);
  }
</style>
