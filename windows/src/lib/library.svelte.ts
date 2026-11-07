import { SvelteSet } from 'svelte/reactivity';
import type { SortMode } from './lineup';
import type { Episode } from './models';
import { t } from './i18n.svelte';
import { prefs } from './prefs';

/** What a Continue card needs to play again, without the screen that started it. */
export type Resume = { kind: 'movie'; id: string } | { kind: 'episode'; series: string; episode: Episode };

export interface Progress {
  position: number;
  duration: number;
  /** Unix ms of the last save: the Continue row is most recent first. */
  updated: number;
  watched: boolean;
  resume: Resume;
}

/** Past this share of the duration a film counts as watched: the credits don't need to run. */
const WATCHED = 0.92;
const KEEP = 300;
const RECENTS = 10;

/** A favourites list: its own tab, its own order, its own numbers. */
export interface FavoriteList {
  id: string;
  name: string;
  /** Channel ids, in the order the user set. */
  items: string[];
}

/** A programme to be told about when it starts. */
export interface Reminder {
  /** The channel's item id. */
  channel: string;
  channelName: string;
  title: string;
  start: number;
  end: number;
}

/** The first list holds what was starred before lists existed. */
function initialLists(): FavoriteList[] {
  const saved = prefs.json<FavoriteList[] | null>('favoriteLists', null);
  return saved ?? [{ id: 'favorites', name: 'Favorites', items: prefs.json<string[]>('favorites', []) }];
}

/** What the user did with the catalogs: recents, hidden channels and categories, where they stopped, volume and languages. */
class Library {
  /** Ids of the last channels tuned, most recent first. */
  recents = $state<string[]>(prefs.json('recents', []));
  hiddenChannels = new SvelteSet<string>(prefs.json('hiddenChannels', []));
  /** Category keys (`categoryKey`), hidden in every source. */
  hiddenGroups = new SvelteSet<string>(prefs.json('hiddenGroups', []));
  favoriteLists = $state<FavoriteList[]>(initialLists());
  /** Category keys in the order the user set; the others follow in playlist order. */
  categoryOrder = $state<string[]>(prefs.json('categoryOrder', []));
  /** Category key → name the user gave it. */
  categoryNames = $state<Record<string, string>>(prefs.json('categoryNames', {}));
  sort = $state<SortMode>(prefs.json('sort', 'playlist'));
  /** One row per channel, its HD/SD/backup feeds behind it. */
  mergeDuplicates = $state<boolean>(prefs.json('mergeDuplicates', true));
  /** Only the channels of this country or language prefix. */
  country = $state<string | null>(prefs.json('country', null));
  /** Seconds of live TV watched per channel, for the “Most watched” order. */
  watchTime = $state<Record<string, number>>(prefs.json('watchTime', {}));
  reminders = $state<Reminder[]>(prefs.json('reminders', []));
  /** Channel item id → XMLTV channel id chosen by hand, for channels the guide doesn't match. */
  epgOverrides = $state<Record<string, string>>(prefs.json('epgOverrides', {}));
  /** SHA-256 of the parental PIN; null when none is set. */
  pinHash = $state<string | null>(prefs.json('pinHash', null));
  /** Category keys behind the PIN, live and on demand alike. */
  lockedGroups = new SvelteSet<string>(prefs.json('lockedGroups', []));
  /** The PIN was typed in this session: locked categories show until the app closes or is locked again. */
  unlocked = $state(false);
  progress = $state<Record<string, Progress>>(prefs.json('progress', {}));
  volume = $state<number>(prefs.json('volume', 100));
  muted = $state<boolean>(prefs.json('muted', false));
  /** Language codes of the tracks last picked; mpv selects them by itself on the next file. */
  audioLang = $state<string | null>(prefs.json('audioLang', null));
  subLang = $state<string | null>(prefs.json('subLang', null));
  subOff = $state<boolean>(prefs.json('subOff', false));

  private lastSaved = 0;

  /** Saves one of the settings above under its own name. */
  persist(
    key: 'categoryOrder' | 'categoryNames' | 'sort' | 'mergeDuplicates' | 'country' | 'favoriteLists' | 'watchTime' | 'reminders' | 'epgOverrides' | 'pinHash',
  ) {
    prefs.setJson(key, this[key]);
  }

  /** Whether a category is out of sight behind the PIN right now. */
  isLocked(key: string | null | undefined): boolean {
    return !!key && !this.unlocked && !!this.pinHash && this.lockedGroups.has(key);
  }

  toggleLock(key: string) {
    if (!this.lockedGroups.delete(key)) this.lockedGroups.add(key);
    prefs.setJson('lockedGroups', [...this.lockedGroups]);
  }

  async setPin(pin: string | null) {
    this.pinHash = pin ? await sha256(pin) : null;
    this.persist('pinHash');
  }

  async checkPin(pin: string): Promise<boolean> {
    return !!this.pinHash && (await sha256(pin)) === this.pinHash;
  }

  hasReminder(channel: string, start: number): boolean {
    return this.reminders.some((r) => r.channel === channel && r.start === start);
  }

  toggleReminder(r: Reminder) {
    this.reminders = this.hasReminder(r.channel, r.start)
      ? this.reminders.filter((x) => !(x.channel === r.channel && x.start === r.start))
      : [...this.reminders, r].sort((a, b) => a.start - b.start);
    this.persist('reminders');
  }

  /** Reminders that are due: removed from the list and returned. Old ones are dropped. */
  takeDue(now: number): Reminder[] {
    const due = this.reminders.filter((r) => r.start - 60 <= now && r.end > now);
    const keep = this.reminders.filter((r) => r.start - 60 > now);
    if (keep.length !== this.reminders.length) {
      this.reminders = keep;
      this.persist('reminders');
    }
    return due;
  }

  setEpgOverride(channel: string, epgId: string | null) {
    if (epgId) this.epgOverrides[channel] = epgId;
    else delete this.epgOverrides[channel];
    this.persist('epgOverrides');
  }

  isFavorite(id: string): boolean {
    return this.favoriteLists.some((l) => l.items.includes(id));
  }

  /** Adds to the list or takes out of it. */
  toggleFavorite(listId: string, id: string) {
    const list = this.favoriteLists.find((l) => l.id === listId);
    if (!list) return;
    list.items = list.items.includes(id) ? list.items.filter((i) => i !== id) : [...list.items, id];
    this.persist('favoriteLists');
  }

  /** Moves a channel in its list: by `delta` places, or before `before` (drag and drop). */
  moveFavorite(listId: string, id: string, delta: number | { before: string | null }) {
    const list = this.favoriteLists.find((l) => l.id === listId);
    if (!list) return;
    const rest = list.items.filter((i) => i !== id);
    let at: number;
    if (typeof delta === 'number') at = Math.max(0, Math.min(rest.length, list.items.indexOf(id) + delta));
    else at = delta.before === null ? rest.length : Math.max(0, rest.indexOf(delta.before));
    rest.splice(at, 0, id);
    list.items = rest;
    this.persist('favoriteLists');
  }

  createList(name: string): FavoriteList {
    const list = { id: crypto.randomUUID(), name, items: [] };
    this.favoriteLists.push(list);
    this.persist('favoriteLists');
    return this.favoriteLists[this.favoriteLists.length - 1];
  }

  renameList(listId: string, name: string) {
    const list = this.favoriteLists.find((l) => l.id === listId);
    if (list && name.trim()) list.name = name.trim();
    this.persist('favoriteLists');
  }

  deleteList(listId: string) {
    this.favoriteLists = this.favoriteLists.filter((l) => l.id !== listId);
    this.persist('favoriteLists');
  }

  /** Live TV watched: counted once the channel has played a minute. */
  watched(id: string, seconds: number) {
    if (seconds < 60) return;
    this.watchTime[id] = (this.watchTime[id] ?? 0) + Math.round(seconds);
    this.persist('watchTime');
  }

  tuned(id: string) {
    this.recents = [id, ...this.recents.filter((r) => r !== id)].slice(0, RECENTS);
    prefs.setJson('recents', this.recents);
  }

  toggleHidden(set: SvelteSet<string>, value: string) {
    if (!set.delete(value)) set.add(value);
    prefs.setJson(set === this.hiddenGroups ? 'hiddenGroups' : 'hiddenChannels', [...set]);
  }

  /** Saved every few seconds of playback, and at the end. */
  record(id: string, resume: Resume, position: number, duration: number, force = false) {
    if (!(duration > 0)) return;
    const watched = position / duration >= WATCHED;
    const previous = this.progress[id];
    // State and storage change every few seconds, not on every frame: the screens under the player derive from it.
    if (!force && !(watched && !previous?.watched) && Math.abs(position - this.lastSaved) < 5) return;
    this.lastSaved = position;
    this.progress[id] = { position: watched ? 0 : position, duration, updated: Date.now(), watched: watched || (previous?.watched ?? false), resume };
    this.save();
  }

  setWatched(id: string, resume: Resume, watched: boolean) {
    const p = this.progress[id];
    this.progress[id] = { position: 0, duration: p?.duration ?? 0, updated: Date.now(), watched, resume };
    this.save();
  }

  /** Where to start: the saved position, unless it was watched to the end or barely started. */
  startAt(id: string): number {
    const p = this.progress[id];
    return p && !p.watched && p.position > 30 ? p.position : 0;
  }

  /** Unfinished films and episodes, most recent first; one episode per series. */
  get continuing(): [string, Progress][] {
    const seen = new Set<string>();
    return Object.entries(this.progress)
      .filter(([, p]) => !p.watched && p.position > 30)
      .sort(([, a], [, b]) => b.updated - a.updated)
      .filter(([, p]) => {
        if (p.resume.kind !== 'episode') return true;
        if (seen.has(p.resume.series)) return false;
        seen.add(p.resume.series);
        return true;
      });
  }

  private save() {
    // Oldest entries go first once the list is long.
    const entries = Object.entries(this.progress);
    if (entries.length > KEEP) {
      entries.sort(([, a], [, b]) => b.updated - a.updated);
      this.progress = Object.fromEntries(entries.slice(0, KEEP));
    }
    prefs.setJson('progress', this.progress);
  }

  setVolume(volume: number, muted = this.muted) {
    this.volume = Math.round(Math.min(130, Math.max(0, volume)));
    this.muted = muted;
    prefs.setJson('volume', this.volume);
    prefs.setJson('muted', this.muted);
  }

  setAudioLang(lang: string | null) {
    this.audioLang = lang;
    prefs.setJson('audioLang', lang);
  }

  setSubtitles(lang: string | null, off: boolean) {
    this.subLang = lang;
    this.subOff = off;
    prefs.setJson('subLang', lang);
    prefs.setJson('subOff', off);
  }
}

export const library = new Library();

async function sha256(text: string): Promise<string> {
  const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(`nightglass:${text}`));
  return [...new Uint8Array(digest)].map((b) => b.toString(16).padStart(2, '0')).join('');
}

/** A list's name as shown: the first list keeps its default name in the user's language until renamed. */
export function listLabel(list: FavoriteList): string {
  return list.id === 'favorites' && list.name === 'Favorites' ? t('Favorites') : list.name;
}

export function episodeKey(series: string, episode: Episode): string {
  return `${series}/${episode.id}`;
}
