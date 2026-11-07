<script lang="ts">
  import { t } from './i18n.svelte';
  import { library } from './library.svelte';
  import { player } from './player.svelte';

  /** Mute button and slider; the wheel over the picture and +/−/M do the same. */
  const icon = $derived(library.muted || library.volume === 0 ? '' : library.volume < 34 ? '' : library.volume < 67 ? '' : '');
</script>

<div class="volume">
  <button class="icon" onclick={() => player.toggleMute()} title={library.muted ? t('Unmute (M)') : t('Mute (M)')}>{icon}</button>
  <input
    type="range"
    min="0"
    max="130"
    step="1"
    value={library.muted ? 0 : library.volume}
    aria-label={t('Volume')}
    title="Volume {library.volume}% (+/−)"
    oninput={(e) => player.setVolume(Number(e.currentTarget.value))}
  />
</div>

<style>
  .volume {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .icon {
    font-size: 16px;
    padding: 6px;
  }
  input {
    width: 110px;
    accent-color: var(--cyan);
  }
</style>
