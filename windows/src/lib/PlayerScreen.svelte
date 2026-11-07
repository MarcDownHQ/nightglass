<script lang="ts">
  import { t } from './i18n.svelte';
  import { dial, nav } from './nav.svelte';
  import NeonText from './NeonText.svelte';
  import { player } from './player.svelte';
  import ProgressLine from './ProgressLine.svelte';
  import SignalBars from './SignalBars.svelte';
  import StopButton from './StopButton.svelte';
  import { store } from './store.svelte';
  import { clock, displayName, hm, progress } from './theme';
  import TrackMenus from './TrackMenus.svelte';
  import { videoHole } from './video.svelte';
  import Volume from './Volume.svelte';
  import { library } from './library.svelte';
  import { friendly } from './errors';

  /** Fullscreen playback: nothing on the picture but a thin HUD that fades when the pointer rests. */
  let idle = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;

  const item = $derived(player.current?.item);
  const np = $derived(item ? store.nowNext(item.channel) : {});

  function poke() {
    idle = false;
    clearTimeout(timer);
    timer = setTimeout(() => (idle = true), 2500);
  }

  $effect(() => {
    poke();
    return () => clearTimeout(timer);
  });

  function zap(delta: number) {
    const list = store.lineup;
    const i = list.findIndex((c) => c.id === item?.id || c.variants?.some((v) => v.id === item?.id));
    if (i >= 0) void store.tune(list[(i + delta + list.length) % list.length]);
  }

  function onkeydown(e: KeyboardEvent) {
    poke();
    if (nav.searching || e.ctrlKey || e.altKey || e.metaKey) return;
    if (e.key === 'Escape' || e.key.toLowerCase() === 'f') void player.collapse();
    else if (e.key === ' ') player.togglePause();
    else if (e.key === 'ArrowLeft') player.seek(-10);
    else if (e.key === 'ArrowRight') player.seek(10);
    else if (e.key === 'ArrowUp') zap(-1);
    else if (e.key === 'ArrowDown') zap(1);
    else if (e.key.toLowerCase() === 'n' && player.current?.next) player.current.next.play();
    else if (e.key.toLowerCase() === 'r' && player.current?.live) void player.toggleRecording();
    else if (e.key.toLowerCase() === 'p') void player.toggleMini();
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

  function seekAt(e: MouseEvent) {
    const { left, width } = (e.currentTarget as HTMLElement).getBoundingClientRect();
    player.seekTo((player.duration * (e.clientX - left)) / width);
  }
</script>

<svelte:window {onkeydown} />

<div
  class="screen"
  class:idle
  class:mini={player.mini}
  use:videoHole={0}
  onmousemove={poke}
  onmousedown={poke}
  ondblclick={() => (player.mini ? player.expand() : player.collapse())}
  {onwheel}
  role="presentation"
>
  {#if player.mini}
    <!-- The small window has no title bar: the picture moves it. -->
    <div class="drag" data-tauri-drag-region></div>
  {/if}
  {#if player.loading || player.error}
    <div class="status" class:error={player.error}>
      {player.error ? friendly(player.error) : player.reconnecting ? t('Signal lost: reconnecting (attempt {n})…', { n: player.reconnecting }) : t('Loading {name}…', { name: displayName(player.current?.title ?? '') })}
      {#if player.error}<button class="retry" onclick={() => player.retry()}>{t('Retry')}</button>{/if}
    </div>
  {/if}
  {#if dial.digits}<div class="dial"><NeonText text={dial.digits} size={90} /></div>{/if}

  <div class="hud">
    <header>
      {#if player.current?.number}<NeonText text={String(player.current.number)} size={30} />{/if}
      <h1>{displayName(player.current?.title ?? '')}</h1>
      <span class="spacer"></span>
      {#if player.recording}<span class="rec mono" title={player.recording}>● REC</span>{/if}
      {#if !player.mini}
        <SignalBars level={player.signal} />
        <Volume />
        <TrackMenus />
        {#if player.current?.live}
          <button class="icon tool" class:on={player.recording} onclick={() => player.toggleRecording()} title={player.recording ? t('Stop recording (R)') : 'Record to Videos\\Nightglass (R)'}
            >&#xE7C8;</button
          >
        {/if}
      {/if}
      <button class="icon tool" onclick={() => player.toggleMini()} title={player.mini ? t('Back to the full window (P)') : t('Mini player, always on top (P)')}>
        {@html player.mini ? '&#xE740;' : '&#xE944;'}
      </button>
      <StopButton />
      {#if !player.mini}
        <button class="icon back" onclick={() => player.collapse()} title={t('Back to the list (Esc), playback continues')}>&#xE73F;</button>
      {/if}
    </header>

    <footer>
      {#if item}
        <div class="now">
          <span class="label mono">{t('Now')}</span>
          <h2>{np.now?.title ?? displayName(item.channel.name)}</h2>
          {#if np.now}<div class="bar"><ProgressLine value={progress(np.now, store.now)} height={3} /></div>{/if}
        </div>
        {#if player.behind}
          <button class="live-edge" onclick={() => player.goLive()} title={t('Back to live (L)')}>
            <span class="mono">−{clock(player.behind)}</span>
            <span class="label mono">{t('Go live')}</span>
          </button>
        {/if}
        {#if np.next}
          <div class="next">
            <span class="label mono">{t('Next')}</span>
            <span>{hm(np.next.start)} · {np.next.title}</span>
          </div>
        {/if}
      {:else}
        <div class="vod">
          <span class="label mono dim">{player.current?.subtitle}</span>
          <div class="transport mono">
            <button class="icon play" onclick={() => player.togglePause()} aria-label={player.paused ? t('Play') : t('Pause')}>
              {@html player.paused ? '&#xE768;' : '&#xE769;'}
            </button>
            <span>{clock(player.time)}</span>
            <button class="track" onclick={seekAt} aria-label={t('Seek')}>
              <span class="fill" style:width="{player.duration > 0 ? (player.time / player.duration) * 100 : 0}%"></span>
            </button>
            <span>{clock(player.duration)}</span>
          </div>
        </div>
        {#if player.current?.next}
          <button class="next-episode" onclick={() => player.current?.next?.play()} title={t('Next episode (N)')}>
            <span class="label mono">{t('Next episode')}</span>
            <span>{player.current.next.title} <span class="icon">&#xE893;</span></span>
          </button>
        {/if}
      {/if}
    </footer>
  </div>
</div>

<style>
  .screen {
    position: fixed;
    inset: 0;
  }
  .screen.idle {
    cursor: none;
  }
  .hud > :global(*) {
    pointer-events: auto;
  }
  .drag {
    position: absolute;
    inset: 0;
  }
  .mini header {
    padding: 10px 12px 24px;
    gap: 10px;
  }
  .mini h1 {
    font-size: 16px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .mini footer,
  .mini .dial {
    display: none;
  }
  .mini .status {
    font-size: 13px;
  }
  .tool {
    padding: 6px;
    font-size: 16px;
    color: var(--dim);
  }
  .tool:hover {
    color: var(--text);
  }
  .tool.on {
    color: var(--magenta);
  }
  .rec {
    font-size: 12px;
    color: var(--magenta);
    text-shadow: 0 0 6px var(--magenta);
    animation: blink 1s steps(2) infinite;
  }
  @keyframes blink {
    50% {
      opacity: 0.4;
    }
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
  .status {
    max-width: 60%;
    text-align: center;
  }
  .live-edge {
    flex: none;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 8px 14px;
    border-radius: 10px;
    border: 1px solid rgb(249 240 2 / 0.5);
    background: rgb(5 1 15 / 0.6);
    font-size: 18px;
  }
  .live-edge .label {
    color: var(--acid);
  }
  .hud {
    position: absolute;
    inset: 0;
    /* Only the bars take clicks: the picture under them is the screen (double-click, dragging the mini player). */
    pointer-events: none;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    transition: opacity 0.4s ease-out;
  }
  .idle .hud {
    opacity: 0;
  }
  header {
    display: flex;
    align-items: center;
    gap: 20px;
    padding: 34px 34px 40px;
    background: linear-gradient(to bottom, rgb(5 1 15 / 0.85), transparent);
  }
  h1 {
    margin: 0;
    font-size: 30px;
    font-weight: 700;
  }
  .spacer {
    flex: 1;
  }
  .back {
    font-size: 16px;
    padding: 8px;
  }
  footer {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 40px;
    padding: 50px 34px 34px;
    background: linear-gradient(to top, rgb(5 1 15 / 0.9), transparent);
  }
  .now,
  .next,
  .vod {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  .vod {
    flex: 1;
    gap: 10px;
  }
  .next {
    align-items: flex-end;
    font-size: 17px;
    font-weight: 500;
  }
  .label {
    font-size: 11px;
    font-weight: 500;
    color: var(--acid);
  }
  .next .label {
    color: var(--cyan);
  }
  .label.dim {
    color: var(--dim);
  }
  h2 {
    margin: 0;
    font-size: 30px;
    font-weight: 700;
  }
  .bar {
    width: 420px;
    max-width: 100%;
  }
  .transport {
    display: flex;
    align-items: center;
    gap: 14px;
    font-size: 12px;
  }
  .play {
    font-size: 20px;
  }
  .track {
    flex: 1;
    height: 20px;
    position: relative;
  }
  .track::before,
  .fill {
    content: '';
    position: absolute;
    left: 0;
    top: 50%;
    height: 3px;
    margin-top: -1.5px;
    border-radius: 2px;
  }
  .track::before {
    right: 0;
    background: rgb(243 238 255 / 0.15);
  }
  .fill {
    background: var(--cyan);
    box-shadow: 0 0 8px var(--cyan);
  }
  .next-episode {
    flex: none;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 6px;
    padding: 10px 16px;
    border-radius: 10px;
    border: 1px solid rgb(5 217 232 / 0.4);
    background: rgb(5 1 15 / 0.6);
    font-size: 16px;
    font-weight: 600;
  }
  .next-episode:hover {
    border-color: var(--cyan);
    box-shadow: 0 0 12px rgb(5 217 232 / 0.35);
  }
  .next-episode .label {
    color: var(--cyan);
  }
  .next-episode .icon {
    font-size: 12px;
  }
  .status {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    margin: 0;
    padding: 10px 18px;
    border-radius: 10px;
    background: rgb(5 1 15 / 0.75);
    font-size: 17px;
    color: var(--dim);
  }
  .status.error {
    color: var(--magenta);
  }
  .dial {
    position: absolute;
    top: 110px;
    right: 50px;
  }
</style>
