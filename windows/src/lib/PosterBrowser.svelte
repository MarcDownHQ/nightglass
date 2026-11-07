<script module lang="ts">
/** A card of the Continue row: a film or an episode left unfinished. */
export interface ContinueCard {
  id: string;
  title: string;
  subtitle: string;
  poster: string | null;
  progress: number;
  play: () => void;
  menu: (e: MouseEvent) => void;
}
</script>

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
    menu,
    status,
    continuing = [],
  }: {
    items: T[];
    info: (item: T) => { name: string; group: string | null; poster: string | null; rating: number | null };
    empty: string;
    onpick: (item: T) => void;
    menu?: (e: MouseEvent, item: T) => void;
    /** What was watched of it: films only, a series has its episodes. */
    status?: (item: T) => { progress: number; watched: boolean };
    /** Shown above the grid in All. */
    continuing?: ContinueCard[];
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
  const header = $derived(category === 'All' && continuing.length ? 40 + rowHeight : 0);
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
        <VirtualList items={rows} {rowHeight} key={(row) => String(row.r)} row={posterRow} header={header ? continueRow : undefined} headerHeight={header} />
      {/key}
    </div>
  {/if}
</div>

{#snippet continueRow()}
  <h3 class="mono">Continue watching</h3>
  <div class="strip" style:gap="{GAP}px" style:grid-auto-columns="{card}px">
    {#each continuing as c (c.id)}
      <Poster title={c.title} subtitle={c.subtitle} url={c.poster} rating={null} progress={c.progress} onclick={c.play} oncontextmenu={c.menu} />
    {/each}
  </div>
{/snippet}

{#snippet posterRow(row: { cells: T[] })}
  <div class="cells" style:grid-template-columns="repeat({columns}, {card}px)" style:gap="{GAP}px">
    {#each row.cells as item (item.id)}
      {@const i = info(item)}
      {@const s = status?.(item)}
      <Poster
        title={i.name}
        url={i.poster}
        rating={i.rating}
        progress={s?.progress}
        watched={s?.watched}
        onclick={() => onpick(item)}
        oncontextmenu={menu && ((e) => menu(e, item))}
      />
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
  h3 {
    margin: 0;
    padding: 14px 0 0;
    height: 26px;
    font-size: 12px;
    font-weight: 500;
    color: var(--acid);
  }
  .strip {
    display: grid;
    grid-auto-flow: column;
    justify-content: start;
    overflow-x: auto;
    scrollbar-width: none;
    padding-top: 14px;
  }
  .cells {
    display: grid;
    padding-top: 14px;
  }
</style>
