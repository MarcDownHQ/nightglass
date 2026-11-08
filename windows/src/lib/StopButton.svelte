<script lang="ts">
  import { t } from './i18n.svelte';
  import { player } from './player.svelte';

  /** Smaller, for the corner of the Live preview. */
  let { compact = false }: { compact?: boolean } = $props();
</script>

<!-- Stops playback entirely (Esc in Live, Ctrl+. anywhere). Same shape as the player header's pills. -->
<button class="stop" class:compact onclick={() => player.stop()} title={t('Stop playback (Ctrl+.)')}><i class="square"></i>{t('Stop')}</button>

<style>
  .stop {
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
    transition: background 0.15s;
  }
  .stop:hover {
    background: rgb(255 90 78 / 0.22);
  }
  .stop.compact {
    height: 32px;
    padding: 0 11px;
    border-radius: 9px;
    gap: 7px;
    font-size: 12px;
  }
  /* Drawn, not the icon font's glyph: that one sits low in its box and missed the text's middle. */
  .square {
    flex: none;
    width: 10px;
    height: 10px;
    border-radius: 2px;
    background: var(--live);
  }
  .compact .square {
    width: 8px;
    height: 8px;
  }
</style>
