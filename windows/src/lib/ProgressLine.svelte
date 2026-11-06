<script lang="ts">
  let { value, color = 'var(--magenta)', height = 2 }: { value: number; color?: string; height?: number } = $props();
</script>

<div class="line" style:height="{height}px">
  <div class="fill" style:width="{value * 100}%" style:--c={color}></div>
</div>

<style>
  .line {
    background: rgb(243 238 255 / 0.1);
  }
  .fill {
    height: 100%;
    background: var(--c);
    box-shadow: 0 0 6px var(--c);
  }
</style>
