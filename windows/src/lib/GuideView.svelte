<script lang="ts">
  import { t } from './i18n.svelte';
  import ChannelLogo from './ChannelLogo.svelte';
  import { openChannelMenu } from './channelMenu';
  import { library } from './library.svelte';
  import { contextMenu, type MenuEntry } from './menuState.svelte';
  import type { Programme } from './models';
  import HudPanel from './HudPanel.svelte';
  import type { LiveItem } from './models';
  import { nav, typing } from './nav.svelte';
  import { player } from './player.svelte';
  import { store } from './store.svelte';
  import { displayName, hm, neon } from './theme';
  import VirtualList from './VirtualList.svelte';

  /**
   * EPG grid. A 3-hour window fitted to the width and paged with ←/→, so the only scroll is vertical
   * and the rows stay virtual.
   */
  const ROW = 58;
  const LABEL = 220;
  const WINDOW = 3 * 3600;
  const HALF = 1800;

  let shift = $state(0);
  let width = $state(0);

  /** Half an hour before now, on the half hour, moved by the paging. */
  const start = $derived((Math.floor((store.now - HALF) / HALF) + shift) * HALF);
  const end = $derived(start + WINDOW);
  const perSecond = $derived(Math.max(0, width - LABEL) / WINDOW);
  const rows = $derived(store.lineup.filter((c) => store.programmes(c.channel).length));
  const day = $derived(new Date(start * 1000).toLocaleDateString([], { weekday: 'short', day: 'numeric', month: 'short' }));

  let listView: { reveal: (index: number) => void } | undefined = $state();
  /** The channel asked for from a right-click elsewhere, marked for a moment. */
  let marked = $state<string | null>(null);

  $effect(() => {
    const id = nav.guideChannel;
    if (!id || !listView) return;
    nav.guideChannel = null;
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

  /** A click: replay a past programme when the server keeps it, a reminder for a future one, else the channel. */
  function pick(item: LiveItem, p: Programme) {
    if (p.end <= store.now && store.canReplay(item, p)) void store.playReplay(item, p);
    else if (p.start > store.now) remind(item, p);
    else open(item);
  }

  function programmeMenu(e: MouseEvent, item: LiveItem, p: Programme) {
    const now = store.now;
    const entries: MenuEntry[] = [{ heading: `${hm(p.start)}–${hm(p.end)} · ${p.title}` }];
    if (p.start <= now && p.end > now) entries.push({ label: t('Watch live'), icon: '\uE768', action: () => open(item) });
    if (store.canReplay(item, p)) {
      entries.push({ label: p.end > now ? t('Watch from the start') : t('Replay'), icon: '\uE72C', action: () => void store.playReplay(item, p) });
    }
    if (p.start > now) {
      entries.push({
        label: library.hasReminder(item.id, p.start) ? t('Cancel the reminder') : t('Remind me'),
        icon: '\uEA8F',
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
    } else if (e.key === 'Enter' && cursorProgramme) pick(rows[cursorRow], cursorProgramme);
    else if (e.key === 'Escape' && keyboard) keyboard = false;
    else return;
    e.preventDefault();
  }
</script>

<svelte:window {onkeydown} />

<div class="guide">
  <HudPanel caption={t('Guide')} trailing={day} class="panel">
    {#if !rows.length}
      <p class="empty">
        {store.loading.size ? t('Loading the programme guide…') : t('No programme guide yet. Add an XMLTV URL to a source, or wait for it to load.')}
      </p>
    {:else}
      <div class="grid" bind:clientWidth={width}>
        <div class="ruler">
          <div class="paging" style:width="{LABEL}px">
            <button class="icon" title={t('Earlier (Page ↑)')} onclick={() => (shift -= 2)}>&#xE76B;</button>
            <button class="now" disabled={shift === 0} onclick={() => (shift = 0)}>{t('Now')}</button>
            <button class="icon" title={t('Later (Page ↓)')} onclick={() => (shift += 2)}>&#xE76C;</button>
          </div>
          <div class="times">
            {#each { length: WINDOW / HALF } as _, i (i)}
              <span class="mono" style:left="{i * HALF * perSecond + 13}px">{hm(start + i * HALF)}</span>
            {/each}
          </div>
        </div>
        <VirtualList items={rows} rowHeight={ROW} key={(c) => c.id} row={guideRow} bind:this={listView} />
      </div>
    {/if}
  </HudPanel>
</div>

{#snippet guideRow(item: LiveItem)}
  {@const color = neon(item.channel.name)}
  {@const now = store.now}
  <div class="row" class:marked={marked === item.id} style:--c={color}>
    <div class="label" style:width="{LABEL}px" oncontextmenu={(e) => openChannelMenu(e, item)} role="presentation">
      <ChannelLogo channel={item.channel} {color} size={34} />
      <b>{displayName(item.channel.name)}</b>
    </div>
    <div class="track">
      {#each store.programmes(item.channel).filter((p) => p.end > start && p.start < end) as p, i (p.start + ':' + i)}
        {@const x0 = Math.max(0, p.start - start) * perSecond}
        {@const x1 = (Math.min(p.end, end) - start) * perSecond}
        {@const replay = p.end <= now && store.canReplay(item, p)}
        {@const reminded = library.hasReminder(item.id, p.start)}
        <button
          class="programme"
          class:live={p.start <= now && p.end > now}
          class:replay
          class:reminded
          class:cursor={cursorProgramme === p && rows[cursorRow] === item}
          style:left="{x0 + 2}px"
          style:width="{Math.max(x1 - x0 - 4, 2)}px"
          title={(p.desc ? `${p.title}\n\n${p.desc}` : p.title) + (replay ? `\n\n${t('Click to replay')}` : p.start > now ? `\n\n${t('Click for a reminder')}` : '')}
          onclick={() => pick(item, p)}
          oncontextmenu={(e) => programmeMenu(e, item, p)}
        >
          <b>{#if replay}<span class="icon mark">&#xE72C;</span>{:else if reminded}<span class="icon mark">&#xEA8F;</span>{/if}{p.title}</b>
          <small class="mono">{hm(p.start)}–{hm(p.end)}</small>
        </button>
      {/each}
      {#if now > start && now < end}<span class="line" style:left="{(now - start) * perSecond}px"></span>{/if}
    </div>
  </div>
{/snippet}

<style>
  .guide {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding: 4px 28px 20px;
  }
  .guide :global(.panel) {
    flex: 1;
  }
  .empty {
    margin: auto;
    padding: 40px;
    font-size: 16px;
    color: var(--dim);
  }
  .grid {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .ruler {
    flex: none;
    height: 34px;
    display: flex;
    border-bottom: 1px solid rgb(5 217 232 / 0.1);
  }
  .paging {
    flex: none;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 14px;
    box-sizing: border-box;
  }
  .paging button {
    height: 28px;
    padding: 0 6px;
    color: var(--cyan);
  }
  .paging .icon {
    font-size: 12px;
  }
  .paging .now {
    font-size: 14px;
    font-weight: 600;
  }
  .paging .now:disabled {
    color: var(--dim);
    cursor: default;
  }
  .times {
    flex: 1;
    position: relative;
    overflow: hidden;
  }
  .times span {
    position: absolute;
    top: 10px;
    font-size: 11px;
    color: var(--dim);
  }
  .row {
    height: 100%;
    display: flex;
    border-bottom: 1px solid rgb(5 217 232 / 0.08);
    box-sizing: border-box;
  }
  .programme.cursor {
    outline: 2px solid var(--acid);
    outline-offset: -1px;
    box-shadow: 0 0 12px rgb(249 240 2 / 0.4);
    z-index: 1;
  }
  .programme .mark {
    margin-right: 5px;
    font-size: 11px;
    color: var(--cyan);
  }
  .programme.reminded .mark {
    color: var(--acid);
  }
  .row.marked {
    background: color-mix(in srgb, var(--c) 14%, transparent);
    box-shadow: inset 3px 0 0 var(--c);
  }
  .label {
    flex: none;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 14px;
    box-sizing: border-box;
    min-width: 0;
  }
  .label b {
    font-size: 16px;
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
    gap: 2px;
    padding: 0 10px;
    border-radius: 6px;
    text-align: left;
    background: rgb(5 1 15 / 0.5);
    border: 1px solid rgb(243 238 255 / 0.15);
    overflow: hidden;
  }
  .programme:hover {
    border-color: rgb(243 238 255 / 0.35);
  }
  .programme.live {
    background: color-mix(in srgb, var(--c) 20%, transparent);
    border: 1.5px solid var(--c);
  }
  .programme b,
  .programme small {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .programme b {
    font-size: 14px;
    font-weight: 600;
  }
  .programme small {
    font-size: 9px;
    color: var(--dim);
  }
  .line {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 2px;
    background: var(--cyan);
    box-shadow: 0 0 6px var(--cyan);
    pointer-events: none;
  }
</style>
