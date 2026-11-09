<script lang="ts">
  import { MAC, t } from './i18n.svelte';
  import ChannelLogo from './ChannelLogo.svelte';
  import { dial, nav } from './nav.svelte';
  import NeonText from './NeonText.svelte';
  import { player } from './player.svelte';
  import SignalBars from './SignalBars.svelte';
  import StopButton from './StopButton.svelte';
  import { store } from './store.svelte';
  import { clock, displayName, hm, qualityTag } from './theme';
  import TrackMenus from './TrackMenus.svelte';
  import Menu from './Menu.svelte';
  import { videoHole } from './video.svelte';
  import VirtualList from './VirtualList.svelte';
  import Volume from './Volume.svelte';
  import { library } from './library.svelte';
  import { categoryKey, categoryLabel } from './lineup';
  import type { LiveItem } from './models';
  import { friendly } from './errors';

  /** Fullscreen playback: the picture, with bars that fade when the pointer rests. */
  let idle = $state(false);
  let drawer = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;

  const item = $derived(player.current?.item);
  const np = $derived(item ? store.nowNext(item.channel) : {});
  const quality = $derived(item ? qualityTag(item.channel.name) : null);
  const left = $derived(np.now ? Math.max(0, Math.round((np.now.end - store.now) / 60)) : 0);
  const isRow = (row: LiveItem) => row.id === item?.id || !!row.variants?.some((v) => v.id === item?.id);
  /** The playing channel's feeds, to switch between them here too. */
  const feeds = $derived(item ? (item.variants ?? store.lineup.find(isRow)?.variants ?? []) : []);
  /** What the arrows and the drawer zap through: the playing channel's category, else every channel. */
  const zapList = $derived.by(() => {
    const key = item?.channel.group ? categoryKey(item.channel.group) : null;
    const same = key ? store.lineup.filter((c) => c.channel.group && categoryKey(c.channel.group) === key) : [];
    return same.length > 1 ? same : store.lineup;
  });
  const zapTitle = $derived(zapList === store.lineup || !item?.channel.group ? t('All channels') : categoryLabel(item.channel.group));
  const firstList = $derived(library.favoriteLists[0]);
  const favorite = $derived(!!item && !!firstList?.items.includes(item.id));

  function poke() {
    idle = false;
    clearTimeout(timer);
    // The bars stay while the drawer is open.
    timer = setTimeout(() => (idle = !drawer), 2500);
  }

  $effect(() => {
    poke();
    return () => clearTimeout(timer);
  });

  function zap(delta: number) {
    const i = zapList.findIndex(isRow);
    if (i >= 0) void store.tune(zapList[(i + delta + zapList.length) % zapList.length]);
  }

  /** Back to the window: live TV comes back in the Live preview, whichever screen started it. */
  async function reduce() {
    const live = player.current?.live;
    await player.collapse();
    if (live && player.current) nav.section = 'live';
  }

  function onkeydown(e: KeyboardEvent) {
    poke();
    if (nav.searching || e.ctrlKey || e.altKey || e.metaKey) return;
    if (e.key === 'Escape' || e.key.toLowerCase() === 'f') void reduce();
    else if (e.key === ' ') player.togglePause();
    else if (e.key === 'ArrowLeft') player.seek(-10);
    else if (e.key === 'ArrowRight') player.seek(10);
    else if (e.key === 'ArrowUp') zap(-1);
    else if (e.key === 'ArrowDown') zap(1);
    else if (e.key.toLowerCase() === 'c' && item) drawer = !drawer;
    else if (e.key.toLowerCase() === 'n' && player.current?.next) player.current.next.play();
    else if (e.key.toLowerCase() === 'r' && player.current?.live) void player.toggleRecording();
    else if (e.key.toLowerCase() === 'p') void player.togglePip();
    else if (e.key.toLowerCase() === 'l' && player.behind) player.goLive();
    else if (/^[0-9]$/.test(e.key)) {
      dial.type(e.key, (n) => {
        const found = store.byNumber(n);
        if (found) void store.tune(found);
      });
    } else return;
    e.preventDefault();
  }

  function onwheel(e: WheelEvent) {
    poke();
    player.setVolume(library.volume + (e.deltaY < 0 ? 5 : -5));
  }

  /** Live: the buffer bar, from the oldest second kept to the last one arrived. Dragging previews, letting go seeks. */
  let scrub = $state<number | null>(null);
  const span = $derived(player.cacheTime - player.cacheStart);
  const pos = $derived(scrub ?? player.time);
  const pct = (s: number) => (span > 0 ? Math.min(100, Math.max(0, ((s - player.cacheStart) / span) * 100)) : 0);

  function bufferAt(e: PointerEvent) {
    const { left, width } = (e.currentTarget as HTMLElement).getBoundingClientRect();
    return player.cacheStart + span * Math.min(1, Math.max(0, (e.clientX - left) / width));
  }

  function scrubStart(e: PointerEvent) {
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    scrub = bufferAt(e);
  }

  function scrubEnd() {
    if (scrub !== null) player.seekTo(scrub);
    scrub = null;
  }

  /** A double click on the picture leaves fullscreen; not one on a control (two quick -10 s). */
  function ondblclick(e: MouseEvent) {
    if ((e.target as Element).closest('button, input, [role=slider], aside')) return;
    void reduce();
  }

  function seekAt(e: MouseEvent) {
    const { left, width } = (e.currentTarget as HTMLElement).getBoundingClientRect();
    player.seekTo((player.duration * (e.clientX - left)) / width);
  }
</script>

<svelte:window {onkeydown} />

<div
  class="screen"
  class:idle
  use:videoHole={0}
  onmousemove={poke}
  onmousedown={poke}
  {ondblclick}
  {onwheel}
  role="presentation"
>
  {#if player.loading || player.error}
    <div class="status" class:error={player.error}>
      {player.error ? friendly(player.error) : player.reconnecting ? t('Signal lost: reconnecting (attempt {n})…', { n: player.reconnecting }) : t('Loading {name}…', { name: displayName(player.current?.title ?? '') })}
      {#if player.error}<button class="retry" onclick={() => player.retry()}>{t('Retry')}</button>{/if}
    </div>
  {/if}
  {#if dial.digits}<div class="dial"><NeonText text={dial.digits} size={90} /></div>{/if}

  <div class="hud">
    <header class:mac={MAC}>
      {#if item}<ChannelLogo channel={item.channel} size={56} />{/if}
      <div class="title">
        <div class="meta">
          {#if player.current?.live}<span class="live-badge"><span></span>{t('LIVE')}</span>{/if}
          {#if player.recording}<span class="rec" title={player.recording}>● REC</span>{/if}
          {#if item}
            {#if item.channel.number}<span class="mono">{item.channel.number}</span><span>·</span>{/if}
            <b>{displayName(item.channel.name)}</b>
            {#if quality}<span>·</span><span>{quality}</span>{/if}
          {:else if !item && player.current?.subtitle}
            <span>{player.current.subtitle}</span>
          {/if}
        </div>
        <h1>{item ? (np.now?.title ?? displayName(item.channel.name)) : displayName(player.current?.title ?? '')}</h1>
        {#if np.now}
          <p class="when mono">{hm(np.now.start)} – {hm(np.now.end)} · {t('{n} min left', { n: left })}</p>
        {/if}
      </div>
      <div class="tools">
      {#if feeds.length > 1}
        <Menu
          label={t('Feeds')}
          items={feeds.map((v) => ({ id: v, name: `${v.channel.name} · ${v.source.name}`, selected: v.id === item?.id }))}
          onpick={(v) => store.chooseFeed(v, feeds)}
        />
      {/if}
      <TrackMenus />
      <button class="icon tool" onclick={() => player.togglePip()} title={t('Mini player above the other windows (P)')} aria-label={t('Mini player above the other windows (P)')}>&#xE8A7;</button>
      <StopButton />
      <button class="pill" onclick={reduce} title={t('Back to the list (Esc), playback continues')}>
        <span class="icon">&#xE73F;</span>{t('Reduce')}<kbd>{t('Esc')}</kbd>
      </button>
      </div>
    </header>

    {#if drawer && item}
      <aside class="drawer" aria-label={t('Change channel')}>
        <div class="drawer-head">
          <h2>{zapTitle}</h2>
          <span class="mono">{zapList.length}</span>
          <span class="spacer"></span>
          <button class="icon tool small" aria-label={t('Close')} onclick={() => (drawer = false)}>&#xE711;</button>
        </div>
        <VirtualList items={zapList} rowHeight={60} key={(c) => c.id} row={zapRow} />
        <p class="hint">{t('↑ ↓: previous or next channel · C: this list')}</p>
      </aside>
    {/if}

    <footer>
      {#if item}
        {#if player.current?.live && span > 1}
          <div class="buffer mono">
            <span title={t('Oldest moment kept in memory')}>−{clock(player.liveEdge - player.cacheStart)}</span>
            <div
              class="bar"
              class:dragging={scrub !== null}
              role="slider"
              tabindex="-1"
              aria-label={t('Buffer: drag to go back or forward')}
              title={t('Buffer: drag to go back or forward')}
              aria-valuemin={0}
              aria-valuemax={Math.round(span)}
              aria-valuenow={Math.round(pos - player.cacheStart)}
              onpointerdown={scrubStart}
              onpointermove={(e) => scrub !== null && (scrub = bufferAt(e))}
              onpointerup={scrubEnd}
              onpointercancel={() => (scrub = null)}
            >
              <span class="ahead" style:left="{pct(pos)}%" style:width="{100 - pct(pos)}%"></span>
              <span class="fill" style:width="{pct(pos)}%"></span>
              <span class="knob" style:left="{pct(pos)}%"></span>
            </div>
            <span class="edge">{(scrub ?? player.time) >= player.liveEdge - 3 || (scrub === null && !player.behind) ? t('Live') : '−' + clock(scrub !== null ? player.liveEdge - scrub : player.behind)}</span>
          </div>
        {/if}
        <div class="controls">
          <button class="icon tool" onclick={() => zap(-1)} title={t('Previous channel (↑)')} aria-label={t('Previous channel (↑)')}>&#xE70E;</button>
          <button class="icon tool" onclick={() => player.seek(-10)} title={t('Back 10 seconds (←)')} aria-label={t('Back 10 seconds (←)')}>&#xED3C;</button>
          <button class="icon play" onclick={() => player.togglePause()} aria-label={player.paused ? t('Play') : t('Pause')}>{@html player.paused ? '&#xE768;' : '&#xE769;'}</button>
          <button class="icon tool" onclick={() => zap(1)} title={t('Next channel (↓)')} aria-label={t('Next channel (↓)')}>&#xE70D;</button>
          <Volume />
          {#if player.behind}
            <button class="pill accent" onclick={() => player.goLive()} title={t('Back to live (L)')}>{t('Go live')}</button>
          {/if}
          <span class="spacer"></span>
          <SignalBars level={player.signal} />
          {#if firstList}
            <button class="pill" class:fav={favorite} aria-pressed={favorite} onclick={() => library.toggleFavorite(firstList.id, item.id)}>
              <span class="icon">{@html favorite ? '&#xE735;' : '&#xE734;'}</span>{favorite ? t('In favorites') : t('Favorite')}
            </button>
          {/if}
          {#if player.current?.live}
            <button class="pill" class:recording={player.recording} onclick={() => player.toggleRecording()} title={player.recording ? t('Stop recording (R)') : t('Record to Videos\\Nightglass (R)')}>
              <span class="dot"></span>{player.recording ? t('Stop recording') : t('Record')}
            </button>
          {/if}
          <button class="pill" class:on={drawer} aria-expanded={drawer} onclick={() => (drawer = !drawer)}><span class="icon">&#xE8FD;</span>{t('Channels')}</button>
        </div>
      {:else}
        <div class="controls">
          <button class="icon play" onclick={() => player.togglePause()} aria-label={player.paused ? t('Play') : t('Pause')}>{@html player.paused ? '&#xE768;' : '&#xE769;'}</button>
          <span class="mono time">{clock(player.time)}</span>
          <button class="seek" onclick={seekAt} aria-label={t('Seek')}>
            {#if player.duration > 0 && player.cacheTime > player.time}
              <span class="ahead" style:left="{(player.time / player.duration) * 100}%" style:width="{(Math.min(player.cacheTime, player.duration) - player.time) / player.duration * 100}%"></span>
            {/if}
            <span class="fill" style:width="{player.duration > 0 ? (player.time / player.duration) * 100 : 0}%"></span>
          </button>
          <span class="mono time">{clock(player.duration)}</span>
          <Volume />
          {#if player.current?.next}
            <button class="pill" onclick={() => player.current?.next?.play()} title={t('Next episode (N)')}>
              {t('Next episode')} · {player.current.next.title}<span class="icon">&#xE893;</span>
            </button>
          {/if}
        </div>
      {/if}
    </footer>
  </div>
</div>

{#snippet zapRow(row: LiveItem)}
  {@const now = store.nowNext(row.channel).now}
  <button class="zap" class:on={isRow(row)} onclick={() => store.tune(row)}>
    <span class="num mono">{row.channel.number ?? ''}</span>
    <ChannelLogo channel={row.channel} size={40} />
    <span class="zap-text">
      <b>{displayName(row.channel.name)}</b>
      <small>{now?.title ?? ''}</small>
    </span>
  </button>
{/snippet}

<style>
  .screen {
    position: fixed;
    inset: 0;
  }
  .screen.idle {
    cursor: none;
  }
  .hud {
    position: absolute;
    inset: 0;
    /* Only the bars take clicks: the picture under them is the screen (double-click). */
    pointer-events: none;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    transition: opacity 0.4s ease-out;
  }
  .hud > :global(*) {
    pointer-events: auto;
  }
  .idle .hud {
    opacity: 0;
  }
  header {
    display: flex;
    align-items: flex-start;
    gap: 14px;
    padding: 28px 36px 60px;
    background: linear-gradient(to bottom, rgb(3 4 6 / 0.85), transparent);
  }
  /* Below the traffic lights. */
  header.mac {
    padding-top: 44px;
  }
  /* Buttons of different heights: one middle line for all. */
  .tools {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .title {
    flex: 1;
    min-width: 0;
  }
  .meta {
    display: flex;
    align-items: center;
    gap: 9px;
    font-size: 14px;
    color: var(--soft);
    white-space: nowrap;
  }
  .meta b {
    color: #fff;
    font-weight: 600;
  }
  .when {
    margin: 6px 0 0;
    font-size: 14px;
    color: var(--soft);
  }
  h1 {
    margin: 8px 0 0;
    font-size: 28px;
    font-weight: 700;
    letter-spacing: -0.015em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .live-badge {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 24px;
    padding: 0 9px;
    border-radius: 7px;
    background: rgb(255 90 78 / 0.18);
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
  .rec {
    font-size: 12px;
    font-weight: 700;
    color: #ff9a8d;
    animation: blink 1s steps(2) infinite;
  }
  @keyframes blink {
    50% {
      opacity: 0.4;
    }
  }
  .tool {
    width: 42px;
    height: 42px;
    flex: none;
    border-radius: 11px;
    font-size: 16px;
    color: var(--text);
  }
  .tool:hover {
    background: rgb(255 255 255 / 0.12);
  }
  .tool.small {
    width: 34px;
    height: 34px;
    font-size: 12px;
  }
  .pill {
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 40px;
    padding: 0 14px;
    border-radius: 11px;
    background: rgb(20 24 33 / 0.8);
    color: var(--text);
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
  }
  .pill:hover {
    background: rgb(36 42 56 / 0.92);
  }
  .pill .icon {
    font-size: 14px;
  }
  .pill.on {
    background: rgb(143 168 255 / 0.22);
  }
  .pill.accent {
    background: var(--accent);
    color: var(--bg);
    font-weight: 600;
  }
  .pill.fav .icon {
    color: var(--fav);
  }
  kbd {
    font: inherit;
    font-size: 11px;
    color: var(--muted);
    padding: 2px 6px;
    border-radius: 6px;
    background: rgb(255 255 255 / 0.08);
  }
  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    border: 2px solid var(--live);
    box-sizing: border-box;
  }
  .recording .dot {
    background: var(--live);
  }
  .drawer {
    position: absolute;
    top: 120px;
    right: 36px;
    bottom: 170px;
    width: 380px;
    display: flex;
    flex-direction: column;
    border-radius: 18px;
    background: rgb(12 14 20 / 0.94);
    border: 1px solid rgb(255 255 255 / 0.08);
    overflow: hidden;
  }
  .drawer-head {
    flex: none;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 14px 12px 8px 18px;
  }
  .drawer-head h2 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .drawer-head .mono {
    font-size: 13px;
    color: var(--muted);
  }
  .drawer :global(.viewport) {
    padding: 0 8px;
  }
  .hint {
    flex: none;
    margin: 0;
    padding: 12px 18px;
    border-top: 1px solid var(--border);
    font-size: 12px;
    color: var(--muted);
  }
  .zap {
    width: 100%;
    height: 56px;
    margin: 2px 0;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 10px;
    border-radius: 12px;
    text-align: left;
  }
  .zap:hover {
    background: rgb(255 255 255 / 0.06);
  }
  .zap.on {
    background: rgb(143 168 255 / 0.16);
  }
  .num {
    flex: none;
    width: 34px;
    text-align: right;
    font-size: 13px;
    color: var(--muted);
  }
  .zap-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .zap-text b,
  .zap-text small {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .zap-text b {
    font-size: 14px;
    font-weight: 600;
  }
  .zap-text small {
    min-height: 1lh;
    font-size: 12px;
    color: var(--muted);
  }
  footer {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 60px 36px 30px;
    background: linear-gradient(to top, rgb(3 4 6 / 0.95), transparent);
  }
  .seek {
    position: relative;
    flex: 1;
    height: 18px;
  }
  .seek::before {
    content: '';
    position: absolute;
    left: 0;
    right: 0;
    top: 6px;
    height: 6px;
    border-radius: 3px;
    background: rgb(255 255 255 / 0.14);
  }
  .fill {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    border-radius: 3px;
    background: var(--accent);
  }
  .seek .fill,
  .seek .ahead,
  .bar .fill,
  .bar .ahead {
    top: 6px;
    bottom: 6px;
  }
  /* Arrived, not played yet. */
  .ahead {
    position: absolute;
    border-radius: 3px;
    background: rgb(255 255 255 / 0.32);
  }
  .buffer {
    display: flex;
    align-items: center;
    gap: 14px;
    font-size: 13px;
    color: var(--soft);
  }
  .bar {
    position: relative;
    flex: 1;
    height: 18px;
    cursor: pointer;
    touch-action: none;
  }
  .knob {
    position: absolute;
    top: 3px;
    width: 12px;
    height: 12px;
    margin-left: -6px;
    border-radius: 50%;
    background: #fff;
    opacity: 0;
    transition: opacity 0.15s;
  }
  .bar:hover .knob,
  .bar.dragging .knob {
    opacity: 1;
  }
  .buffer > span {
    width: 52px;
  }
  .edge {
    text-align: right;
  }
  .controls {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .play {
    width: 52px;
    height: 52px;
    flex: none;
    border-radius: 50%;
    background: var(--text);
    color: var(--bg);
    font-size: 18px;
  }
  .play:hover {
    background: #fff;
  }
  .time {
    font-size: 13px;
    color: var(--soft);
  }
  .spacer {
    flex: 1;
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
  .status {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    max-width: 60%;
    margin: 0;
    padding: 10px 18px;
    border-radius: 10px;
    background: rgb(10 12 17 / 0.75);
    font-size: 16px;
    text-align: center;
    color: var(--muted);
  }
  .status.error {
    color: #ff9a8d;
  }
  .dial {
    position: absolute;
    top: 120px;
    right: 50px;
  }
</style>
