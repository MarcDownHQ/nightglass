<script lang="ts">
  import { t } from './i18n.svelte';
  import CategoryPicker from './CategoryPicker.svelte';
  import ChannelLogo from './ChannelLogo.svelte';
  import { newList, openChannelMenu, openVariantMenu } from './channelMenu';
  import { organizer } from './dialogs.svelte';
  import HudPanel from './HudPanel.svelte';
  import { friendly } from './errors';
  import { library, listLabel } from './library.svelte';
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
      .map((l) => ({ id: `list:${l.id}`, label: `★ ${listLabel(l)}` })),
    ...(recent.length ? [{ id: 'recent', label: t('↺ Recent') }] : []),
    { id: 'all', label: t('All') },
    ...store.categories.map((c) => ({ id: `cat:${c.key}`, label: c.label })),
    ...(hidden.length ? [{ id: 'hidden', label: t('Hidden') }] : []),
  ]);

  /** Past this many categories the strip keeps only the shortcuts and the category shown; the ☰ list has them all. */
  const STRIP_MAX = 30;
  const stripTabs = $derived(store.categories.length <= STRIP_MAX ? tabs : tabs.filter((x) => !x.id.startsWith('cat:') || x.id === tab));

  const listId = $derived(tab.startsWith('list:') ? tab.slice(5) : undefined);

  const list = $derived.by((): LiveItem[] => {
    if (tab === 'all') return store.lineup;
    if (tab === 'recent') return recent;
    if (tab === 'hidden') return hidden;
    if (listId) return listRows(listId);
    const key = tab.slice(4);
    return store.lineup.filter((c) => c.channel.group && categoryKey(c.channel.group) === key);
  });

  /** Channels per category key, for the counts in the category picker. */
  const perCategory = $derived.by(() => {
    const map = new Map<string, number>();
    for (const c of store.lineup) if (c.channel.group) map.set(categoryKey(c.channel.group), (map.get(categoryKey(c.channel.group)) ?? 0) + 1);
    return map;
  });

  function tabCount(id: string): number {
    if (id === 'all') return store.lineup.length;
    if (id === 'recent') return recent.length;
    if (id === 'hidden') return hidden.length;
    if (id.startsWith('list:')) return listRows(id.slice(5)).length;
    return perCategory.get(id.slice(4)) ?? 0;
  }

  /** The strip has no scrollbar: the wheel scrolls it sideways. */
  function tabsWheel(e: WheelEvent) {
    if (!e.deltaY || e.shiftKey) return;
    e.preventDefault();
    tabsEl.scrollLeft += e.deltaY;
  }

  const caption = $derived(tab === 'all' ? t('All channels') : (tabs.find((x) => x.id === tab)?.label ?? ''));

  $effect(() => {
    if (!tabs.some((x) => x.id === tab)) tab = 'all';
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
    const i = tabs.findIndex((x) => x.id === tab);
    tab = tabs[(i + delta + tabs.length) % tabs.length].id;
  }

  function tabMenu(e: MouseEvent, tb: Tab) {
    const entries: MenuEntry[] = [];
    if (tb.id.startsWith('cat:')) {
      const key = tb.id.slice(4);
      entries.push(
        { label: t('Hide “{name}”', { name: tb.label }), icon: '', action: () => library.toggleHidden(library.hiddenGroups, key) },
        { label: t('Organize categories…'), icon: '', action: () => (organizer.open = true) },
      );
    } else if (tb.id.startsWith('list:')) {
      entries.push({ label: t('Organize lists…'), icon: '', action: () => (organizer.open = true) });
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
      { heading: t('Order') },
      ...SORTS.map(([mode, label]) => ({ label: t(label), checked: library.sort === mode, action: () => set('sort', mode) })),
      { heading: t('Feeds') },
      { label: t('Merge HD, SD and backup feeds'), checked: library.mergeDuplicates, action: () => set('mergeDuplicates', !library.mergeDuplicates) },
      ...(store.countries.length > 1
        ? [
            { heading: t('Country') },
            { label: t('All countries'), checked: !library.country, action: () => set('country', null) },
            ...store.countries.slice(0, 20).map((code) => ({ label: code, checked: library.country === code, action: () => set('country', code) })),
          ]
        : []),
      { heading: t('Organize') },
      { label: t('Categories and lists…'), icon: '', action: () => (organizer.open = true) },
      { label: t('New favorites list…'), icon: '', action: () => void newList() },
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
    else if (e.key === 'Enter' || e.key.toLowerCase() === 'f') void player.expand();
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
      <CategoryPicker {tabs} selected={tab} count={tabCount} onpick={(id) => (tab = id)} />
      <div class="tabs" bind:this={tabsEl} onwheel={tabsWheel}>
        {#each stripTabs as tb (tb.id)}
          <NeonTab title={tb.label} selected={tb.id === tab} onclick={() => (tab = tb.id)} oncontextmenu={(e) => tabMenu(e, tb)} />
        {/each}
      </div>
      <button class="icon view" title={t('Multi-view: 2 or 4 channels at once')} onclick={() => multiview.show()}>&#xF0E2;</button>
      <button class="icon view filter" title={t('Order, feeds, countries, categories')} onclick={viewMenu}>&#xE71C;</button>
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
          <div class="status" class:error={player.error}>
            {player.error ? friendly(player.error) : player.reconnecting ? t('Signal lost: reconnecting (attempt {n})…', { n: player.reconnecting }) : t('Loading {name}…', { name: displayName(player.current.title) })}
            {#if player.error}<button class="retry" onclick={() => player.retry()}>{t('Retry')}</button>{/if}
          </div>
        {/if}
        <div class="corner">
          {#if player.recording}<span class="rec mono" title={player.recording}>● REC</span>{/if}
          <StopButton />
        </div>
        {#if dial.digits}<div class="dial"><NeonText text={dial.digits} size={90} /></div>{/if}
      </div>
    {:else}
      <div class="preview empty">
        <p class="status" class:error={player.error}>{player.error ? friendly(player.error) : t('Select a channel')}</p>
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
    <span class="side">
    {#if library.isFavorite(item.id)}<span class="star">★</span>{/if}
    {#if variants > 1}
      <span
        class="feeds"
        role="button"
        tabindex="-1"
        title={t('{n} feeds of this channel', { n: variants })}
        onclick={(e) => {
          e.stopPropagation();
          openVariantMenu(e, item);
        }}
        onkeydown={() => {}}
      >
        <Tag text={quality ? `${quality} · ${variants}` : t('{n} feeds', { n: variants })} />
      </span>
    {:else if quality}<Tag text={quality} />{/if}
    </span>
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
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
    min-height: 0;
  }
  /* The tabs keep 8 px of room for their glow; the bar gives it back so they line up with the preview. */
  .bar {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    margin-top: -8px;
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
  /* The funnel glyph sits on the baseline, a pixel lower than the grid one: lifted so both look centred. */
  .view.filter {
    padding: 7px 8px 9px;
  }
  .tabs {
    flex: 1;
    min-width: 0;
    display: flex;
    gap: 8px;
    padding: 8px 4px;
    overflow-x: auto;
    scrollbar-width: none;
    /* The tabs that don't fit fade out instead of being cut. */
    mask-image: linear-gradient(to right, #000 calc(100% - 28px), transparent);
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
    text-align: right;
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
  /* Kept when empty, so rows without a programme line up with the others. */
  .text small {
    min-height: 1lh;
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
    display: flex;
    cursor: pointer;
  }
  .feeds:hover :global(*) {
    filter: brightness(1.4);
  }
  .retry {
    display: block;
    margin: 10px auto 0;
    padding: 5px 16px;
    border-radius: 999px;
    border: 1px solid var(--cyan);
    color: var(--cyan);
    font-size: 14px;
    font-weight: 600;
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
  /* A fixed column for the star and the tags, so every progress line ends at the same place. */
  .side {
    flex: none;
    width: 76px;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
