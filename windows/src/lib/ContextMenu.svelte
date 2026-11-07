<script lang="ts">
  import { contextMenu } from './menuState.svelte';

  let root: HTMLElement | undefined = $state();
  let left = $state(0);
  let top = $state(0);

  // Kept inside the window: flipped left or up when it would overflow.
  $effect(() => {
    if (!root) return;
    const { width, height } = root.getBoundingClientRect();
    left = Math.min(contextMenu.x, innerWidth - width - 8);
    top = contextMenu.y + height > innerHeight - 8 ? Math.max(8, contextMenu.y - height) : contextMenu.y;
    root.querySelector('button')?.focus();
  });

  function onkeydown(e: KeyboardEvent) {
    if (!contextMenu.entries.length) return;
    const buttons = [...(root?.querySelectorAll('button') ?? [])];
    const i = buttons.indexOf(document.activeElement as HTMLButtonElement);
    if (e.key === 'Escape') contextMenu.close();
    else if (e.key === 'ArrowDown') buttons[(i + 1) % buttons.length]?.focus();
    else if (e.key === 'ArrowUp') buttons[(i - 1 + buttons.length) % buttons.length]?.focus();
    else return;
    e.preventDefault();
    e.stopImmediatePropagation();
  }
</script>

<svelte:window
  onkeydowncapture={onkeydown}
  onmousedown={(e) => contextMenu.entries.length && !root?.contains(e.target as Node) && contextMenu.close()}
  onblur={() => contextMenu.close()}
/>

{#if contextMenu.entries.length}
  <ul class="menu" role="menu" bind:this={root} style:left="{left}px" style:top="{top}px">
    {#each contextMenu.entries as entry, i (i)}
      <li>
        {#if 'heading' in entry}
          <span class="heading mono">{entry.heading}</span>
        {:else}
          <button
            role={entry.checked === undefined ? 'menuitem' : 'menuitemradio'}
            aria-checked={entry.checked}
            onclick={() => {
              contextMenu.close();
              entry.action();
            }}
          >
            <span class="icon">{entry.checked ? '' : (entry.icon ?? '')}</span>{entry.label}
          </button>
        {/if}
      </li>
    {/each}
  </ul>
{/if}

<style>
  .menu {
    max-height: calc(100vh - 16px);
    overflow-y: auto;
    position: fixed;
    z-index: 100;
    min-width: 220px;
    margin: 0;
    padding: 6px;
    list-style: none;
    background: var(--ink);
    border: 1px solid rgb(5 217 232 / 0.35);
    border-radius: 10px;
    box-shadow: 0 0 18px rgb(5 217 232 / 0.18);
  }
  button {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    text-align: left;
    padding: 7px 10px;
    border-radius: 6px;
    font-size: 15px;
    color: var(--dim);
    white-space: nowrap;
  }
  button:hover,
  button:focus-visible {
    outline: none;
    background: rgb(243 238 255 / 0.07);
    color: var(--text);
  }
  .heading {
    display: block;
    padding: 8px 10px 4px;
    font-size: 11px;
    color: var(--acid);
  }
  li:not(:first-child) > .heading {
    margin-top: 4px;
    border-top: 1px solid rgb(243 238 255 / 0.08);
  }
  .icon {
    width: 16px;
    font-size: 13px;
    color: var(--cyan);
  }
</style>
