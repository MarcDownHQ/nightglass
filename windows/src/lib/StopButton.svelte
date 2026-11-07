<script lang="ts">
  import { t } from './i18n.svelte';
  import { player } from './player.svelte';
</script>

<!-- Stops playback entirely (Esc in Live, Ctrl+. anywhere). -->
<button class="stop" onclick={() => player.stop()} title={t('Stop playback (Ctrl+.)')}><i class="square"></i>{t('Stop')}</button>

<style>
  .stop {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    /* 28 px: the 10 px capitals and the 8 px square, both even, share its exact middle. */
    height: 28px;
    padding: 0 12px;
    border-radius: 999px;
    background: rgb(5 1 15 / 0.85);
    border: 1.5px solid rgb(255 42 109 / 0.6);
    color: #fff;
    font-size: 14px;
    font-weight: 600;
    transition: border-color 0.15s, box-shadow 0.15s;
  }
  .stop:hover {
    border-color: var(--magenta);
    box-shadow: 0 0 10px rgb(255 42 109 / 0.6);
  }
  /* Drawn, not the icon font's glyph: that one sits low in its box and missed the text's middle. */
  .square {
    flex: none;
    width: 8px;
    height: 8px;
    box-sizing: border-box;
    border: 1.5px solid currentColor;
    border-radius: 2px;
  }
</style>
