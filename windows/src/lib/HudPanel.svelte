<script lang="ts">
  import type { Snippet } from 'svelte';

  /** A surface card with an optional caption row. */
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
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 14px;
    overflow: hidden;
  }
  header {
    display: flex;
    justify-content: space-between;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
    font-size: 14px;
    font-weight: 600;
  }
  .dim {
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
</style>
