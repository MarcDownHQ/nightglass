import { SvelteSet } from 'svelte/reactivity';
import type { Episode } from './models';
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

/** What the user did with the catalogs: recents, hidden channels and categories, where they stopped, volume and languages. */
class Library {
  /** Ids of the last channels tuned, most recent first. */
  recents = $state<string[]>(prefs.json('recents', []));
  hiddenChannels = new SvelteSet<string>(prefs.json('hiddenChannels', []));
  /** Category names, hidden in every source. */
  hiddenGroups = new SvelteSet<string>(prefs.json('hiddenGroups', []));
  progress = $state<Record<string, Progress>>(prefs.json('progress', {}));
  volume = $state<number>(prefs.json('volume', 100));
  muted = $state<boolean>(prefs.json('muted', false));
  /** Language codes of the tracks last picked; mpv selects them by itself on the next file. */
  audioLang = $state<string | null>(prefs.json('audioLang', null));
  subLang = $state<string | null>(prefs.json('subLang', null));
  subOff = $state<boolean>(prefs.json('subOff', false));

  private lastSaved = 0;

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

export function episodeKey(series: string, episode: Episode): string {
  return `${series}/${episode.id}`;
}
