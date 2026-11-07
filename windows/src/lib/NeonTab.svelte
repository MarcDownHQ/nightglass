<script lang="ts">
  /** Filter capsule: a neon outline that lights up when selected. */
  let {
    title,
    selected,
    color = 'var(--magenta)',
    onclick,
    oncontextmenu,
  }: { title: string; selected: boolean; color?: string; onclick: () => void; oncontextmenu?: (e: MouseEvent) => void } = $props();
</script>

<!-- type=button: inside a form (Add a source), a tab must not submit it. -->
<button type="button" class="tab" class:on={selected} style:--c={color} aria-pressed={selected} {onclick} {oncontextmenu}>{title}</button>

<style>
  /* A set height: a ★ or ↺ drawn from a fallback font would otherwise make its tab taller. */
  .tab {
    flex: none;
    height: 34px;
    box-sizing: border-box;
    padding: 0 14px;
    line-height: 1;
    border-radius: 999px;
    border: 1px solid rgb(243 238 255 / 0.15);
    color: var(--dim);
    font-size: 16px;
    font-weight: 600;
    white-space: nowrap;
    transition: color 0.15s, border-color 0.15s, background 0.3s, box-shadow 0.3s;
  }
  .tab:hover {
    color: var(--text);
    border-color: rgb(243 238 255 / 0.35);
  }
  /* The thicker ring is drawn inside, so selecting a tab doesn't move the others. */
  .tab.on {
    color: #fff;
    border-color: var(--c);
    background: color-mix(in srgb, var(--c) 18%, transparent);
    box-shadow:
      inset 0 0 0 0.5px var(--c),
      0 0 8px color-mix(in srgb, var(--c) 60%, transparent);
  }
</style>
