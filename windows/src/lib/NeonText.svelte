<script lang="ts">
  /** White text lit by a coloured neon halo. With `lines`, long text wraps and is clamped. */
  let {
    text,
    size = 40,
    weight = 800,
    color = 'var(--magenta)',
    lines,
  }: { text: string; size?: number; weight?: number; color?: string; lines?: number } = $props();
</script>

<span
  class="neon"
  class:clamped={lines}
  style:font-size="{size}px"
  style:font-weight={weight}
  style:--glow={color}
  style:--r="{size * 0.35}px"
  style:-webkit-line-clamp={lines}>{text}</span
>

<style>
  .neon {
    color: #fff;
    line-height: 1.1;
    white-space: nowrap;
    text-shadow:
      0 0 calc(var(--r) * 0.5) color-mix(in srgb, var(--glow) 90%, transparent),
      0 0 calc(var(--r) * 1.6) color-mix(in srgb, var(--glow) 55%, transparent);
  }
  /* Room for the halo inside the clipped box, given back by the negative margin. */
  .clamped {
    display: -webkit-box;
    -webkit-box-orient: vertical;
    white-space: normal;
    overflow: hidden;
    padding: 0.2em 0.4em;
    margin: -0.2em -0.4em;
  }
</style>
