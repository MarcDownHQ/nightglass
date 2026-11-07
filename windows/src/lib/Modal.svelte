<script lang="ts">
  import type { Snippet } from 'svelte';

  /** A window over the app: Esc or a click outside closes it. */
  let { title, close, width = 640, children }: { title: string; close: () => void; width?: number; children: Snippet } = $props();

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      close();
    }
  }
</script>

<svelte:window onkeydowncapture={onkeydown} />

<div class="scrim" role="presentation" onmousedown={(e) => e.target === e.currentTarget && close()}>
  <div class="dialog" role="dialog" aria-label={title} style:width="min({width}px, calc(100vw - 80px))">
    <header>
      <h2>{title}</h2>
      <button class="icon close" title="Close (Esc)" onclick={close}>&#xE8BB;</button>
    </header>
    <div class="body">{@render children()}</div>
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 105;
    display: grid;
    place-items: center;
    background: rgb(5 1 15 / 0.6);
  }
  .dialog {
    max-height: calc(100vh - 100px);
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 22px 24px;
    border-radius: 14px;
    background: var(--ink);
    border: 1px solid rgb(5 217 232 / 0.4);
    box-shadow: 0 0 24px rgb(5 217 232 / 0.2);
  }
  header {
    display: flex;
    align-items: center;
  }
  h2 {
    flex: 1;
    margin: 0;
    font-size: 24px;
  }
  .close {
    padding: 8px;
    font-size: 12px;
    color: var(--dim);
  }
  .body {
    min-height: 0;
    overflow-y: auto;
    scrollbar-width: thin;
    scrollbar-color: rgb(243 238 255 / 0.15) transparent;
  }
</style>
