<script lang="ts">
  /** Stream health, 0 to 5; turns magenta when the stream degrades. */
  let { level }: { level: number } = $props();
  const color = $derived(level <= 2 ? 'var(--magenta)' : 'var(--cyan)');
</script>

<span class="bars" role="img" aria-label="Signal {level} of 5">
  {#each [1, 2, 3, 4, 5] as i (i)}
    <i class:lit={i <= level} style:height="{4 + i * 3}px" style:--c={color}></i>
  {/each}
</span>

<style>
  .bars {
    display: inline-flex;
    align-items: flex-end;
    gap: 3px;
    height: 19px;
  }
  i {
    width: 4px;
    border-radius: 1px;
    background: rgb(233 236 242 / 0.15);
    transition: background 0.3s;
  }
  i.lit {
    background: var(--c);
  }
</style>
