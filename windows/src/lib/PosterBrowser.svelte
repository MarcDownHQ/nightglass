<script lang="ts" generics="T extends { id: string }">
  import NeonTab from './NeonTab.svelte';
  import Poster from './Poster.svelte';
  import VirtualList from './VirtualList.svelte';

  /** Category filters over a grid of posters. Only the rows in view are built: Xtream lists run to tens of thousands. */
  let {
    items,
    info,
    empty,
    onpick,
  }: {
    items: T[];
    info: (item: T) => { name: string; group: string | null; poster: string | null; rating: number | null };
    empty: string;
    onpick: (item: T) => void;
  } = $props();

  const MIN = 160;
  const GAP = 22;
  let category = $state('All');
  let width = $state(0);
  let tabs: HTMLElement | undefined = $state();

  const categories = $derived.by(() => {
    const seen = new Set<string>();
    for (const i of items) {
      const g = info(i).group;
      if (g) seen.add(g);
    }
    return ['All', ...seen];
  });

  const shown = $derived(category === 'All' ? items : items.filter((i) => info(i).group === category));
  // The measured width includes the glow padding (2 × 10) and room for the scrollbar.
  const inner = $derived(width - 20 - 14);
  const columns = $derived(Math.max(1, Math.floor((inner + GAP) / (MIN + GAP))));
  const card = $derived(Math.floor(Math.max(MIN, (inner - GAP * (columns - 1)) / columns)));
  // Poster (2:3), two lines of title, then the gap to the next row.
  const rowHeight = $derived(Math.round(card * 1.5 + 8 + 44 + 26));
  const rows = $derived(
    Array.from({ length: Math.ceil(shown.length / columns) }, (_, r) => ({ r, cells: shown.slice(r * columns, (r + 1) * columns) })),
  );

  $effect(() => {
    if (!categories.includes(category)) category = 'All';
  });

  $effect(() => {
    void category;
    tabs?.querySelector('[aria-pressed="true"]')?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  });
</script>

<div class="browser">
  {#if !items.length}
    <p class="empty">{empty}</p>
  {:else}
    <div class="tabs" bind:this={tabs}>
      {#each categories as c (c)}
        <NeonTab title={c} selected={c === category} onclick={() => (category = c)} />
      {/each}
    </div>
    <div class="grid" bind:clientWidth={width}>
      {#key category}
        <VirtualList items={rows} {rowHeight} key={(row) => String(row.r)} row={posterRow} />
      {/key}
    </div>
  {/if}
</div>

{#snippet posterRow(row: { cells: T[] })}
  <div class="cells" style:grid-template-columns="repeat({columns}, {card}px)" style:gap="{GAP}px">
    {#each row.cells as item (item.id)}
      {@const i = info(item)}
      <Poster title={i.name} url={i.poster} rating={i.rating} onclick={() => onpick(item)} />
    {/each}
  </div>
{/snippet}

<style>
  .browser {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 4px 28px 0;
  }
  .empty {
    margin: auto;
    padding-bottom: 64px;
    font-size: 16px;
    color: var(--dim);
  }
  .tabs {
    display: flex;
    gap: 8px;
    padding: 8px 4px;
    overflow-x: auto;
    scrollbar-width: none;
  }
  .grid {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    /* Room for the hover glow and zoom at the edges. */
    margin: 0 -10px;
    padding: 0 10px;
  }
  .cells {
    display: grid;
    padding-top: 14px;
  }
</style>
