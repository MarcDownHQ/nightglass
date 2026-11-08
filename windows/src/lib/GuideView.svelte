<script lang="ts">
  import { t } from './i18n.svelte';
  import ChannelLogo from './ChannelLogo.svelte';
  import { openChannelMenu } from './channelMenu';
  import { library } from './library.svelte';
  import { contextMenu, type MenuEntry } from './menuState.svelte';
  import type { LiveItem, Programme } from './models';
  import { nav, typing } from './nav.svelte';
  import { player } from './player.svelte';
  import { store } from './store.svelte';
  import { displayName, hm } from './theme';
  import VirtualList from './VirtualList.svelte';

  /**
   * EPG grid. A 3-hour window fitted to the width and paged with ←/→, so the only scroll is vertical
   * and the rows stay virtual. A click picks a programme for the action bar; a double-click acts.
   */
  const ROW = 66;
  const LABEL = 228;
  const WINDOW = 3 * 3600;
  const HALF = 1800;

  let shift = $state(0);
  let width = $state(0);
  let favoritesOnly = $state(false);

  /** Half an hour before now, on the half hour, moved by the paging. */
  const start = $derived((Math.floor((store.now - HALF) / HALF) + shift) * HALF);
  const end = $derived(start + WINDOW);
  const perSecond = $derived(Math.max(0, width - LABEL) / WINDOW);
  const rows = $derived(store.lineup.filter((c) => store.programmes(c.channel).length && (!favoritesOnly || library.isFavorite(c.id))));
  const day = $derived(new Date(start * 1000).toLocaleDateString([], { weekday: 'long', day: 'numeric', month: 'long' }));

  let listView: { reveal: (index: number) => void } | undefined = $state();
  /** The channel asked for from a right-click elsewhere, marked for a moment. */
  let marked = $state<string | null>(null);
  /** The programme clicked, shown in the action bar. */
  let chosen = $state.raw<{ item: LiveItem; p: Programme } | null>(null);

  $effect(() => {
    const id = nav.guideChannel;
    if (!id || !listView) return;
    nav.guideChannel = null;
    favoritesOnly = false;
    const i = rows.findIndex((c) => c.id === id || c.variants?.some((v) => v.id === id));
    if (i < 0) return player.notify(t('No programme guide for this channel'));
    shift = 0;
    listView.reveal(i);
    marked = id;
    setTimeout(() => marked === id && (marked = null), 2500);
  });

  function open(item: LiveItem) {
    nav.focusChannel = item.id;
    nav.section = 'live';
  }

  function remind(item: LiveItem, p: Programme) {
    library.toggleReminder({ channel: item.id, channelName: displayName(item.channel.name), title: p.title, start: p.start, end: p.end });
    if (library.hasReminder(item.id, p.start)) player.notify(t('Reminder set for {time}', { time: hm(p.start) }));
  }

  /** The default action: replay a past programme when the server keeps it, a reminder for a future one, else the channel. */
  function act(item: LiveItem, p: Programme) {
    if (p.end <= store.now && store.canReplay(item, p)) void store.playReplay(item, p);
    else if (p.start > store.now) remind(item, p);
    else open(item);
  }

  function programmeMenu(e: MouseEvent, item: LiveItem, p: Programme) {
    const now = store.now;
    const entries: MenuEntry[] = [{ heading: `${hm(p.start)}–${hm(p.end)} · ${p.title}` }];
    if (p.start <= now && p.end > now) entries.push({ label: t('Watch live'), icon: '', action: () => open(item) });
    if (store.canReplay(item, p)) {
      entries.push({ label: p.end > now ? t('Watch from the start') : t('Replay'), icon: '', action: () => void store.playReplay(item, p) });
    }
    if (p.start > now) {
      entries.push({
        label: library.hasReminder(item.id, p.start) ? t('Cancel the reminder') : t('Remind me'),
        icon: '',
        action: () => remind(item, p),
      });
    }
    contextMenu.open(e, entries);
  }

  /**
   * The keyboard cursor: a row and a moment. The programme under it is the one the arrows left it on;
   * going up or down keeps the moment, so the cursor stays in the same column.
   */
  let cursorRow = $state(0);
  let cursorTime = $state(0);
  let keyboard = $state(false);

  const cursorProgramme = $derived.by(() => {
    const row = rows[cursorRow];
    if (!row || !keyboard) return undefined;
    const list = store.programmes(row.channel);
    return list.find((p) => p.start <= cursorTime && p.end > cursorTime) ?? list.find((p) => p.start > cursorTime);
  });

  /** What the action bar shows: the keyboard cursor while it moves, else the programme clicked. */
  const shown = $derived(keyboard && cursorProgramme ? { item: rows[cursorRow], p: cursorProgramme } : chosen);

  /** Pages the window so the moment shows. */
  function follow(t: number) {
    while (t < start + HALF / 2 && shift > -200) shift -= 1;
    while (t >= end - HALF / 2 && shift < 200) shift += 1;
  }

  function moveCursor(rowDelta: number, programmeDelta: number) {
    if (!rows.length) return;
    if (!keyboard) {
      keyboard = true;
      cursorTime = store.now;
      // The first key shows the cursor where the eye already is: the first row, now.
      follow(cursorTime);
      listView?.reveal(cursorRow);
      return;
    }
    cursorRow = Math.max(0, Math.min(rows.length - 1, cursorRow + rowDelta));
    const list = store.programmes(rows[cursorRow].channel);
    if (programmeDelta) {
      const i = list.findIndex((p) => p.start <= cursorTime && p.end > cursorTime);
      const next = list[Math.max(0, Math.min(list.length - 1, (i < 0 ? 0 : i) + programmeDelta))];
      if (next) cursorTime = next.start + 1;
    }
    follow(cursorTime);
    listView?.reveal(cursorRow);
  }

  function choose(item: LiveItem, p: Programme) {
    keyboard = false;
    chosen = { item, p };
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.defaultPrevented || player.expanded || nav.searching || typing(e) || e.ctrlKey || e.altKey || e.metaKey) return;
    if (e.key === 'ArrowLeft') moveCursor(0, -1);
    else if (e.key === 'ArrowRight') moveCursor(0, 1);
    else if (e.key === 'ArrowUp') moveCursor(-1, 0);
    else if (e.key === 'ArrowDown') moveCursor(1, 0);
    else if (e.key === 'PageUp') shift -= 2;
    else if (e.key === 'PageDown') shift += 2;
    else if (e.key === 'Home') {
      shift = 0;
      cursorTime = store.now;
    } else if (e.key === 'Enter' && shown) act(shown.item, shown.p);
    else if (e.key === 'Escape' && (keyboard || chosen)) {
      keyboard = false;
      chosen = null;
    } else return;
    e.preventDefault();
  }

  const minutes = (s: number) => Math.max(0, Math.round(s / 60));
  function duration(p: Programme): string {
    const m = minutes(p.end - p.start);
    return m >= 60 ? `${Math.floor(m / 60)} h ${String(m % 60).padStart(2, '0')}` : `${m} min`;
  }
</script>

<svelte:window {onkeydown} />

<div class="guide">
  <div class="toolbar">
    <button class="chip icon-only" title={t('Earlier (Page ↑)')} aria-label={t('Earlier (Page ↑)')} onclick={() => (shift -= 2)}><span class="icon">&#xE76B;</span></button>
    <button class="chip" class:on={shift === 0} onclick={() => (shift = 0)}>{t('Now')}</button>
    <button class="chip icon-only" title={t('Later (Page ↓)')} aria-label={t('Later (Page ↓)')} onclick={() => (shift += 2)}><span class="icon">&#xE76C;</span></button>
    <span class="day">{day}</span>
    <span class="spacer"></span>
    <button class="chip" class:on={favoritesOnly} aria-pressed={favoritesOnly} onclick={() => (favoritesOnly = !favoritesOnly)}>
      <span class="icon star">&#xE735;</span>{t('Favorites only')}
    </button>
  </div>

  {#if !rows.length}
    <p class="empty">
      {favoritesOnly
        ? t('None of your favorite channels has a guide.')
        : store.loading.size
          ? t('Loading the programme guide…')
          : t('No programme guide yet. Add an XMLTV URL to a source, or wait for it to load.')}
    </p>
  {:else}
    <div class="grid" bind:clientWidth={width}>
      <div class="ruler">
        <div style:width="{LABEL}px"></div>
        <div class="times">
          {#each { length: WINDOW / HALF } as _, i (i)}
            <span class="mono" style:left="{i * HALF * perSecond + 2}px">{hm(start + i * HALF)}</span>
          {/each}
        </div>
      </div>
      <div class="rows">
        <VirtualList items={rows} rowHeight={ROW} key={(c) => c.id} row={guideRow} bind:this={listView} />
        {#if store.now > start && store.now < end}
          <span class="line" style:left="{LABEL + (store.now - start) * perSecond - 1}px"></span>
          <span class="now-tag mono" style:left="{LABEL + (store.now - start) * perSecond}px">{hm(store.now)}</span>
        {/if}
      </div>
    </div>
  {/if}

  {#if shown}
    {@const { item, p } = shown}
    {@const now = store.now}
    {@const past = p.end <= now}
    {@const live = p.start <= now && !past}
    {@const reminded = library.hasReminder(item.id, p.start)}
    <section class="details" aria-label={t('Selected programme')}>
      <ChannelLogo channel={item.channel} size={56} />
      <div class="info">
        <div class="meta">
          {#if item.channel.number}<span class="mono">{item.channel.number}</span> · {/if}{displayName(item.channel.name)} · <span class="mono">{hm(p.start)} – {hm(p.end)}</span> · {duration(p)}
        </div>
        <h2>{p.title}</h2>
        <div class="state">
          {past ? (store.canReplay(item, p) ? t('Finished · available in replay') : t('Finished')) : live ? t('On now · {n} min left', { n: minutes(p.end - now) }) : t('Starts in {n} min', { n: minutes(p.start - now) })}
        </div>
      </div>
      <div class="actions">
        {#if live}
          <button class="chip accent big" onclick={() => open(item)}><span class="icon">&#xE768;</span>{t('Watch')}</button>
          {#if store.canReplay(item, p)}<button class="chip big" onclick={() => store.playReplay(item, p)}><span class="icon">&#xE72C;</span>{t('Watch from the start')}</button>{/if}
        {:else if past}
          {#if store.canReplay(item, p)}<button class="chip accent big" onclick={() => store.playReplay(item, p)}><span class="icon">&#xE72C;</span>{t('Replay')}</button>{/if}
        {:else}
          <button class="chip accent big" aria-pressed={reminded} onclick={() => remind(item, p)}><span class="icon">&#xEA8F;</span>{reminded ? t('Reminder set') : t('Remind me')}</button>
        {/if}
        <button class="chip big icon-only" aria-label={t('Close')} title={t('Close')} onclick={() => ((chosen = null), (keyboard = false))}><span class="icon">&#xE711;</span></button>
      </div>
      {#if p.desc}<p class="desc">{p.desc}</p>{/if}
    </section>
  {/if}
</div>

{#snippet guideRow(item: LiveItem)}
  {@const now = store.now}
  <div class="row" class:marked={marked === item.id}>
    <div class="label" style:width="{LABEL}px" oncontextmenu={(e) => openChannelMenu(e, item)} role="presentation">
      <span class="num mono">{item.channel.number ?? ''}</span>
      <ChannelLogo channel={item.channel} size={40} />
      <b>{displayName(item.channel.name)}</b>
    </div>
    <div class="track">
      {#each store.programmes(item.channel).filter((p) => p.end > start && p.start < end) as p (p.start)}
        {@const x0 = Math.max(0, p.start - start) * perSecond}
        {@const x1 = (Math.min(p.end, end) - start) * perSecond}
        {@const replay = p.end <= now && store.canReplay(item, p)}
        {@const reminded = library.hasReminder(item.id, p.start)}
        <button
          class="programme"
          class:past={p.end <= now}
          class:live={p.start <= now && p.end > now}
          class:on={shown?.item === item && shown.p === p}
          style:left="{x0 + 2}px"
          style:width="{Math.max(x1 - x0 - 4, 2)}px"
          title={p.desc ? `${p.title}\n\n${p.desc}` : p.title}
          onclick={() => choose(item, p)}
          ondblclick={() => act(item, p)}
          oncontextmenu={(e) => programmeMenu(e, item, p)}
        >
          <b>{#if replay}<span class="icon mark">&#xE72C;</span>{:else if reminded}<span class="icon mark">&#xEA8F;</span>{/if}{p.title}</b>
          <small class="mono">{hm(p.start)} – {hm(p.end)}</small>
        </button>
      {/each}
    </div>
  </div>
{/snippet}

<style>
  .guide {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .toolbar {
    flex: none;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 14px 24px;
    border-bottom: 1px solid var(--border);
  }
  .day {
    margin-left: 10px;
    font-size: 14px;
    font-weight: 600;
    color: var(--soft);
    display: inline-block;
  }
  .day::first-letter {
    text-transform: uppercase;
  }
  .spacer {
    flex: 1;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    height: 34px;
    padding: 0 13px;
    border-radius: 9px;
    background: var(--raised);
    color: var(--soft);
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
  }
  .chip:hover {
    background: var(--hover);
    color: #fff;
  }
  .chip.on {
    background: var(--text);
    color: var(--bg);
  }
  .chip.icon-only {
    width: 34px;
    padding: 0;
  }
  .chip .icon {
    font-size: 12px;
  }
  .chip .star {
    color: var(--fav);
  }
  .chip.accent {
    background: var(--accent);
    color: var(--bg);
    font-weight: 600;
  }
  .chip.accent:hover {
    background: var(--accent-hover);
  }
  .chip.big {
    height: 42px;
    padding: 0 18px;
    font-size: 14px;
  }
  .chip.big.icon-only {
    width: 42px;
    padding: 0;
  }
  .empty {
    margin: auto;
    padding: 40px;
    font-size: 15px;
    color: var(--muted);
  }
  .grid {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .ruler {
    flex: none;
    height: 40px;
    display: flex;
    border-bottom: 1px solid var(--border);
  }
  .times {
    flex: 1;
    position: relative;
    overflow: hidden;
  }
  .times span {
    position: absolute;
    top: 12px;
    font-size: 12px;
    color: var(--muted);
  }
  .rows {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .line {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 2px;
    background: var(--accent);
    pointer-events: none;
  }
  .now-tag {
    position: absolute;
    top: 0;
    transform: translateX(-50%);
    padding: 2px 6px;
    border-radius: 0 0 6px 6px;
    background: var(--accent);
    color: var(--bg);
    font-size: 11px;
    font-weight: 700;
    pointer-events: none;
  }
  .row {
    height: 100%;
    display: flex;
    border-bottom: 1px solid rgb(255 255 255 / 0.04);
    box-sizing: border-box;
  }
  .row.marked {
    background: rgb(143 168 255 / 0.1);
  }
  .label {
    flex: none;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 16px 0 10px;
    box-sizing: border-box;
    min-width: 0;
  }
  .num {
    flex: none;
    width: 34px;
    text-align: right;
    font-size: 13px;
    color: var(--muted);
  }
  .label b {
    font-size: 14px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .track {
    flex: 1;
    position: relative;
    overflow: hidden;
  }
  .programme {
    position: absolute;
    top: 6px;
    bottom: 6px;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 3px;
    padding: 0 12px;
    border-radius: 9px;
    text-align: left;
    background: var(--raised);
    overflow: hidden;
  }
  .programme:hover {
    background: var(--hover);
  }
  .programme.past {
    background: #0f1218;
    color: #8a92a3;
  }
  .programme.live {
    background: #1a2131;
  }
  .programme.on {
    background: var(--accent);
    color: var(--bg);
  }
  .programme b,
  .programme small {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .programme b {
    font-size: 13px;
    font-weight: 600;
  }
  .programme small {
    font-size: 11px;
    color: var(--muted);
  }
  .programme.on small {
    color: #1b2340;
  }
  .programme .mark {
    margin-right: 5px;
    font-size: 10px;
  }
  .details {
    flex: none;
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    column-gap: 22px;
    padding: 18px 28px;
    border-top: 1px solid var(--border);
    background: #0e1117;
  }
  .info {
    min-width: 0;
  }
  .meta {
    font-size: 13px;
    color: var(--muted);
  }
  h2 {
    margin: 6px 0 0;
    font-size: 21px;
    font-weight: 700;
    letter-spacing: -0.01em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .state {
    margin-top: 6px;
    font-size: 13px;
    color: var(--soft);
  }
  .actions {
    display: flex;
    gap: 10px;
  }
  .desc {
    grid-column: 2 / 4;
    margin: 10px 0 0;
    font-size: 13px;
    line-height: 1.5;
    color: var(--muted);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
</style>
