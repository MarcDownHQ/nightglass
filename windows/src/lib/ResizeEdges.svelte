<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { onMount } from 'svelte';

  /**
   * The window has no frame and the webview covers all of it, so Windows never sees the pointer on its
   * edges: these strips hand the drag to the native resize. Gone while maximized or fullscreen.
   */
  const win = getCurrentWindow();
  const EDGES = ['North', 'South', 'East', 'West', 'NorthEast', 'NorthWest', 'SouthEast', 'SouthWest'] as const;
  let fixed = $state(false);

  async function check() {
    fixed = (await win.isMaximized()) || (await win.isFullscreen());
  }

  onMount(() => {
    void check();
    const off = win.onResized(check);
    return () => void off.then((f) => f());
  });
</script>

{#if !fixed}
  {#each EDGES as edge (edge)}
    <div class="edge {edge}" onmousedown={(e) => e.button === 0 && void win.startResizeDragging(edge)} aria-hidden="true"></div>
  {/each}
{/if}

<style>
  .edge {
    position: fixed;
    z-index: 1000;
  }
  .North,
  .South {
    left: 8px;
    right: 8px;
    height: 5px;
    cursor: ns-resize;
  }
  .North {
    top: 0;
  }
  .South {
    bottom: 0;
  }
  .East,
  .West {
    top: 8px;
    bottom: 8px;
    width: 5px;
    cursor: ew-resize;
  }
  .East {
    right: 0;
  }
  .West {
    left: 0;
  }
  .NorthEast,
  .NorthWest,
  .SouthEast,
  .SouthWest {
    width: 10px;
    height: 10px;
  }
  .NorthWest {
    top: 0;
    left: 0;
    cursor: nwse-resize;
  }
  .SouthEast {
    bottom: 0;
    right: 0;
    cursor: nwse-resize;
  }
  .NorthEast {
    top: 0;
    right: 0;
    cursor: nesw-resize;
  }
  .SouthWest {
    bottom: 0;
    left: 0;
    cursor: nesw-resize;
  }
</style>
