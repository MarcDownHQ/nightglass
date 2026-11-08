<script lang="ts" generics="T">
  import type { Snippet } from 'svelte';

  /** Renders only the rows in view: providers list tens of thousands of channels. */
  let {
    items,
    rowHeight,
    key,
    row,
    header,
    headerHeight = 0,
  }: {
    items: T[];
    rowHeight: number;
    key: (item: T) => string;
    row: Snippet<[T]>;
    /** Scrolls with the rows, above them. */
    header?: Snippet;
    headerHeight?: number;
  } = $props();

  let viewport: HTMLElement;
  let scrollTop = $state(0);
  let height = $state(0);
  const overscan = 6;
  const start = $derived(Math.max(0, Math.floor((scrollTop - headerHeight) / rowHeight) - overscan));
  const visible = $derived(items.slice(start, Math.ceil((scrollTop - headerHeight + height) / rowHeight) + overscan));

  // Its own observer rather than bind:clientHeight, which missed the first layout of a list mounted in a hidden or collapsing area.
  $effect(() => {
    const observer = new ResizeObserver(() => (height = viewport.clientHeight));
    observer.observe(viewport);
    height = viewport.clientHeight;
    return () => observer.disconnect();
  });

  /** Scrolls just enough to show the row. */
  export function reveal(index: number) {
    const top = headerHeight + index * rowHeight;
    if (top < viewport.scrollTop) viewport.scrollTop = top;
    else if (top + rowHeight > viewport.scrollTop + height) viewport.scrollTop = top + rowHeight - height;
  }
</script>

<div class="viewport" bind:this={viewport} onscroll={() => (scrollTop = viewport.scrollTop)}>
  <div class="content" style:height="{headerHeight + items.length * rowHeight}px">
    {#if header}<div class="header" style:height="{headerHeight}px">{@render header()}</div>{/if}
    {#each visible as item, i (key(item))}
      <div class="row" style:top="{headerHeight + (start + i) * rowHeight}px" style:height="{rowHeight}px">{@render row(item)}</div>
    {/each}
  </div>
</div>

<style>
  .viewport {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    scrollbar-width: thin;
    scrollbar-color: rgb(233 236 242 / 0.15) transparent;
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
