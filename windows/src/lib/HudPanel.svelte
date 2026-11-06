<script lang="ts">
  import type { Snippet } from 'svelte';

  /** Dark glass panel with a faint cyan edge. */
  let { caption, trailing, children, class: cls = '' }: { caption?: string; trailing?: string; children: Snippet; class?: string } =
    $props();
</script>

<section class="hud {cls}">
  {#if caption}
    <header>
      <span>{caption}</span>
      {#if trailing}<span class="dim">{trailing}</span>{/if}
    </header>
  {/if}
  {@render children()}
</section>

<style>
  .hud {
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--panel);
    border: 1px solid rgb(5 217 232 / 0.2);
    border-radius: 14px;
    overflow: hidden;
  }
  header {
    display: flex;
    justify-content: space-between;
    padding: 12px 16px;
    border-bottom: 1px solid rgb(5 217 232 / 0.14);
    font-size: 15px;
    font-weight: 600;
  }
  .dim {
    color: var(--dim);
  }
</style>
