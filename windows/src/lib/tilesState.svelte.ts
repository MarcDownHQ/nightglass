import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { LiveItem } from './models';
import { player } from './player.svelte';
import { store } from './store.svelte';
import type { HoleRect } from './video.svelte';

export interface Tile {
  item: LiveItem | null;
  loading: boolean;
  error: string | null;
}

type TileEvent = { tile: number } & ({ event: 'playback-restart' } | { event: 'end-file'; reason: string; error: string | null } | { event: string });

const empty = (): Tile => ({ item: null, loading: false, error: null });

/**
 * Two or four channels at once, each in its own mpv instance (Rust `tiles`). The main player stops
 * meanwhile; only the active tile is heard.
 */
class MultiView {
  open = $state(false);
  layout = $state<2 | 4>(4);
  tiles = $state<Tile[]>([empty(), empty(), empty(), empty()]);
  active = $state(0);
  /** Where each tile's video goes, in CSS pixels: holes in the page's backdrop. */
  rects = $state<(HoleRect | null)[]>([null, null, null, null]);

  constructor() {
    // The tiles outlive the page: after a reload, start from none.
    void invoke('tile_close', { index: null });
    void invoke('tiles_mode', { on: false });
    void listen<TileEvent>('tile', ({ payload }) => {
      const t = this.tiles[payload.tile];
      if (!t) return;
      if (payload.event === 'playback-restart') t.loading = false;
      else if (payload.event === 'end-file' && 'reason' in payload && payload.reason === 'error') {
        t.loading = false;
        t.error = payload.error ?? 'Stream ended.';
      }
    });
  }

  async show(item?: LiveItem) {
    if (!this.open) {
      player.stop();
      await invoke('tiles_mode', { on: true });
      this.open = true;
    }
    if (item) this.add(item);
  }

  async close() {
    this.open = false;
    this.tiles = [empty(), empty(), empty(), empty()];
    this.rects = [null, null, null, null];
    this.pending.clear();
    await invoke('tile_close', { index: null });
    await invoke('tiles_mode', { on: false });
  }

  /** Into the first empty tile, or the active one when all are taken. */
  add(item: LiveItem) {
    const free = this.tiles.slice(0, this.layout).findIndex((t) => !t.item);
    void this.assign(free >= 0 ? free : this.active, item);
  }

  /** Tiles with a channel whose window is not playing yet: waiting for the tile to be laid out. */
  private pending = new Set<number>();

  async assign(index: number, item: LiveItem) {
    this.tiles[index] = { item, loading: true, error: null };
    if (!this.tiles.some((t, i) => i !== index && t.item)) this.active = index;
    this.pending.add(index);
    await this.start(index);
  }

  private async start(index: number) {
    const rect = this.physical(index);
    const item = this.tiles[index].item;
    if (!rect || !item || !this.pending.delete(index)) return;
    try {
      const p = await store.playable(item);
      await invoke('tile_play', { index, url: p.url, headers: p.headers ?? {}, rect, muted: index !== this.active });
    } catch (e) {
      this.tiles[index] = { item, loading: false, error: String(e) };
    }
  }

  async clear(index: number) {
    this.pending.delete(index);
    this.tiles[index] = empty();
    await invoke('tile_close', { index });
  }

  /** The tile that is heard. */
  async focus(index: number) {
    this.active = index;
    await Promise.all(this.tiles.map((t, i) => (t.item ? invoke('tile_mute', { index: i, muted: i !== index }) : null)));
  }

  async setLayout(layout: 2 | 4) {
    this.layout = layout;
    // Tiles that no longer fit stop.
    for (let i = layout; i < 4; i++) if (this.tiles[i].item) await this.clear(i);
    if (this.active >= layout) await this.focus(0);
  }

  /** A tile's element moved or resized: its window follows. */
  place(index: number, rect: HoleRect | null) {
    this.rects[index] = rect;
    const physical = this.physical(index);
    if (!physical || !this.tiles[index].item) return;
    if (this.pending.has(index)) void this.start(index);
    else void invoke('tile_rect', { index, rect: physical });
  }

  private physical(index: number): [number, number, number, number] | null {
    const r = this.rects[index];
    if (!r) return null;
    const k = devicePixelRatio;
    return [Math.round(r.x * k), Math.round(r.y * k), Math.round(r.w * k), Math.round(r.h * k)];
  }
}

export const multiview = new MultiView();
