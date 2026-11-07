import { invoke } from '@tauri-apps/api/core';
import { SvelteSet } from 'svelte/reactivity';
import { episodeKey, library } from './library.svelte';
import {
  epgKey,
  type Catalog,
  type Channel,
  type Episode,
  type LiveItem,
  type MovieItem,
  type Programme,
  type SeriesItem,
  type Source,
} from './models';
import { player, type Playable } from './player.svelte';
import { prefs } from './prefs';

class Store {
  sources = $state.raw<Source[]>([]);
  catalogs = $state.raw<Record<string, Catalog>>({});
  loading = new SvelteSet<string>();
  errors = $state<Record<string, string>>({});
  /** Programmes per normalised EPG id, sorted by start. */
  guide = $state.raw<Record<string, Programme[]>>({});
  favorites = new SvelteSet<string>(prefs.json('favorites', []));
  /** Source the screens show; null means every source. */
  activeSource = $state<string | null>(prefs.get('activeSource'));
  /** Unix seconds, ticking every 30 s for progress lines and now/next. */
  now = $state(Date.now() / 1000);

  visibleSources = $derived(
    this.sources.some((s) => s.id === this.activeSource) ? this.sources.filter((s) => s.id === this.activeSource) : this.sources,
  );

  /**
   * Every channel of the sources shown, hidden ones included. Playlists without tvg-chno get their
   * position in their own source as number, so every channel can be dialled and keeps its number
   * whichever sources are shown.
   */
  allChannels = $derived(
    this.visibleSources.flatMap((source) =>
      (this.catalogs[source.id]?.channels ?? []).map(
        (channel, i): LiveItem => ({
          id: source.id + channel.id,
          source,
          channel: channel.number === null ? { ...channel, number: i + 1 } : channel,
        }),
      ),
    ),
  );

  /** The channels the screens list, zap and search: without the hidden channels and categories. */
  channels = $derived(
    !library.hiddenChannels.size && !library.hiddenGroups.size
      ? this.allChannels
      : this.allChannels.filter(
          (c) => !library.hiddenChannels.has(c.id) && !(c.channel.group && library.hiddenGroups.has(c.channel.group)),
        ),
  );

  movies = $derived(
    this.visibleSources.flatMap((source) =>
      (this.catalogs[source.id]?.movies ?? []).map((movie): MovieItem => ({ id: source.id + movie.id, source, movie })),
    ),
  );

  series = $derived(
    this.visibleSources.flatMap((source) =>
      (this.catalogs[source.id]?.series ?? []).map((series): SeriesItem => ({ id: source.id + series.id, source, series })),
    ),
  );

  async init() {
    setInterval(() => (this.now = Date.now() / 1000), 30_000);
    this.sources = await invoke<Source[]>('sources_load');
    // Last launch's catalogs and guides first, so the screens fill at once; then the network.
    await Promise.all(this.sources.map((s) => this.restore(s)));
    for (const s of this.sources) void this.refresh(s);
  }

  private async restore(source: Source) {
    try {
      const [catalog, guide] = await Promise.all([
        invoke<Catalog | null>('catalog_cached', { id: source.id }),
        invoke<Record<string, Programme[]> | null>('guide_cached', { id: source.id }),
      ]);
      if (catalog && !this.catalogs[source.id]) this.catalogs = { ...this.catalogs, [source.id]: catalog };
      if (guide) this.mergeGuide(guide);
    } catch (e) {
      console.warn(`cache ${source.name}: ${e}`);
    }
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
    if (catalog.epgUrl) void this.loadGuide(source, catalog.epgUrl, catalog.channels);
  }

  async loadGuide(source: Source, url: string, channels: Channel[]) {
    const keys = [...new Set(channels.flatMap((c) => (c.epgId ? [epgKey(c.epgId)] : [])))];
    if (!keys.length) return;
    try {
      this.mergeGuide(await invoke<Record<string, Programme[]>>('guide', { url, keys, cache: source.id }));
    } catch (e) {
      console.warn(`guide ${source.name}: ${e}`);
    }
  }

  /** Into the current guide, not a snapshot: several guides load at once. */
  private mergeGuide(incoming: Record<string, Programme[]>) {
    const merged = { ...this.guide };
    for (const [key, list] of Object.entries(incoming)) {
      merged[key] = merged[key] ? withoutRepeats([...merged[key], ...list]) : list;
    }
    this.guide = merged;
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
    if (player.current?.item?.source.id === source.id) player.stop();
    void invoke('cache_drop', { id: source.id });
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
    prefs.setJson('favorites', [...this.favorites]);
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

  /** From where it was left, unless `fromStart`. */
  playMovie({ id, movie }: MovieItem, fromStart = false) {
    const resume = { kind: 'movie' as const, id };
    player.play({
      id,
      title: movie.name,
      subtitle: movie.group ?? '',
      url: movie.stream,
      live: false,
      start: fromStart ? 0 : library.startAt(id),
      onprogress: (t, d, final) => library.record(id, resume, t, d, final),
    });
    void player.expand();
  }

  /** `list` is the series' episodes, for the next one; asked for when missing (the Continue row). */
  async playEpisode(item: SeriesItem, e: Episode, list?: Episode[], fromStart = false) {
    let episodes = list;
    if (!episodes) {
      try {
        episodes = await this.episodes(item);
      } catch (err) {
        console.warn(`episodes ${item.series.name}: ${err}`);
      }
    }
    const id = episodeKey(item.id, e);
    const resume = { kind: 'episode' as const, series: item.id, episode: e };
    const i = episodes?.findIndex((x) => x.id === e.id) ?? -1;
    const next = i >= 0 ? episodes?.[i + 1] : undefined;
    player.play({
      id,
      title: e.title,
      subtitle: `${item.series.name} · S${e.season}E${e.number}`,
      url: e.stream,
      live: false,
      start: fromStart ? 0 : library.startAt(id),
      onprogress: (t, d, final) => library.record(id, resume, t, d, final),
      next: next && {
        title: `S${next.season}E${next.number} · ${next.title}`,
        play: () => void this.playEpisode(item, next, episodes),
      },
    });
    void player.expand();
  }

  episodes({ source, series }: SeriesItem): Promise<Episode[]> {
    return invoke<Episode[]>('episodes', { source, series });
  }

  /** A channel by the number typed, from the source playing first when several sources share it. */
  byNumber(n: number): LiveItem | undefined {
    const matches = this.channels.filter((c) => c.channel.number === n);
    return matches.find((c) => c.source.id === player.current?.item?.source.id) ?? matches[0];
  }

  async tune(item: LiveItem) {
    library.tuned(item.id);
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
