<script lang="ts">
  import ChannelLogo from './ChannelLogo.svelte';
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
  const rows = $derived(store.channels.filter((c) => store.programmes(c.channel).length));
  const day = $derived(new Date(start * 1000).toLocaleDateString([], { weekday: 'short', day: 'numeric', month: 'short' }));

  function open(item: LiveItem) {
    nav.focusChannel = item.id;
    nav.section = 'live';
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.defaultPrevented || player.expanded || nav.searching || typing(e) || e.ctrlKey || e.altKey || e.metaKey) return;
    if (e.key === 'ArrowLeft') shift -= 2;
    else if (e.key === 'ArrowRight') shift += 2;
    else return;
    e.preventDefault();
  }
</script>

<svelte:window {onkeydown} />

<div class="guide">
  <HudPanel caption="Guide" trailing={day} class="panel">
    {#if !rows.length}
      <p class="empty">
        {store.loading.size ? 'Loading the programme guide…' : 'No programme guide yet. Add an XMLTV URL to a source, or wait for it to load.'}
      </p>
    {:else}
      <div class="grid" bind:clientWidth={width}>
        <div class="ruler">
          <div class="paging" style:width="{LABEL}px">
            <button class="icon" title="Earlier (←)" onclick={() => (shift -= 2)}>&#xE76B;</button>
            <button class="now" disabled={shift === 0} onclick={() => (shift = 0)}>Now</button>
            <button class="icon" title="Later (→)" onclick={() => (shift += 2)}>&#xE76C;</button>
          </div>
          <div class="times">
            {#each { length: WINDOW / HALF } as _, i (i)}
              <span class="mono" style:left="{i * HALF * perSecond + 6}px">{hm(start + i * HALF)}</span>
            {/each}
          </div>
        </div>
        <VirtualList items={rows} rowHeight={ROW} key={(c) => c.id} row={guideRow} />
      </div>
    {/if}
  </HudPanel>
</div>

{#snippet guideRow(item: LiveItem)}
  {@const color = neon(item.channel.name)}
  {@const now = store.now}
  <div class="row" style:--c={color}>
    <div class="label" style:width="{LABEL}px">
      <ChannelLogo channel={item.channel} {color} size={34} />
      <b>{displayName(item.channel.name)}</b>
    </div>
    <div class="track">
      {#each store.programmes(item.channel).filter((p) => p.end > start && p.start < end) as p (p.start)}
        {@const x0 = Math.max(0, p.start - start) * perSecond}
        {@const x1 = (Math.min(p.end, end) - start) * perSecond}
        <button
          class="programme"
          class:live={p.start <= now && p.end > now}
          style:left="{x0 + 2}px"
          style:width="{Math.max(x1 - x0 - 4, 2)}px"
          title={p.desc ? `${p.title}\n\n${p.desc}` : p.title}
          onclick={() => open(item)}
        >
          <b>{p.title}</b>
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
    padding: 6px;
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
