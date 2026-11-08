<script lang="ts">
  import { tick } from 'svelte';
  import { t } from './i18n.svelte';
  import { fold } from './lineup';

  /** Every tab of the Live strip in one list you can scroll and filter: the way through hundreds of categories with a mouse. */
  let {
    tabs,
    selected,
    count,
    onpick,
  }: {
    tabs: { id: string; label: string }[];
    selected: string;
    count: (id: string) => number;
    onpick: (id: string) => void;
  } = $props();

  let open = $state(false);
  let query = $state('');
  let index = $state(0);
  let root: HTMLElement;
  let input: HTMLInputElement | undefined = $state();
  let listEl: HTMLElement | undefined = $state();

  const hits = $derived.by(() => {
    const q = fold(query.trim());
    return q ? tabs.filter((tb) => fold(tb.label).includes(q)) : tabs;
  });

  $effect(() => {
    void query;
    index = 0;
  });

  $effect(() => {
    listEl?.children[index]?.scrollIntoView({ block: 'nearest' });
  });

  async function toggle() {
    open = !open;
    if (!open) return;
    query = '';
    await tick();
    index = Math.max(hits.findIndex((tb) => tb.id === selected), 0);
    input?.focus();
  }

  function pick(id: string) {
    open = false;
    onpick(id);
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') open = false;
    else if (e.key === 'ArrowDown') index = Math.min(index + 1, Math.max(hits.length - 1, 0));
    else if (e.key === 'ArrowUp') index = Math.max(index - 1, 0);
    else if (e.key === 'Enter' && hits[index]) pick(hits[index].id);
    else return;
    e.preventDefault();
    e.stopPropagation();
  }
</script>

<svelte:window onmousedown={(e) => open && !root.contains(e.target as Node) && (open = false)} />

<div class="picker" bind:this={root}>
  <button class="icon trigger" class:on={open} title={t('All categories')} aria-expanded={open} onclick={toggle}>&#xE8FD;</button>
  {#if open}
    <div class="panel" role="dialog" aria-label={t('All categories')}>
      <label class="field">
        <span class="icon">&#xE721;</span>
        <input bind:this={input} bind:value={query} {onkeydown} placeholder={t('Find a category')} spellcheck="false" />
      </label>
      {#if hits.length}
        <ul bind:this={listEl} role="listbox">
          {#each hits as tb, i (tb.id)}
            <li role="option" aria-selected={tb.id === selected}>
              <button class:on={i === index} class:current={tb.id === selected} onclick={() => pick(tb.id)} onmousemove={() => (index = i)}>
                <span class="label">{tb.label}</span>
                <span class="count mono">{count(tb.id)}</span>
              </button>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="none">{t('Nothing matches.')}</p>
      {/if}
    </div>
  {/if}
</div>

<style>
  .trigger {
    flex: none;
    padding: 8px;
    font-size: 15px;
    color: var(--dim);
  }
  .trigger:hover,
  .trigger.on {
    color: var(--cyan);
  }
  .panel {
    position: absolute;
    top: 44px;
    left: 0;
    z-index: 10;
    width: min(100%, 440px);
    display: flex;
    flex-direction: column;
    max-height: min(70vh, 640px);
    background: var(--ink);
    border: 1px solid rgb(233 236 242 / 0.1);
    border-radius: 12px;
    overflow: hidden;
  }
  .field {
    flex: none;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 14px;
    border-bottom: 1px solid rgb(233 236 242 / 0.06);
  }
  .field .icon {
    color: var(--cyan);
    font-size: 15px;
  }
  input {
    flex: 1;
    min-width: 0;
    font: inherit;
    font-size: 16px;
    font-weight: 500;
    line-height: 19px;
    color: var(--text);
    background: none;
    border: 0;
    outline: none;
  }
  input::placeholder {
    color: var(--dim);
  }
  ul {
    margin: 0;
    padding: 6px;
    list-style: none;
    overflow-y: auto;
  }
  li button {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 7px 10px;
    border-radius: 6px;
    text-align: left;
    font-size: 15px;
    /* 33 px rows: an odd height puts the capitals exactly in the middle. */
    line-height: 19px;
    color: var(--dim);
  }
  li button.on {
    background: rgb(233 236 242 / 0.07);
    color: var(--text);
  }
  li button.current {
    color: var(--cyan);
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
    color: var(--dim);
  }
  .none {
    margin: 0;
    padding: 12px 16px;
    font-size: 15px;
    color: var(--dim);
  }
</style>
