<script lang="ts">
  import ChannelLogo from './ChannelLogo.svelte';
  import { openChannelMenu } from './channelMenu';
  import { contextMenu } from './menuState.svelte';
  import HudPanel from './HudPanel.svelte';
  import { library } from './library.svelte';
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

  const FAVORITES = '★ Favorites';
  const RECENT = '↺ Recent';
  const HIDDEN = 'Hidden';
  let category = $state('All');
  let listView: { reveal: (index: number) => void } | undefined = $state();
  let tabs: HTMLElement;

  const hidden = $derived.by(() => {
    if (store.channels === store.allChannels) return [];
    const shown = new Set(store.channels);
    return store.allChannels.filter((c) => !shown.has(c));
  });
  const recent = $derived.by(() => {
    const byId = new Map(store.channels.map((c) => [c.id, c]));
    return library.recents.flatMap((id) => byId.get(id) ?? []);
  });

  const categories = $derived.by(() => {
    const out = [...(store.favorites.size ? [FAVORITES] : []), ...(recent.length ? [RECENT] : []), 'All'];
    const seen = new Set<string>();
    for (const { channel } of store.channels) {
      if (channel.group && !seen.has(channel.group)) {
        seen.add(channel.group);
        out.push(channel.group);
      }
    }
    if (hidden.length) out.push(HIDDEN);
    return out;
  });

  const list = $derived(
    category === 'All'
      ? store.channels
      : category === FAVORITES
        ? store.channels.filter((c) => store.favorites.has(c.id))
        : category === RECENT
          ? recent
          : category === HIDDEN
            ? hidden
            : store.channels.filter((c) => c.channel.group === category),
  );

  function tabMenu(e: MouseEvent, c: string) {
    if (c === 'All' || c === FAVORITES || c === RECENT || c === HIDDEN) return;
    contextMenu.open(e, [{ label: `Hide “${c}”`, icon: '', action: () => library.toggleHidden(library.hiddenGroups, c) }]);
  }

  function onwheel(e: WheelEvent) {
    e.preventDefault();
    player.setVolume(library.volume + (e.deltaY < 0 ? 5 : -5));
  }

  const selected = $derived(player.current?.item?.id);

  $effect(() => {
    if (!categories.includes(category)) category = 'All';
  });

  // Keep the playing channel in view, whoever tuned it (keys, dial, search, zapping in fullscreen).
  $effect(() => {
    const i = list.findIndex((c) => c.id === selected);
    if (i >= 0) listView?.reveal(i);
  });

  $effect(() => {
    void category;
    tabs?.querySelector('[aria-pressed="true"]')?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  });

  // A channel picked in the guide or the search: show it in All and tune it.
  $effect(() => {
    const item = nav.focusChannel && store.channels.find((c) => c.id === nav.focusChannel);
    if (!item) return;
    nav.focusChannel = null;
    category = 'All';
    void store.tune(item);
  });

  function move(delta: number) {
    if (!list.length) return;
    const i = list.findIndex((c) => c.id === selected);
    void store.tune(list[i < 0 ? 0 : (i + delta + list.length) % list.length]);
  }

  function shiftCategory(delta: number) {
    const i = categories.indexOf(category);
    category = categories[(i + delta + categories.length) % categories.length];
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.defaultPrevented || player.expanded || nav.searching || typing(e) || e.ctrlKey || e.altKey || e.metaKey) return;
    if (e.key === 'ArrowDown') move(1);
    else if (e.key === 'ArrowUp') move(-1);
    else if (e.key === 'ArrowRight') shiftCategory(1);
    else if (e.key === 'ArrowLeft') shiftCategory(-1);
    else if (e.key === 'Enter') void player.expand();
    else if (e.key === 'Escape') player.stop();
    else if (e.key === ' ') player.togglePause();
    else if (e.key.toLowerCase() === 's' && player.current?.item) store.toggleFavorite(player.current.item);
    else if (/^[0-9]$/.test(e.key)) {
      dial.type(e.key, (n) => {
        const item = store.byNumber(n);
        if (item) {
          category = 'All';
          void store.tune(item);
        }
      });
    } else return;
    e.preventDefault();
  }
</script>

<svelte:window {onkeydown} />

<div class="live">
  <div class="left">
    <div class="tabs" bind:this={tabs}>
      {#each categories as c (c)}
        <NeonTab title={c} selected={c === category} onclick={() => (category = c)} oncontextmenu={(e) => tabMenu(e, c)} />
      {/each}
    </div>
    <HudPanel caption={category === 'All' ? 'All channels' : category} trailing={String(list.length)} class="channels">
      {#key category}
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
        <div class="corner"><StopButton /></div>
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
  {@const quality = qualityTag(ch.name)}
  <button class="row" class:on={item.id === selected} style:--c={c} onclick={() => store.tune(item)} ondblclick={() => player.expand()} oncontextmenu={(e) => openChannelMenu(e, item)}>
    <span class="num mono">{ch.number ?? ''}</span>
    <ChannelLogo channel={ch} color={c} />
    <span class="text">
      <b>{displayName(ch.name)}</b>
      <small>{np.now?.title ?? ch.group ?? ''}</small>
      <span class="progress"><ProgressLine value={np.now ? progress(np.now, store.now) : 0} color={c} /></span>
    </span>
    {#if store.favorites.has(item.id)}<span class="star">★</span>{/if}
    {#if quality}<Tag text={quality} />{/if}
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
  .tabs {
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
  .star {
    color: var(--acid);
  }
</style>
