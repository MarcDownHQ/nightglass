<script lang="ts">
  import ChannelLogo from './ChannelLogo.svelte';
  import { newList, openChannelMenu, openVariantMenu } from './channelMenu';
  import { organizer } from './dialogs.svelte';
  import HudPanel from './HudPanel.svelte';
  import { library } from './library.svelte';
  import { multiview } from './tilesState.svelte';
  import { categoryKey, categoryLabel, type SortMode } from './lineup';
  import { contextMenu, type MenuEntry } from './menuState.svelte';
  import type { LiveItem } from './models';
  import { dial, nav, typing } from './nav.svelte';
  import NeonTab from './NeonTab.svelte';
  import NeonText from './NeonText.svelte';
  import NowNextPanel from './NowNextPanel.svelte';
  import { player } from './player.svelte';
  import ProgressLine from './ProgressLine.svelte';
  import StopButton from './StopButton.svelte';
  import { store } from './store.svelte';
  import Tag from './Tag.svelte';
  import { displayName, neon, progress, qualityTag } from './theme';
  import { videoHole } from './video.svelte';
  import VirtualList from './VirtualList.svelte';

  /** A tab: `list:<id>` a favourites list, `cat:<key>` a category, or all / recent / hidden. */
  interface Tab {
    id: string;
    label: string;
  }

  let tab = $state('all');
  let listView: { reveal: (index: number) => void } | undefined = $state();
  let tabsEl: HTMLElement;
  let dragged = $state<string | null>(null);

  const playingId = $derived(player.current?.item?.id);
  /** The row of the channel playing: the channel itself, or the merged row holding its feed. */
  const isPlaying = (row: LiveItem) => row.id === playingId || !!row.variants?.some((v) => v.id === playingId);

  const byId = $derived.by(() => {
    const map = new Map<string, LiveItem>();
    for (const row of store.lineup) {
      map.set(row.id, row);
      for (const v of row.variants ?? []) map.set(v.id, row);
    }
    return map;
  });

  const hidden = $derived.by(() => {
    if (store.channels === store.allChannels) return [];
    const shown = new Set(store.channels);
    // Hidden by the user, not locked: what the PIN guards never shows here.
    return store.allChannels.filter((c) => !shown.has(c) && !library.isLocked(c.channel.group && categoryKey(c.channel.group)));
  });
  const recent = $derived([...new Set(library.recents.flatMap((id) => byId.get(id) ?? []))]);

  /** A list's channels in its own order; a channel in it twice through two feeds shows once. */
  function listRows(listId: string): LiveItem[] {
    const items = library.favoriteLists.find((l) => l.id === listId)?.items ?? [];
    return [...new Set(items.flatMap((id) => byId.get(id) ?? []))];
  }

  const tabs = $derived.by((): Tab[] => [
    // The first list shows once it has something; the lists made on purpose show even empty.
    ...library.favoriteLists
      .filter((l, i) => i > 0 || l.items.length)
      .map((l) => ({ id: `list:${l.id}`, label: `★ ${l.name}` })),
    ...(recent.length ? [{ id: 'recent', label: '↺ Recent' }] : []),
    { id: 'all', label: 'All' },
    ...store.categories.map((c) => ({ id: `cat:${c.key}`, label: c.label })),
    ...(hidden.length ? [{ id: 'hidden', label: 'Hidden' }] : []),
  ]);

  const listId = $derived(tab.startsWith('list:') ? tab.slice(5) : undefined);

  const list = $derived.by((): LiveItem[] => {
    if (tab === 'all') return store.lineup;
    if (tab === 'recent') return recent;
    if (tab === 'hidden') return hidden;
    if (listId) return listRows(listId);
    const key = tab.slice(4);
    return store.lineup.filter((c) => c.channel.group && categoryKey(c.channel.group) === key);
  });

  const caption = $derived(tab === 'all' ? 'All channels' : (tabs.find((t) => t.id === tab)?.label ?? ''));

  $effect(() => {
    if (!tabs.some((t) => t.id === tab)) tab = 'all';
  });

  // Keep the playing channel in view, whoever tuned it (keys, dial, search, zapping in fullscreen).
  $effect(() => {
    const i = list.findIndex(isPlaying);
    if (i >= 0) listView?.reveal(i);
  });

  $effect(() => {
    void tab;
    tabsEl?.querySelector('[aria-pressed="true"]')?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  });

  // A channel picked in the guide or the search: show it in All and tune it.
  $effect(() => {
    const id = nav.focusChannel;
    const row = id && (byId.get(id) ?? store.channels.find((c) => c.id === id));
    if (!row) return;
    nav.focusChannel = null;
    tab = 'all';
    void store.tune(row);
  });

  function move(delta: number) {
    if (!list.length) return;
    const i = list.findIndex(isPlaying);
    void store.tune(list[i < 0 ? 0 : (i + delta + list.length) % list.length]);
  }

  function shiftTab(delta: number) {
    const i = tabs.findIndex((t) => t.id === tab);
    tab = tabs[(i + delta + tabs.length) % tabs.length].id;
  }

  function tabMenu(e: MouseEvent, t: Tab) {
    const entries: MenuEntry[] = [];
    if (t.id.startsWith('cat:')) {
      const key = t.id.slice(4);
      entries.push(
        { label: `Hide “${t.label}”`, icon: '', action: () => library.toggleHidden(library.hiddenGroups, key) },
        { label: 'Organize categories…', icon: '', action: () => (organizer.open = true) },
      );
    } else if (t.id.startsWith('list:')) {
      entries.push({ label: 'Organize lists…', icon: '', action: () => (organizer.open = true) });
    } else return;
    contextMenu.open(e, entries);
  }

  const SORTS: [SortMode, string][] = [
    ['playlist', 'Playlist order'],
    ['name', 'Name'],
    ['number', 'Number'],
    ['watched', 'Most watched'],
  ];

  function viewMenu(e: MouseEvent) {
    const set = <K extends 'sort' | 'country' | 'mergeDuplicates'>(key: K, value: (typeof library)[K]) => {
      library[key] = value;
      library.persist(key);
    };
    contextMenu.open(e, [
      { heading: 'Order' },
      ...SORTS.map(([mode, label]) => ({ label, checked: library.sort === mode, action: () => set('sort', mode) })),
      { heading: 'Feeds' },
      { label: 'Merge HD, SD and backup feeds', checked: library.mergeDuplicates, action: () => set('mergeDuplicates', !library.mergeDuplicates) },
      ...(store.countries.length > 1
        ? [
            { heading: 'Country' },
            { label: 'All countries', checked: !library.country, action: () => set('country', null) },
            ...store.countries.slice(0, 20).map((code) => ({ label: code, checked: library.country === code, action: () => set('country', code) })),
          ]
        : []),
      { heading: 'Organize' },
      { label: 'Categories and lists…', icon: '', action: () => (organizer.open = true) },
      { label: 'New favorites list…', icon: '', action: () => void newList() },
    ]);
  }

  /** `S`: the first list, or the list shown. */
  function toggleFavorite() {
    const item = player.current?.item;
    if (item) library.toggleFavorite(listId ?? library.favoriteLists[0]?.id ?? '', item.id);
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.defaultPrevented || player.expanded || nav.searching || typing(e) || e.ctrlKey || e.metaKey) return;
    const playing = list.find(isPlaying);
    if (e.altKey) {
      if (!listId || !playing || (e.key !== 'ArrowUp' && e.key !== 'ArrowDown')) return;
      library.moveFavorite(listId, playing.id, e.key === 'ArrowUp' ? -1 : 1);
    } else if (e.key === 'ArrowDown') move(1);
    else if (e.key === 'ArrowUp') move(-1);
    else if (e.key === 'ArrowRight') shiftTab(1);
    else if (e.key === 'ArrowLeft') shiftTab(-1);
    else if (e.key === 'Enter') void player.expand();
    else if (e.key === 'Escape') player.stop();
    else if (e.key === ' ') player.togglePause();
    else if (e.key.toLowerCase() === 's') toggleFavorite();
    else if (e.key.toLowerCase() === 'r' && player.current?.live) void player.toggleRecording();
    else if (e.key.toLowerCase() === 'p') void player.toggleMini();
    else if (/^[0-9]$/.test(e.key)) {
      // In a favourites list the numbers are the list's own: 1 is its first channel.
      const rows = list;
      const inList = !!listId;
      dial.type(e.key, (n) => {
        const item = inList ? rows[n - 1] : store.byNumber(n);
        if (!item) return;
        if (!inList) tab = 'all';
        void store.tune(byId.get(item.id) ?? item);
      });
    } else return;
    e.preventDefault();
  }

  function onwheel(e: WheelEvent) {
    e.preventDefault();
    player.setVolume(library.volume + (e.deltaY < 0 ? 5 : -5));
  }

  function drop(target: LiveItem | null) {
    if (listId && dragged && dragged !== target?.id) library.moveFavorite(listId, dragged, { before: target?.id ?? null });
    dragged = null;
  }
</script>

<svelte:window {onkeydown} />

<div class="live">
  <div class="left">
    <div class="bar">
      <div class="tabs" bind:this={tabsEl}>
        {#each tabs as t (t.id)}
          <NeonTab title={t.label} selected={t.id === tab} onclick={() => (tab = t.id)} oncontextmenu={(e) => tabMenu(e, t)} />
        {/each}
      </div>
      <button class="icon view" title="Multi-view: 2 or 4 channels at once" onclick={() => multiview.show()}>&#xF0E2;</button>
      <button class="icon view" title="Order, feeds, countries, categories" onclick={viewMenu}>&#xE71C;</button>
    </div>
    <HudPanel {caption} trailing={String(list.length)} class="channels">
      {#key tab}
        <VirtualList items={list} rowHeight={66} key={(c) => c.id} row={channelRow} bind:this={listView} />
      {/key}
    </HudPanel>
  </div>

  <div class="right">
    {#if player.current && !player.expanded}
      <div class="preview" use:videoHole={14} ondblclick={() => player.expand()} {onwheel} role="presentation">
        {#if player.loading || player.error}
          <p class="status" class:error={player.error}>{player.error ?? `Loading ${displayName(player.current.title)}…`}</p>
        {/if}
        <div class="corner">
          {#if player.recording}<span class="rec mono" title={player.recording}>● REC</span>{/if}
          <StopButton />
        </div>
        {#if dial.digits}<div class="dial"><NeonText text={dial.digits} size={90} /></div>{/if}
      </div>
    {:else}
      <div class="preview empty">
        <p class="status" class:error={player.error}>{player.error ?? 'Select a channel'}</p>
        {#if dial.digits}<div class="dial"><NeonText text={dial.digits} size={90} /></div>{/if}
      </div>
    {/if}
    <NowNextPanel item={player.current?.item} />
  </div>
</div>

{#snippet channelRow(item: LiveItem)}
  {@const ch = item.channel}
  {@const c = neon(ch.name)}
  {@const np = store.nowNext(ch)}
  {@const variants = item.variants?.length ?? 0}
  {@const quality = qualityTag(ch.name)}
  <button
    class="row"
    class:on={isPlaying(item)}
    class:dragging={dragged === item.id}
    style:--c={c}
    draggable={!!listId}
    onclick={() => store.tune(item)}
    ondblclick={() => player.expand()}
    oncontextmenu={(e) => openChannelMenu(e, item, listId)}
    ondragstart={(e) => {
      dragged = item.id;
      e.dataTransfer?.setData('text/plain', item.id);
    }}
    ondragend={() => (dragged = null)}
    ondragover={(e) => listId && e.preventDefault()}
    ondrop={(e) => {
      e.preventDefault();
      drop(item);
    }}
  >
    <span class="num mono">{listId ? list.indexOf(item) + 1 : (ch.number ?? '')}</span>
    <ChannelLogo channel={ch} color={c} />
    <span class="text">
      <b>{displayName(ch.name)}</b>
      <small>{np.now?.title ?? (ch.group ? categoryLabel(ch.group) : '')}</small>
      <span class="progress"><ProgressLine value={np.now ? progress(np.now, store.now) : 0} color={c} /></span>
    </span>
    {#if library.isFavorite(item.id)}<span class="star">★</span>{/if}
    {#if variants > 1}
      <span
        class="feeds"
        role="button"
        tabindex="-1"
        title="{variants} feeds of this channel"
        onclick={(e) => {
          e.stopPropagation();
          openVariantMenu(e, item);
        }}
        onkeydown={() => {}}
      >
        <Tag text={quality ? `${quality} · ${variants}` : `${variants} feeds`} />
      </span>
    {:else if quality}<Tag text={quality} />{/if}
  </button>
{/snippet}

<style>
  .live {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(440px, 1fr) minmax(0, 820px);
    gap: 32px;
    padding: 4px 28px 20px;
  }
  .left {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
    min-height: 0;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }
  .view {
    flex: none;
    padding: 8px;
    font-size: 15px;
    color: var(--dim);
  }
  .view:hover {
    color: var(--cyan);
  }
  .tabs {
    flex: 1;
    min-width: 0;
    display: flex;
    gap: 8px;
    padding: 8px 4px;
    overflow-x: auto;
    scrollbar-width: none;
  }
  .left :global(.channels) {
    flex: 1;
  }
  .right {
    display: flex;
    flex-direction: column;
    gap: 24px;
    min-width: 0;
  }
  .preview {
    position: relative;
    aspect-ratio: 16 / 9;
    border-radius: 14px;
    /* Glow on the frame only, never on the picture. */
    outline: 1.5px solid rgb(5 217 232 / 0.7);
    box-shadow: 0 0 14px rgb(5 217 232 / 0.45);
  }
  .preview.empty {
    display: grid;
    place-items: center;
    background: #000;
  }
  .status {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    margin: 0;
    padding: 10px 18px;
    max-width: 80%;
    border-radius: 10px;
    background: rgb(5 1 15 / 0.75);
    font-size: 16px;
    font-weight: 500;
    text-align: center;
    color: var(--dim);
  }
  .empty .status {
    background: none;
  }
  .status.error {
    color: var(--magenta);
  }
  .corner {
    position: absolute;
    top: 10px;
    right: 10px;
  }
  .dial {
    position: absolute;
    top: 40px;
    right: 40px;
  }
  .row {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 0 16px;
    text-align: left;
    position: relative;
  }
  .row:hover {
    background: rgb(243 238 255 / 0.04);
  }
  .row.on {
    background: color-mix(in srgb, var(--c) 14%, transparent);
  }
  .row.on::before {
    content: '';
    position: absolute;
    left: 0;
    top: 10px;
    bottom: 10px;
    width: 3px;
    border-radius: 2px;
    background: var(--c);
    box-shadow: 0 0 6px var(--c);
  }
  .num {
    flex: none;
    width: 32px;
    font-size: 12px;
    color: var(--dim);
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .text b,
  .text small {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .text b {
    font-size: 18px;
    font-weight: 600;
  }
  .row.on b {
    color: #fff;
    text-shadow: 0 0 6px var(--c);
  }
  .text small {
    font-size: 14px;
    color: var(--dim);
  }
  .progress {
    padding-top: 4px;
  }
  .row.dragging {
    opacity: 0.4;
  }
  .row[draggable='true'] {
    cursor: grab;
  }
  .feeds {
    cursor: pointer;
  }
  .feeds:hover :global(*) {
    filter: brightness(1.4);
  }
  .corner {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .rec {
    font-size: 12px;
    color: var(--magenta);
    text-shadow: 0 0 6px var(--magenta);
  }
  .star {
    color: var(--acid);
  }
</style>
