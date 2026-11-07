export interface MenuEntry {
  label: string;
  /** Segoe Fluent Icons code point. */
  icon?: string;
  action: () => void;
}

/** The right-click menu: one at a time, drawn by `ContextMenu.svelte` at the pointer. */
class ContextMenuState {
  entries = $state.raw<MenuEntry[]>([]);
  x = $state(0);
  y = $state(0);

  open(e: MouseEvent, entries: MenuEntry[]) {
    e.preventDefault();
    e.stopPropagation();
    this.x = e.clientX;
    this.y = e.clientY;
    this.entries = entries;
  }

  close() {
    this.entries = [];
  }
}

export const contextMenu = new ContextMenuState();
