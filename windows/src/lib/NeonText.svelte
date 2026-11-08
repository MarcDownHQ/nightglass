<script lang="ts">
  /** A display title. With `lines`, long text wraps and is clamped. (`color` is kept for callers; titles are plain now.) */
  let {
    text,
    size = 40,
    weight = 700,
    lines,
  }: { text: string; size?: number; weight?: number; color?: string; lines?: number } = $props();
</script>

<span class="title" class:clamped={lines} style:font-size="{size}px" style:font-weight={Math.min(weight, 700)} style:-webkit-line-clamp={lines}>{text}</span>

<style>
  .title {
    color: var(--text);
    line-height: 1.15;
    letter-spacing: -0.015em;
    white-space: nowrap;
  }
  .clamped {
    display: -webkit-box;
    -webkit-box-orient: vertical;
    white-space: normal;
    overflow: hidden;
  }
</style>
