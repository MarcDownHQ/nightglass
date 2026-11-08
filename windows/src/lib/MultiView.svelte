<script lang="ts">
  import { t } from './i18n.svelte';
  import ChannelLogo from './ChannelLogo.svelte';
  import HudPanel from './HudPanel.svelte';
  import { friendly } from './errors';
  import { fold } from './lineup';
  import type { LiveItem } from './models';
  import { multiview } from './tilesState.svelte';
  import { store } from './store.svelte';
  import { displayName, neon } from './theme';
  import VirtualList from './VirtualList.svelte';

  /** Two or four channels side by side: pick a tile, then a channel on the left. */
  let filter = $state('');

  const list = $derived.by(() => {
    const q = fold(filter.trim());
    return q ? store.lineup.filter((c) => fold(c.channel.name).includes(q)) : store.lineup;
  });

  let grid: HTMLElement | undefined = $state();

  /** Each tile's video area is a window onto its mpv instance: measured as a whole, whenever the grid changes. */
  function measure() {
    const videos = grid ? [...grid.querySelectorAll<HTMLElement>('.video')] : [];
    for (let i = 0; i < 4; i++) {
      const r = videos[i]?.getBoundingClientRect();
      multiview.place(i, r ? { x: r.left, y: r.top, w: r.width, h: r.height, radius: 10 } : null);
    }
  }

  $effect(() => {
    if (!grid) return;
    void multiview.layout;
    const observer = new ResizeObserver(measure);
    observer.observe(grid);
    addEventListener('resize', measure);
    measure();
    return () => {
      observer.disconnect();
      removeEventListener('resize', measure);
    };
  });

  // A tile that gets a channel changes its caption, so its video area: measure again.
  $effect(() => {
    void multiview.tiles.map((t) => t.item);
    queueMicrotask(measure);
  });

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && !e.defaultPrevented) {
      e.preventDefault();
      void multiview.close();
    } else if (e.altKey && /^[1-4]$/.test(e.key) && Number(e.key) <= multiview.layout) {
      e.preventDefault();
      void multiview.focus(Number(e.key) - 1);
    }
  }
</script>

<svelte:window {onkeydown} />

<div class="multiview">
  <div class="side">
    <div class="tools">
      <button class:on={multiview.layout === 2} onclick={() => multiview.setLayout(2)} title={t('Two channels')}><span class="icon">&#xE745;</span> 2</button>
      <button class:on={multiview.layout === 4} onclick={() => multiview.setLayout(4)} title={t('Four channels')}><span class="icon">&#xF0E2;</span> 4</button>
      <span class="spacer"></span>
      <button class="close" onclick={() => multiview.close()} title={t('Close multi-view (Esc)')}>{t('Close')}</button>
    </div>
    <input class="filter" placeholder={t('Filter channels')} bind:value={filter} spellcheck="false" />
    <HudPanel caption={t('Into tile {n}', { n: multiview.active + 1 })} trailing={String(list.length)} class="channels">
      <VirtualList items={list} rowHeight={52} key={(c) => c.id} row={channelRow} />
    </HudPanel>
  </div>

  <div class="grid" class:two={multiview.layout === 2} bind:this={grid}>
    {#each multiview.tiles.slice(0, multiview.layout) as tile, i (i)}
      <div
        class="tile"
        class:active={multiview.active === i}
        class:empty={!tile.item}
        role="button"
        tabindex="0"
        onclick={() => multiview.focus(i)}
        onkeydown={(e) => e.key === 'Enter' && multiview.focus(i)}
      >
        <div class="video">
          {#if !tile.item}
            <p class="hint">{t('Tile {n}', { n: i + 1 })}<br /><small>{t('Select it, then pick a channel')}</small></p>
          {:else if tile.loading || tile.error}
            <p class="hint" class:error={tile.error}>{tile.error ? friendly(tile.error) : t('Loading…')}</p>
          {/if}
        </div>
        {#if tile.item}
          <div class="caption">
            <span class="icon sound" title={multiview.active === i ? t('Heard') : t('Muted (click the tile to hear it)')}>
              {multiview.active === i ? '' : ''}
            </span>
            <b>{displayName(tile.item.channel.name)}</b>
            <button
              class="icon"
              title={t('Empty this tile')}
              onclick={(e) => {
                e.stopPropagation();
                void multiview.clear(i);
              }}>&#xE8BB;</button
            >
          </div>
        {/if}
      </div>
    {/each}
  </div>
</div>

{#snippet channelRow(item: LiveItem)}
  {@const c = neon(item.channel.name)}
  <button class="row" style:--c={c} onclick={() => multiview.assign(multiview.active, item)}>
    <ChannelLogo channel={item.channel} color={c} size={32} />
    <b>{displayName(item.channel.name)}</b>
  </button>
{/snippet}

<style>
  .multiview {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 320px 1fr;
    gap: 24px;
    padding: 4px 28px 20px;
  }
  .side {
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .side :global(.channels) {
    flex: 1;
  }
  /* Its top on the tiles' top. */
  .tools {
    display: flex;
    gap: 8px;
  }
  .tools button {
    height: 33px;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 12px;
    border-radius: 999px;
    border: 1px solid rgb(233 236 242 / 0.15);
    font-size: 15px;
    font-weight: 600;
    color: var(--dim);
  }
  .tools .icon {
    font-size: 13px;
  }
  .tools button.on {
    color: #fff;
    border-color: var(--magenta);
  }
  .tools .close {
    border-color: rgb(233 236 242 / 0.1);
    color: var(--cyan);
  }
  .spacer {
    flex: 1;
  }
  .filter {
    padding: 8px 12px;
    border-radius: 8px;
    border: 1px solid rgb(233 236 242 / 0.15);
    background: var(--night);
    color: var(--text);
    font: inherit;
    font-size: 15px;
  }
  .filter:focus {
    outline: none;
    border-color: var(--cyan);
  }
  .row {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 14px;
    text-align: left;
  }
  .row:hover {
    background: rgb(233 236 242 / 0.05);
  }
  .row b {
    font-size: 16px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .grid {
    min-height: 0;
    display: grid;
    grid-template-columns: 1fr 1fr;
    grid-template-rows: 1fr 1fr;
    gap: 14px;
  }
  .grid.two {
    grid-template-rows: 1fr;
    align-content: center;
  }
  .grid.two .tile {
    aspect-ratio: 16 / 10;
  }
  .tile {
    position: relative;
    min-height: 0;
    display: flex;
    flex-direction: column;
    border-radius: 12px;
    outline: 1.5px solid rgb(233 236 242 / 0.15);
    cursor: pointer;
  }
  .tile.active {
    outline-color: var(--cyan);
  }
  .video {
    flex: 1;
    min-height: 0;
    display: grid;
    place-items: center;
    border-radius: 12px 12px 0 0;
  }
  .tile.empty .video {
    border-radius: 12px;
    background: #000;
  }
  .hint {
    margin: 0;
    padding: 8px 14px;
    border-radius: 8px;
    background: rgb(10 12 17 / 0.7);
    text-align: center;
    font-size: 16px;
    color: var(--dim);
  }
  .hint.error {
    color: var(--magenta);
  }
  .caption {
    flex: none;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    border-radius: 0 0 12px 12px;
    background: var(--ink);
  }
  .caption b {
    flex: 1;
    font-size: 15px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .caption button {
    padding: 4px;
    font-size: 10px;
    color: var(--dim);
  }
  .sound {
    font-size: 13px;
    color: var(--cyan);
  }
</style>
