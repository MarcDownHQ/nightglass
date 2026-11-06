<script lang="ts">
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
    const list = store.channels;
    const i = list.findIndex((c) => c.id === item?.id);
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
    else if (/^[0-9]$/.test(e.key)) {
      dial.type(e.key, (n) => {
        const found = store.channels.find((c) => c.channel.number === n);
        if (found) void store.tune(found);
      });
    } else return;
    e.preventDefault();
  }

  function seekAt(e: MouseEvent) {
    const { left, width } = (e.currentTarget as HTMLElement).getBoundingClientRect();
    player.seekTo((player.duration * (e.clientX - left)) / width);
  }
</script>

<svelte:window {onkeydown} />

<div class="screen" class:idle use:videoHole={0} onmousemove={poke} onmousedown={poke} ondblclick={() => player.collapse()} role="presentation">
  {#if player.loading || player.error}
    <p class="status" class:error={player.error}>{player.error ?? `Loading ${displayName(player.current?.title ?? '')}…`}</p>
  {/if}
  {#if dial.digits}<div class="dial"><NeonText text={dial.digits} size={90} /></div>{/if}

  <div class="hud">
    <header>
      {#if player.current?.number}<NeonText text={String(player.current.number)} size={30} />{/if}
      <h1>{displayName(player.current?.title ?? '')}</h1>
      <span class="spacer"></span>
      <SignalBars level={player.signal} />
      <TrackMenus />
      <StopButton />
      <button class="icon back" onclick={() => player.collapse()} title="Back to the list (Esc), playback continues">&#xE73F;</button>
    </header>

    <footer>
      {#if item}
        <div class="now">
          <span class="label mono">Now</span>
          <h2>{np.now?.title ?? displayName(item.channel.name)}</h2>
          {#if np.now}<div class="bar"><ProgressLine value={progress(np.now, store.now)} height={3} /></div>{/if}
        </div>
        {#if np.next}
          <div class="next">
            <span class="label mono">Next</span>
            <span>{hm(np.next.start)} · {np.next.title}</span>
          </div>
        {/if}
      {:else}
        <div class="vod">
          <span class="label mono dim">{player.current?.subtitle}</span>
          <div class="transport mono">
            <button class="icon play" onclick={() => player.togglePause()} aria-label={player.paused ? 'Play' : 'Pause'}>
              {@html player.paused ? '&#xE768;' : '&#xE769;'}
            </button>
            <span>{clock(player.time)}</span>
            <button class="track" onclick={seekAt} aria-label="Seek">
              <span class="fill" style:width="{player.duration > 0 ? (player.time / player.duration) * 100 : 0}%"></span>
            </button>
            <span>{clock(player.duration)}</span>
          </div>
        </div>
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
  .hud {
    position: absolute;
    inset: 0;
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
