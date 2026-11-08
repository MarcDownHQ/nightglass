<script lang="ts">
  import { emit, listen } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { t } from './lib/i18n.svelte';

  /**
   * The PiP window: the video is under this page, the main window plays it. Controls show on hover;
   * what they do, the main window does (`pip-action`), and it says what plays (`pip-info`).
   */
  document.title = 'Nightglass PiP';
  let title = $state('');
  let paused = $state(false);
  let idle = $state(true);
  let timer: ReturnType<typeof setTimeout> | undefined;

  $effect(() => {
    const off = listen<{ title: string; paused: boolean }>('pip-info', ({ payload }) => {
      title = payload.title;
      paused = payload.paused;
    });
    void emit('pip-ready');
    return () => void off.then((f) => f());
  });

  const act = (action: 'pause' | 'expand') => void emit('pip-action', action);

  function poke() {
    idle = false;
    clearTimeout(timer);
    timer = setTimeout(() => (idle = true), 2000);
  }

  /** The picture moves the window; a double click on it goes back to the app. */
  function onmousedown(e: MouseEvent) {
    if (e.button !== 0 || (e.target as Element).closest('button')) return;
    if (e.detail === 2) act('expand');
    else void getCurrentWindow().startDragging();
  }
</script>

<div class="pip" class:idle onmousemove={poke} onmouseleave={() => (idle = true)} {onmousedown} role="presentation">
  <div class="top">
    <span class="title">{title}</span>
    <button class="icon" title={t('Back to the app')} aria-label={t('Back to the app')} onclick={() => act('expand')}>&#xE73F;</button>
    <button class="icon" title={t('Close the mini player')} aria-label={t('Close the mini player')} onclick={() => getCurrentWindow().close()}>&#xE8BB;</button>
  </div>
  <button class="icon play" aria-label={paused ? t('Play') : t('Pause')} onclick={() => act('pause')}>{@html paused ? '&#xE768;' : '&#xE769;'}</button>
</div>

<style>
  .pip {
    position: fixed;
    inset: 0;
    display: grid;
    place-items: center;
    transition: opacity 0.2s;
  }
  .pip.idle {
    opacity: 0;
  }
  .top {
    position: absolute;
    inset: 0 0 auto;
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 6px 6px 18px 12px;
    background: linear-gradient(to bottom, rgb(3 4 6 / 0.8), transparent);
  }
  .title {
    flex: 1;
    min-width: 0;
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .top button {
    width: 30px;
    height: 30px;
    border-radius: 8px;
    font-size: 12px;
    color: var(--soft);
  }
  .top button:hover {
    background: rgb(255 255 255 / 0.12);
    color: #fff;
  }
  .play {
    width: 48px;
    height: 48px;
    border-radius: 50%;
    background: rgb(10 12 17 / 0.7);
    font-size: 16px;
  }
  .play:hover {
    background: rgb(10 12 17 / 0.9);
  }
</style>
