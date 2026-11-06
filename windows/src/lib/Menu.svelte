<script lang="ts" generics="T">
  /** A label that opens a short list; used for audio and subtitle tracks. */
  let { label, items, onpick }: { label: string; items: { id: T; name: string; selected: boolean }[]; onpick: (id: T) => void } =
    $props();
  let open = $state(false);
  let root: HTMLElement;
</script>

<svelte:window onmousedown={(e) => open && !root.contains(e.target as Node) && (open = false)} />

<div class="menu" bind:this={root}>
  <button class="trigger" aria-expanded={open} onclick={() => (open = !open)}>{label}</button>
  {#if open}
    <ul role="menu">
      {#each items as item (item.id)}
        <li>
          <button role="menuitemradio" aria-checked={item.selected} class:on={item.selected} onclick={() => ((open = false), onpick(item.id))}>
            {item.name}
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .menu {
    position: relative;
  }
  .trigger {
    font-size: 15px;
    font-weight: 500;
    padding: 6px 4px;
  }
  .trigger:hover,
  .trigger[aria-expanded='true'] {
    color: var(--cyan);
  }
  ul {
    position: absolute;
    top: calc(100% + 6px);
    right: 0;
    z-index: 10;
    min-width: 200px;
    margin: 0;
    padding: 6px;
    list-style: none;
    background: var(--ink);
    border: 1px solid rgb(5 217 232 / 0.35);
    border-radius: 10px;
    box-shadow: 0 0 18px rgb(5 217 232 / 0.18);
  }
  li button {
    width: 100%;
    text-align: left;
    padding: 7px 10px;
    border-radius: 6px;
    font-size: 15px;
    color: var(--dim);
  }
  li button:hover {
    background: rgb(243 238 255 / 0.07);
    color: var(--text);
  }
  li button.on {
    color: var(--cyan);
  }
</style>
