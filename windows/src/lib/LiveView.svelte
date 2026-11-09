<script lang="ts">
  import { t } from './i18n.svelte';
  import CategorySidebar, { type SideEntry } from './CategorySidebar.svelte';
  import ChannelLogo from './ChannelLogo.svelte';
  import { newList, openChannelMenu, openVariantMenu } from './channelMenu';
  import { organizer } from './dialogs.svelte';
  import { options } from './options.svelte';
  import { friendly } from './errors';
  import { library, listLabel } from './library.svelte';
  import LiveDetails from './LiveDetails.svelte';
  import { multiview } from './tilesState.svelte';
  import { categoryKey, categoryLabel, type SortMode } from './lineup';
  import { contextMenu, type MenuEntry } from './menuState.svelte';
  import type { LiveItem } from './models';
  import { dial, nav, typing } from './nav.svelte';
  import NeonText from './NeonText.svelte';
  import { player } from './player.svelte';
  import ProgressLine from './ProgressLine.svelte';
  import StopButton from './StopButton.svelte';
  import { store } from './store.svelte';
  import Tag from './Tag.svelte';
  import { displayName, progress, qualityTag } from './theme';
  import { videoHole, wheelScroll } from './video.svelte';
  import VirtualList from './VirtualList.svelte';
  import Volume from './Volume.svelte';

  /** The shown list: `list:<id>` a favourites list, `cat:<key>` a category, or all / recent / hidden. */
  let tab = $state('all');
  let listView: { reveal: (index: number) => void } | undefined = $state();
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

  /** Channels per category key, for the counts in the sidebar. */
  const perCategory = $derived.by(() => {
    const map = new Map<string, number>();
    for (const c of store.lineup) if (c.channel.group) map.set(categoryKey(c.channel.group), (map.get(categoryKey(c.channel.group)) ?? 0) + 1);
    return map;
  });

  const lists = $derived.by((): SideEntry[] => [
    ...library.favoriteLists.map((l) => ({ id: `list:${l.id}`, label: listLabel(l), count: listRows(l.id).length, glyph: '', tint: 'var(--fav)' })),
    ...(recent.length ? [{ id: 'recent', label: t('Recent'), count: recent.length, glyph: '' }] : []),
    { id: 'all', label: t('All channels'), count: store.lineup.length, glyph: '' },
  ]);
  const categories = $derived(store.categories.map((c): SideEntry => ({ id: `cat:${c.key}`, label: c.label, count: perCategory.get(c.key) ?? 0 })));
  const pinned = $derived(categories.filter((c) => library.pinnedGroups.has(c.id.slice(4))).map((c) => ({ ...c, glyph: '' })));
  const hiddenEntry = $derived<SideEntry | undefined>(hidden.length ? { id: 'hidden', label: t('Hidden'), count: hidden.length, glyph: '' } : undefined);
  /** Every entry once, in sidebar order: what ← → step through. */
  const tabs = $derived([...lists, ...categories, ...(hiddenEntry ? [hiddenEntry] : [])]);

  const listId = $derived(tab.startsWith('list:') ? tab.slice(5) : undefined);

  const list = $derived.by((): LiveItem[] => {
    if (tab === 'all') return store.lineup;
    if (tab === 'recent') return recent;
    if (tab === 'hidden') return hidden;
    if (listId) return listRows(listId);
    const key = tab.slice(4);
    return store.lineup.filter((c) => c.channel.group && categoryKey(c.channel.group) === key);
  });

  const caption = $derived(tabs.find((x) => x.id === tab)?.label ?? '');

  /** The list the star adds to: the one shown, else the first. */
  const starList = $derived(listId ?? library.favoriteLists[0]?.id);
  const starred = (id: string) => !!starList && !!library.favoriteLists.find((l) => l.id === starList)?.items.includes(id);
  const toggleStar = (id: string) => starList && library.toggleFavorite(starList, id);

  $effect(() => {
    if (!tabs.some((x) => x.id === tab)) tab = 'all';
  });

  // Keep the playing channel in view, whoever tuned it (keys, dial, search, zapping in fullscreen).
  $effect(() => {
    const i = list.findIndex(isPlaying);
    if (i >= 0) listView?.reveal(i);
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

  function sideMenu(e: MouseEvent, entry: SideEntry) {
    const entries: MenuEntry[] = [];
    if (entry.id.startsWith('cat:')) {
      const key = entry.id.slice(4);
      const isPinned = library.pinnedGroups.has(key);
      entries.push(
        { label: isPinned ? t('Unpin') : t('Pin to the top'), icon: '', action: () => library.toggleHidden(library.pinnedGroups, key) },
        { label: t('Hide “{name}”', { name: entry.label }), icon: '', action: () => library.toggleHidden(library.hiddenGroups, key) },
        { label: t('Organize categories…'), icon: '', action: () => (organizer.open = true) },
      );
    } else if (entry.id.startsWith('list:')) {
      entries.push({ label: t('Organize lists…'), icon: '', action: () => (organizer.open = true) });
    } else return;
    contextMenu.open(e, entries);
  }

  /** Cinema mode has no sidebar: the list's title picks the tab instead. */
  function tabMenu(e: MouseEvent) {
    const entry = (x: SideEntry) => ({ label: x.label, checked: x.id === tab, action: () => (tab = x.id) });
    contextMenu.open(e, [
      ...lists.map(entry),
      ...(categories.length ? [{ heading: t('Categories') }, ...categories.map(entry)] : []),
      ...(hiddenEntry ? [entry(hiddenEntry)] : []),
    ]);
  }

  const toggleCinema = () => options.set('cinema', !options.cinema);
  const slim = $derived(options.cinema && options.slimList);

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
      { label: t('Categories and lists…'), icon: '', action: () => (organizer.open = true) },
      { label: t('New favorites list…'), icon: '', action: () => void newList() },
    ]);
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
    else if (e.key.toLowerCase() === 's') {
      if (player.current?.item) toggleStar(player.current.item.id);
    } else if (e.key.toLowerCase() === 'r' && player.current?.live) void player.toggleRecording();
    else if (e.key.toLowerCase() === 'p') void player.togglePip();
    else if (e.key.toLowerCase() === 't') toggleCinema();
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

<div class="live" class:cinema={options.cinema} class:slim>
  {#if !options.cinema}
    <CategorySidebar {lists} {pinned} {categories} hidden={hiddenEntry} selected={tab} onpick={(id) => (tab = id)} onmenu={sideMenu} onnewlist={() => void newList()} />
  {/if}

  <section class="channels" aria-label={caption}>
    <header>
      {#if slim}
        <button class="icon tool" title={t('Show channel names')} aria-label={t('Show channel names')} onclick={() => options.set('slimList', false)}>&#xE76C;</button>
      {:else}
        {#if options.cinema}
          <button class="pick" title={t('Lists and categories')} onclick={tabMenu}><h2>{caption}</h2><span class="icon">&#xE70D;</span></button>
        {:else}
          <h2>{caption}</h2>
        {/if}
        <span class="count mono">{list.length}</span>
        <span class="spacer"></span>
        <button class="icon tool" title={t('Order, feeds, countries, categories')} aria-label={t('Order, feeds, countries, categories')} onclick={viewMenu}>&#xE71C;</button>
        {#if options.cinema}
          <button class="icon tool" title={t('Logos only')} aria-label={t('Logos only')} onclick={() => options.set('slimList', true)}>&#xE76B;</button>
        {/if}
      {/if}
    </header>
    {#key tab}
      <VirtualList items={list} rowHeight={68} key={(c) => c.id} row={channelRow} bind:this={listView} />
    {/key}
    {#if !list.length}
      <p class="none">{listId ? t('Click the star of a channel to add it here.') : t('No channels here.')}</p>
    {/if}
  </section>

  <section class="right" use:wheelScroll>
    {#if player.current && player.pip}
      <div class="preview empty">
        <div class="status">
          {t('Playing in the mini player')}
          <button class="retry" onclick={() => player.togglePip()}>{t('Bring it back here')}</button>
        </div>
      </div>
    {:else if player.current && !player.expanded}
      <div class="preview" class:paused={player.paused} use:videoHole={14} ondblclick={() => player.expand()} {onwheel} role="presentation">
        {#if player.current.live}<span class="live-badge"><span></span>{t('LIVE')}</span>{/if}
        {#if player.loading || player.error}
          <div class="status" class:error={player.error}>
            {player.error ? friendly(player.error) : player.reconnecting ? t('Signal lost: reconnecting (attempt {n})…', { n: player.reconnecting }) : t('Loading {name}…', { name: displayName(player.current.title) })}
            {#if player.error}<button class="retry" onclick={() => player.retry()}>{t('Retry')}</button>{/if}
          </div>
        {/if}
        <div class="corner">
          {#if player.recording}<span class="rec" title={player.recording}>● REC</span>{/if}
          <StopButton compact />
        </div>
        {#if dial.digits}<div class="dial"><NeonText text={dial.digits} size={90} /></div>{/if}
        <div class="controls">
          <button class="icon ctl" aria-label={player.paused ? t('Play') : t('Pause')} title={player.paused ? t('Play') : t('Pause')} onclick={() => player.togglePause()}>{player.paused ? '' : ''}</button>
          <Volume />
          <span class="spacer"></span>
          <button class="icon ctl" aria-label={t('Multi-view: 2 or 4 channels at once')} title={t('Multi-view: 2 or 4 channels at once')} onclick={() => multiview.show()}>&#xF0E2;</button>
          <button class="icon ctl" aria-label={t('Mini player above the other windows (P)')} title={t('Mini player above the other windows (P)')} onclick={() => player.togglePip()}>&#xE8A7;</button>
          <button class="icon ctl" aria-pressed={options.cinema} aria-label={options.cinema ? t('Leave cinema mode (T)') : t('Cinema mode (T)')} title={options.cinema ? t('Leave cinema mode (T)') : t('Cinema mode (T)')} onclick={toggleCinema}>{options.cinema ? '' : ''}</button>
          <button class="icon ctl" aria-label={t('Fullscreen (F)')} title={t('Fullscreen (F)')} onclick={() => player.expand()}>&#xE740;</button>
        </div>
      </div>
    {:else}
      <div class="preview empty">
        <p class="status" class:error={player.error}>{player.error ? friendly(player.error) : t('Select a channel')}</p>
        {#if dial.digits}<div class="dial"><NeonText text={dial.digits} size={90} /></div>{/if}
      </div>
    {/if}
    <LiveDetails
      item={player.current?.item}
      favorite={!!player.current?.item && starred(player.current.item.id)}
      ontogglefavorite={() => player.current?.item && toggleStar(player.current.item.id)}
    />
  </section>
</div>

{#snippet channelRow(item: LiveItem)}
  {@const ch = item.channel}
  {@const np = store.nowNext(ch)}
  {@const variants = item.variants?.length ?? 0}
  {@const quality = qualityTag(ch.name)}
  {@const star = starred(item.id)}
  <div
    class="row"
    class:on={isPlaying(item)}
    class:dragging={dragged === item.id}
    role="listitem"
    ondragover={(e) => listId && e.preventDefault()}
    ondrop={(e) => {
      e.preventDefault();
      drop(item);
    }}
  >
    <button
      class="main"
      title={slim ? `${ch.number ?? ''} ${displayName(ch.name)}`.trim() : undefined}
      draggable={!!listId}
      onclick={() => store.tune(item)}
      ondblclick={() => player.expand()}
      oncontextmenu={(e) => openChannelMenu(e, item, listId)}
      ondragstart={(e) => {
        dragged = item.id;
        e.dataTransfer?.setData('text/plain', item.id);
      }}
      ondragend={() => (dragged = null)}
    >
      <span class="num mono">{listId ? list.indexOf(item) + 1 : (ch.number ?? '')}</span>
      <ChannelLogo channel={ch} size={44} />
      <span class="text">
        <span class="name"><b>{displayName(ch.name)}</b>{#if quality && variants <= 1}<Tag text={quality} />{/if}</span>
        <small>{np.now?.title ?? (ch.group ? categoryLabel(ch.group) : '')}</small>
        <span class="progress"><ProgressLine value={np.now ? progress(np.now, store.now) : 0} /></span>
      </span>
    </button>
    <span class="side">
    {#if variants > 1}
      <button class="feeds" title={t('{n} feeds of this channel', { n: variants })} onclick={(e) => openVariantMenu(e, item)}>
        <Tag text={quality ? `${quality} · ${variants}` : t('{n} feeds', { n: variants })} />
      </button>
    {/if}
    {#if starList}
      <button class="star" class:fav={star} aria-pressed={star} title={star ? t('Remove from favorites') : t('Add to favorites')} aria-label={star ? t('Remove from favorites') : t('Add to favorites')} onclick={() => toggleStar(item.id)}>{star ? '' : ''}</button>
    {/if}
    </span>
  </div>
{/snippet}

<style>
  .live {
    flex: 1;
    min-height: 0;
    display: flex;
  }
  .channels {
    width: 420px;
    flex: none;
    display: flex;
    flex-direction: column;
    min-height: 0;
    border-right: 1px solid var(--border);
  }
  .channels header {
    flex: none;
    height: 62px;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 12px 0 20px;
  }
  h2 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .pick {
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    height: 36px;
    margin-left: -10px;
    padding: 0 10px;
    border-radius: 9px;
  }
  .pick:hover {
    background: var(--raised);
  }
  .pick .icon {
    font-size: 11px;
    color: var(--muted);
  }
  .count {
    font-size: 13px;
    color: var(--muted);
  }
  .spacer {
    flex: 1;
  }
  .tool {
    width: 36px;
    height: 36px;
    border-radius: 9px;
    font-size: 15px;
    color: var(--muted);
  }
  .tool:hover {
    background: var(--raised);
    color: var(--text);
  }
  .channels :global(.viewport) {
    padding: 0 10px;
  }
  .none {
    margin: 0 20px;
    font-size: 14px;
    line-height: 1.5;
    color: var(--muted);
  }
  .row {
    height: 64px;
    margin: 2px 0;
    display: flex;
    align-items: center;
    gap: 4px;
    padding-right: 6px;
    border-radius: 12px;
  }
  .row:hover {
    background: #141821;
  }
  .row.on {
    background: #1a1f2b;
  }
  .row.dragging {
    opacity: 0.4;
  }
  .main {
    flex: 1;
    min-width: 0;
    height: 100%;
    display: flex;
    align-items: center;
    gap: 12px;
    padding-left: 6px;
    text-align: left;
  }
  .main[draggable='true'] {
    cursor: grab;
  }
  .num {
    flex: none;
    width: 40px;
    text-align: right;
    font-size: 13px;
    color: var(--muted);
  }
  .row.on .num {
    color: var(--accent);
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .name {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .name b {
    min-width: 0;
    font-size: 15px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  /* Kept when empty, so rows without a programme line up with the others. */
  .text small {
    min-height: 1lh;
    font-size: 13px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  /* A fixed column for the feeds tag and the star, so every progress line ends at the same place. */
  .side {
    flex: none;
    width: 112px;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 2px;
  }
  .feeds {
    flex: none;
    display: flex;
    padding: 4px;
    border-radius: 7px;
  }
  .feeds:hover {
    background: var(--hover);
  }
  .star {
    flex: none;
    width: 36px;
    height: 36px;
    border-radius: 9px;
    font-family: var(--icons);
    font-size: 16px;
    color: #5e6678;
  }
  .star:hover {
    background: var(--hover);
    color: var(--text);
  }
  .star.fav {
    color: var(--fav);
  }
  .right {
    flex: 1;
    min-width: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 20px;
    padding: 20px 24px;
  }
  .preview {
    position: relative;
    flex: none;
    aspect-ratio: 16 / 9;
    border-radius: 14px;
    outline: 1px solid var(--border);
  }
  /* Cinema: as wide as the column, unless the window's height runs out first; the details below it. */
  /* Slim list: the logos alone, the name in the tooltip. */
  .slim .channels {
    width: 80px;
  }
  .slim .channels header {
    justify-content: center;
    padding: 0;
  }
  .slim .num,
  .slim .text,
  .slim .side,
  .slim .none {
    display: none;
  }
  .slim .row {
    padding: 0;
  }
  /* The scrollbar would take its width on the right only: the logos would sit off-centre. The wheel still scrolls. */
  .slim .channels :global(.viewport) {
    scrollbar-width: none;
  }
  .slim .main {
    justify-content: center;
    padding: 0;
  }
  .cinema .right {
    container-type: size;
    padding: 16px 20px;
  }
  .cinema .preview {
    width: min(100%, calc(100cqh * 16 / 9));
    align-self: center;
  }
  .preview.empty {
    display: grid;
    place-items: center;
    background: #05070b;
  }
  .live-badge {
    position: absolute;
    top: 12px;
    left: 12px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 24px;
    padding: 0 9px;
    border-radius: 7px;
    background: rgb(10 12 17 / 0.75);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.05em;
    color: #ff9a8d;
  }
  .live-badge span {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--live);
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
    background: rgb(10 12 17 / 0.75);
    font-size: 15px;
    font-weight: 600;
    text-align: center;
    color: var(--muted);
  }
  .empty .status {
    background: none;
  }
  .status.error {
    color: #ff9a8d;
  }
  .retry {
    display: block;
    margin: 10px auto 0;
    padding: 6px 16px;
    border-radius: 9px;
    background: var(--accent);
    color: var(--bg);
    font-size: 13px;
    font-weight: 600;
  }
  .corner {
    position: absolute;
    top: 10px;
    right: 10px;
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .rec {
    font-size: 12px;
    font-weight: 700;
    color: #ff9a8d;
  }
  .dial {
    position: absolute;
    top: 40px;
    right: 40px;
  }
  /* The controls show on hover, and stay while paused: the picture is not covered otherwise. */
  .controls {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 36px 10px 8px;
    border-radius: 0 0 14px 14px;
    background: linear-gradient(to top, rgb(5 7 11 / 0.9), rgb(5 7 11 / 0));
    opacity: 0;
    transition: opacity 0.2s;
  }
  .preview:hover .controls,
  .preview:focus-within .controls,
  .preview.paused .controls {
    opacity: 1;
  }
  .ctl {
    width: 38px;
    height: 38px;
    border-radius: 10px;
    font-size: 15px;
    color: var(--text);
  }
  .ctl:hover {
    background: rgb(255 255 255 / 0.12);
  }
</style>
