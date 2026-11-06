<script lang="ts">
  import { player } from './player.svelte';
</script>

<!-- Stops playback entirely (Esc in Live, Ctrl+. anywhere). -->
<button class="stop" onclick={() => player.stop()} title="Stop playback (Ctrl+.)"><i class="icon">&#xE71A;</i> Stop</button>

<style>
  .stop {
    display: inline-flex;
    align-items: center;
    gap: 7px;
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
  i {
    font-size: 9px;
  }
</style>
