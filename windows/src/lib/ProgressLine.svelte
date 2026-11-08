<script lang="ts">
  let { value, color = 'var(--magenta)', height = 2 }: { value: number; color?: string; height?: number } = $props();
</script>

<div class="line" style:height="{height}px">
  <div class="fill" style:width="{value * 100}%" style:--c={color}></div>
</div>

<style>
  .line {
    background: rgb(233 236 242 / 0.1);
  }
  .fill {
    height: 100%;
    background: var(--c);
  }
</style>
