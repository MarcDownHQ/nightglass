<script lang="ts" generics="T">
  import type { Snippet } from 'svelte';

  /** Renders only the rows in view: providers list tens of thousands of channels. */
  let { items, rowHeight, key, row }: { items: T[]; rowHeight: number; key: (item: T) => string; row: Snippet<[T]> } = $props();

  let viewport: HTMLElement;
  let scrollTop = $state(0);
  let height = $state(0);
  const overscan = 6;
  const start = $derived(Math.max(0, Math.floor(scrollTop / rowHeight) - overscan));
  const visible = $derived(items.slice(start, Math.ceil((scrollTop + height) / rowHeight) + overscan));

  /** Scrolls just enough to show the row. */
  export function reveal(index: number) {
    const top = index * rowHeight;
    if (top < viewport.scrollTop) viewport.scrollTop = top;
    else if (top + rowHeight > viewport.scrollTop + height) viewport.scrollTop = top + rowHeight - height;
  }
</script>

<div class="viewport" bind:this={viewport} bind:clientHeight={height} onscroll={() => (scrollTop = viewport.scrollTop)}>
  <div class="content" style:height="{items.length * rowHeight}px">
    {#each visible as item, i (key(item))}
      <div class="row" style:top="{(start + i) * rowHeight}px" style:height="{rowHeight}px">{@render row(item)}</div>
    {/each}
  </div>
</div>

<style>
  .viewport {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    scrollbar-width: thin;
    scrollbar-color: rgb(243 238 255 / 0.15) transparent;
  }
  .content {
    position: relative;
  }
  .row {
    position: absolute;
    left: 0;
    right: 0;
  }
</style>
