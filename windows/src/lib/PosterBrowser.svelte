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
  import type { Snippet } from 'svelte';
  import { ANDROID, plural, t } from './i18n.svelte';
  import CategorySidebar, { type SideEntry } from './CategorySidebar.svelte';
  import { library } from './library.svelte';
  import { categoryKey } from './lineup';
  import { contextMenu } from './menuState.svelte';
  import { toggleCategoryLock } from './parental';
  import Poster from './Poster.svelte';
  import VirtualList from './VirtualList.svelte';

  /**
   * Genres in a column, a grid of posters, and with `details` a panel for the poster picked: a click
   * picks, a double-click plays. Only the rows in view are built: Xtream lists run to tens of thousands.
   */
  let {
    items,
    info,
    empty,
    onpick,
    menu,
    status,
    details,
    continuing = [],
  }: {
    items: T[];
    info: (item: T) => { name: string; group: string | null; poster: string | null; rating: number | null };
    empty: string;
    onpick: (item: T) => void;
    menu?: (e: MouseEvent, item: T) => void;
    /** What was watched of it: films only, a series has its episodes. */
    status?: (item: T) => { progress: number; watched: boolean };
    /** The panel for the picked poster; without it a click plays (or opens) right away. */
    details?: Snippet<[T]>;
    /** Shown above the grid in All. */
    continuing?: ContinueCard[];
  } = $props();

  /** TV: the system moves the focus between posters, but not from the category column across to them. */
  function onkeydown(e: KeyboardEvent) {
    if (!ANDROID || e.key !== 'ArrowRight' || e.defaultPrevented) return;
    const active = document.activeElement;
    if (!(active instanceof Element) || !active.closest('.side')) return;
    const first = document.querySelector<HTMLElement>('.browser .poster');
    if (!first) return;
    e.preventDefault();
    first.focus();
  }

  const MIN = 150;
  const GAP = 18;
  const ALL = 'all';
  const MINE = 'mine';
  let category = $state(ALL);
  let width = $state(0);
  let picked = $state.raw<T | null>(null);

  const groups = $derived.by(() => {
    const counts = new Map<string, number>();
    for (const i of items) {
      const g = info(i).group;
      if (g) counts.set(g, (counts.get(g) ?? 0) + 1);
    }
    return counts;
  });
  const mine = $derived(items.filter((i) => library.myList.has(i.id)));
  const lists = $derived<SideEntry[]>([
    { id: ALL, label: t('All'), count: items.length, glyph: '' },
    { id: MINE, label: t('My list'), count: mine.length, glyph: '', tint: 'var(--accent)' },
  ]);
  const categories = $derived([...groups].map(([g, count]): SideEntry => ({ id: `cat:${g}`, label: g, count })));

  const shown = $derived(category === ALL ? items : category === MINE ? mine : items.filter((i) => `cat:${info(i).group}` === category));
  const caption = $derived(category === ALL ? t('All') : category === MINE ? t('My list') : category.slice(4));
  // Room for the scrollbar and the hover zoom at the edges.
  const inner = $derived(width - 20 - 14);
  const columns = $derived(Math.max(1, Math.floor((inner + GAP) / (MIN + GAP))));
  const card = $derived(Math.floor(Math.max(MIN, (inner - GAP * (columns - 1)) / columns)));
  // Poster (2:3), two lines of title, then the gap to the next row.
  const rowHeight = $derived(Math.round(card * 1.5 + 8 + 44 + GAP));
  const header = $derived(category === ALL && continuing.length ? 40 + rowHeight : 0);
  const rows = $derived(
    Array.from({ length: Math.ceil(shown.length / columns) }, (_, r) => ({ r, cells: shown.slice(r * columns, (r + 1) * columns) })),
  );

  $effect(() => {
    if (category !== ALL && category !== MINE && !groups.has(category.slice(4))) category = ALL;
  });

  // The first poster is the one the panel shows until another is picked.
  $effect(() => {
    if (details && (!picked || !items.includes(picked))) picked = shown[0] ?? null;
  });

  function sideMenu(e: MouseEvent, entry: SideEntry) {
    if (!entry.id.startsWith('cat:')) return;
    const name = entry.id.slice(4);
    const key = categoryKey(name);
    contextMenu.open(e, [
      {
        label: t(library.lockedGroups.has(key) ? 'Take “{name}” out of the PIN' : 'Put “{name}” behind the PIN', { name }),
        icon: '',
        action: () => void toggleCategoryLock(key),
      },
    ]);
  }
</script>

<svelte:window {onkeydown} />

<div class="browser">
  {#if !items.length}
    <p class="empty">{empty}</p>
  {:else}
    <CategorySidebar {lists} pinned={[]} {categories} selected={category} onpick={(id) => (category = id)} onmenu={sideMenu} listsTitle={t('Library')} />
    <section class="catalog">
      <header>
        <h2>{caption}</h2>
        <span class="count mono">{plural(shown.length, '{n} title', '{n} titles')}</span>
      </header>
      {#if shown.length}
        <div class="grid" bind:clientWidth={width}>
          {#key category}
            <VirtualList items={rows} {rowHeight} key={(row) => String(row.r)} row={posterRow} header={header ? continueRow : undefined} headerHeight={header} />
          {/key}
        </div>
      {:else}
        <p class="none">{category === MINE ? t('Your list is empty. Use “My list” on a title to keep it here.') : t('Nothing matches.')}</p>
      {/if}
    </section>
    {#if details && picked}
      <aside class="details">{@render details(picked)}</aside>
    {/if}
  {/if}
</div>

{#snippet continueRow()}
  <h3>{t('Continue watching')}</h3>
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
        saved={library.myList.has(item.id)}
        selected={!!details && picked === item}
        onclick={() => (details ? (picked = item) : onpick(item))}
        ondblclick={details ? () => onpick(item) : undefined}
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
  }
  .empty {
    margin: auto;
    padding-bottom: 64px;
    font-size: 15px;
    color: var(--muted);
  }
  .catalog {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    padding: 0 10px 0 24px;
  }
  header {
    flex: none;
    height: 62px;
    display: flex;
    align-items: baseline;
    gap: 10px;
    padding-top: 22px;
    box-sizing: border-box;
  }
  h2 {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .count {
    font-size: 13px;
    color: var(--muted);
  }
  .none {
    margin: 10px 0;
    font-size: 14px;
    color: var(--muted);
  }
  .grid {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    /* Room for the hover zoom at the edges. */
    margin: 0 -10px;
    padding: 0 10px;
  }
  h3 {
    margin: 0;
    padding: 14px 0 0;
    height: 26px;
    font-size: 14px;
    font-weight: 600;
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
    padding-top: 10px;
  }
  .details {
    width: 360px;
    flex: none;
    overflow-y: auto;
    padding: 22px 24px;
    border-left: 1px solid var(--border);
    background: #0d1016;
  }
  /* A small window (1024 px, the smallest): a narrower panel, so the grid keeps two columns. */
  @media (max-width: 1100px) {
    .details {
      width: 232px;
      padding: 18px;
    }
  }
</style>
