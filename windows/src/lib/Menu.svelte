<script lang="ts" generics="T">
  import type { Snippet } from 'svelte';

  /** A label that opens a short list: audio and subtitle tracks, the source picker. */
  let {
    label,
    items,
    onpick,
    trigger,
    title,
  }: {
    label: string;
    items: { id: T; name: string; selected: boolean }[];
    onpick: (id: T) => void;
    /** Replaces the plain label. */
    trigger?: Snippet;
    title?: string;
  } = $props();
  let open = $state(false);
  let root: HTMLElement;
</script>

<svelte:window onmousedown={(e) => open && !root.contains(e.target as Node) && (open = false)} />

<div class="menu" bind:this={root}>
  <button class="trigger" class:custom={trigger} aria-expanded={open} {title} onclick={() => (open = !open)}>
    {#if trigger}{@render trigger()}{:else}{label}{/if}
  </button>
  {#if open}
    <ul role="menu" aria-label={label}>
      {#each items as item (item.id)}
        <li>
          <button role="menuitemradio" aria-checked={item.selected} class:on={item.selected} onclick={() => ((open = false), onpick(item.id))}>
            {item.name}
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .menu {
    position: relative;
  }
  .trigger {
    font-size: 15px;
    font-weight: 500;
    padding: 6px 4px;
  }
  .trigger.custom {
    padding: 0;
  }
  .trigger:not(.custom):hover,
  .trigger:not(.custom)[aria-expanded='true'] {
    color: var(--cyan);
  }
  ul {
    position: absolute;
    top: calc(100% + 6px);
    right: 0;
    z-index: 10;
    min-width: 200px;
    max-height: 60vh;
    overflow-y: auto;
    margin: 0;
    padding: 6px;
    list-style: none;
    background: var(--ink);
    border: 1px solid rgb(5 217 232 / 0.35);
    border-radius: 10px;
    box-shadow: 0 0 18px rgb(5 217 232 / 0.18);
  }
  li button {
    width: 100%;
    text-align: left;
    padding: 7px 10px;
    border-radius: 6px;
    font-size: 15px;
    color: var(--dim);
    white-space: nowrap;
  }
  li button:hover {
    background: rgb(243 238 255 / 0.07);
    color: var(--text);
  }
  li button.on {
    color: var(--cyan);
  }
</style>
