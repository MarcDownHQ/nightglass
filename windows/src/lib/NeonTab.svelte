<script lang="ts">
  /** Filter chip: quiet when off, light when selected. (`color` is kept for callers.) */
  let {
    title,
    selected,
    onclick,
    oncontextmenu,
  }: { title: string; selected: boolean; color?: string; onclick: () => void; oncontextmenu?: (e: MouseEvent) => void } = $props();
</script>

<!-- type=button: inside a form (Add a source), a tab must not submit it. -->
<button type="button" class="tab" class:on={selected} aria-pressed={selected} {onclick} {oncontextmenu}>{title}</button>

<style>
  .tab {
    flex: none;
    height: 34px;
    box-sizing: border-box;
    padding: 0 14px;
    border-radius: 9px;
    background: var(--raised);
    color: var(--soft);
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
    transition: color 0.15s, background 0.15s;
  }
  .tab:hover {
    background: var(--hover);
    color: #fff;
  }
  .tab.on {
    background: var(--text);
    color: var(--bg);
  }
</style>
