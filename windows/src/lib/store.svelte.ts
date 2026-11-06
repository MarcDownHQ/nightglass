import { invoke } from '@tauri-apps/api/core';
import { SvelteSet } from 'svelte/reactivity';
import { epgKey, type Catalog, type Channel, type LiveItem, type Programme, type Source } from './models';
import { player, type Playable } from './player.svelte';

/** Settings without secrets live in the webview's storage; sources (passwords) go through DPAPI. */
const prefs = {
  get(key: string): string | null {
    try {
      return localStorage.getItem(key);
    } catch {
      return null;
    }
  },
  set(key: string, value: string | null) {
    try {
      if (value === null) localStorage.removeItem(key);
      else localStorage.setItem(key, value);
    } catch {
      // Private storage unavailable: the setting lasts for this session only.
    }
  },
};

class Store {
  sources = $state.raw<Source[]>([]);
  catalogs = $state.raw<Record<string, Catalog>>({});
  loading = new SvelteSet<string>();
  errors = $state<Record<string, string>>({});
  /** Programmes per normalised EPG id, sorted by start. */
  guide = $state.raw<Record<string, Programme[]>>({});
  favorites = new SvelteSet<string>(JSON.parse(prefs.get('favorites') ?? '[]'));
  /** Source the screens show; null means every source. */
  activeSource = $state<string | null>(prefs.get('activeSource'));
  /** Unix seconds, ticking every 30 s for progress lines and now/next. */
  now = $state(Date.now() / 1000);

  visibleSources = $derived(
    this.sources.some((s) => s.id === this.activeSource) ? this.sources.filter((s) => s.id === this.activeSource) : this.sources,
  );

  /** Playlists without tvg-chno get their position as number, so every channel can be dialled. */
  channels = $derived.by(() => {
    let n = 0;
    return this.visibleSources.flatMap((source) =>
      (this.catalogs[source.id]?.channels ?? []).map((channel): LiveItem => {
        n++;
        return { id: source.id + channel.id, source, channel: channel.number === null ? { ...channel, number: n } : channel };
      }),
    );
  });

  async init() {
    setInterval(() => (this.now = Date.now() / 1000), 30_000);
    this.sources = await invoke<Source[]>('sources_load');
    for (const s of this.sources) void this.refresh(s);
  }

  async refresh(source: Source) {
    this.loading.add(source.id);
    delete this.errors[source.id];
    try {
      this.setCatalog(source, await invoke<Catalog>('catalog', { source }));
    } catch (e) {
      this.errors[source.id] = String(e);
    } finally {
      this.loading.delete(source.id);
    }
  }

  private setCatalog(source: Source, catalog: Catalog) {
    this.catalogs = { ...this.catalogs, [source.id]: catalog };
    if (catalog.epgUrl) void this.loadGuide(catalog.epgUrl, catalog.channels);
  }

  async loadGuide(url: string, channels: Channel[]) {
    const keys = [...new Set(channels.flatMap((c) => (c.epgId ? [epgKey(c.epgId)] : [])))];
    if (!keys.length) return;
    try {
      const incoming = await invoke<Record<string, Programme[]>>('guide', { url, keys });
      // Merge into the current guide, not a snapshot: several guides load at once.
      const merged = { ...this.guide };
      for (const [key, list] of Object.entries(incoming)) {
        merged[key] = merged[key] ? withoutRepeats([...merged[key], ...list]) : list;
      }
      this.guide = merged;
    } catch (e) {
      console.warn(`guide ${url}: ${e}`);
    }
  }

  add(source: Source, catalog: Catalog) {
    this.sources = [...this.sources, source];
    void this.save();
    this.setCatalog(source, catalog);
  }

  remove(source: Source) {
    this.sources = this.sources.filter((s) => s.id !== source.id);
    const { [source.id]: _, ...rest } = this.catalogs;
    this.catalogs = rest;
    if (this.activeSource === source.id) this.setActive(null);
    void this.save();
  }

  private async save() {
    try {
      await invoke('sources_save', { sources: this.sources });
    } catch (e) {
      console.error(`sources_save: ${e}`);
    }
  }

  setActive(id: string | null) {
    this.activeSource = id;
    prefs.set('activeSource', id);
  }

  toggleFavorite(item: LiveItem) {
    if (!this.favorites.delete(item.id)) this.favorites.add(item.id);
    prefs.set('favorites', JSON.stringify([...this.favorites]));
  }

  programmes(channel: Channel): Programme[] {
    return (channel.epgId && this.guide[epgKey(channel.epgId)]) || [];
  }

  nowNext(channel: Channel, at = this.now): { now?: Programme; next?: Programme } {
    const list = this.programmes(channel);
    const i = list.findIndex((p) => p.end > at);
    if (i < 0) return {};
    return list[i].start <= at ? { now: list[i], next: list[i + 1] } : { next: list[i] };
  }

  async playable(item: LiveItem): Promise<Playable> {
    const { source, channel } = item;
    const url = 'stalker' in source.kind ? await invoke<string>('stream_url', { source, channel }) : channel.stream;
    return {
      id: item.id,
      title: channel.name,
      subtitle: this.nowNext(channel).now?.title ?? channel.group ?? '',
      url,
      headers: channel.headers,
      live: true,
      number: channel.number ?? undefined,
      item,
    };
  }

  async tune(item: LiveItem) {
    try {
      player.play(await this.playable(item));
    } catch (e) {
      player.error = String(e);
    }
  }
}

/** The same guide loaded twice must not list a programme twice. */
function withoutRepeats(list: Programme[]): Programme[] {
  list.sort((a, b) => a.start - b.start);
  return list.filter((p, i) => i === 0 || p.start !== list[i - 1].start || p.title !== list[i - 1].title);
}

export const store = new Store();
