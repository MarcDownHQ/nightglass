<script lang="ts">
  /** Small outlined label (FHD, HD, M3U, Xtream). */
  let { text, color = 'var(--acid)' }: { text: string; color?: string } = $props();
</script>

<span class="tag mono" style:--c={color}>{text}</span>

<style>
  .tag {
    flex: none;
    font-size: 10px;
    font-weight: 600;
    padding: 2px 6px;
    border-radius: 4px;
    color: var(--c);
    border: 1px solid color-mix(in srgb, var(--c) 50%, transparent);
  }
</style>
