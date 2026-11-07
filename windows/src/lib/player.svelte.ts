import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { library } from './library.svelte';
import type { LiveItem } from './models';

/** What the player is showing. */
export interface Playable {
  id: string;
  title: string;
  subtitle: string;
  url: string;
  headers?: Record<string, string>;
  live: boolean;
  number?: number;
  /** The channel, for live TV: zapping and now/next. */
  item?: LiveItem;
  /** Seconds to start at: where a film was left. */
  start?: number;
  /** Films and episodes: the position as it plays, then once more at the end or on stop. */
  onprogress?: (time: number, duration: number, final: boolean) => void;
  /** The next episode, offered in the HUD and played when this one ends. */
  next?: { title: string; play: () => void };
}

/** One entry of mpv's `track-list`. */
export interface Track {
  id: number;
  type: 'video' | 'audio' | 'sub';
  lang?: string;
  title?: string;
  codec?: string;
  selected: boolean;
  'demux-channel-count'?: number;
}

type MpvEvent =
  | { event: 'property'; name: string; value: unknown }
  | { event: 'end-file'; reason: string; error: string | null }
  | { event: 'file-loaded' }
  | { event: 'playback-restart' };

const win = getCurrentWindow();

class Player {
  current = $state<Playable | null>(null);
  expanded = $state(false);
  /** Load sent, no picture yet. */
  loading = $state(false);
  paused = $state(false);
  buffering = $state(false);
  time = $state(0);
  duration = $state(0);
  width = $state(0);
  height = $state(0);
  tracks = $state<Track[]>([]);
  error = $state<string | null>(null);
  /** A short message over the picture: the volume as it changes. */
  osd = $state<string | null>(null);
  private osdTimer: ReturnType<typeof setTimeout> | undefined;
  private started = false;
  private retunes = 0;
  private wasFullscreen = false;

  constructor() {
    void listen<MpvEvent>('mpv', ({ payload }) => this.handle(payload));
    // mpv outlives the page: after a reload, start from silence rather than an orphan stream.
    void invoke('mpv_command', { args: ['stop'] });
    void this.applyVolume();
    void this.applyLanguages();
  }

  get signal(): number {
    if (this.error) return 0;
    if (this.loading || this.buffering) return 2;
    if (this.paused) return 4;
    return this.current ? 5 : 1;
  }

  get audioTracks() {
    return this.tracks.filter((t) => t.type === 'audio');
  }

  get subtitleTracks() {
    return this.tracks.filter((t) => t.type === 'sub');
  }

  play(p: Playable) {
    if (p.id === this.current?.id && !this.error) return;
    this.report(true);
    this.current = p;
    this.retunes = 0;
    this.load(p);
  }

  private load(p: Playable) {
    this.error = null;
    this.loading = true;
    this.started = false;
    this.time = 0;
    this.duration = 0;
    // Per-file options: the start position and the subtitle choice must not carry over to the next file.
    const options = [p.start ? `start=${Math.floor(p.start)}` : '', 'aid=auto', `sid=${library.subOff ? 'no' : 'auto'}`].filter(Boolean).join(',');
    invoke('mpv_play', { url: p.url, headers: p.headers ?? {}, options }).catch((e) => (this.error = String(e)));
  }

  /** Tells the film or episode where it is; `final` when it stops being played. */
  private report(final: boolean) {
    const p = this.current;
    if (p?.onprogress && this.started && this.duration > 0) p.onprogress(this.time, this.duration, final);
  }

  stop() {
    this.report(true);
    void invoke('mpv_command', { args: ['stop'] });
    this.current = null;
    this.loading = false;
    this.error = null;
    this.tracks = [];
    void this.leaveFullscreen();
  }

  /** Fullscreen playback: the whole screen, unless the window already filled it. */
  async expand() {
    if (!this.current || this.expanded) return;
    this.wasFullscreen = await win.isFullscreen();
    this.expanded = true;
    if (!this.wasFullscreen) await win.setFullscreen(true);
  }

  /** Back to the list: live TV keeps playing in the preview, a movie stops. */
  async collapse() {
    if (this.current && !this.current.live) this.stop();
    else await this.leaveFullscreen();
  }

  private async leaveFullscreen() {
    if (!this.expanded) return;
    this.expanded = false;
    if (!this.wasFullscreen) await win.setFullscreen(false);
  }

  togglePause() {
    if (this.current) void invoke('mpv_command', { args: ['cycle', 'pause'] });
  }

  seek(by: number) {
    // Without `exact`, mpv lands on the next keyframe, which can be 10 s further.
    if (this.current && !this.current.live) void invoke('mpv_command', { args: ['seek', String(by), 'relative+exact'] });
  }

  seekTo(seconds: number) {
    void invoke('mpv_command', { args: ['seek', String(seconds), 'absolute'] });
  }

  /** The language of the track picked becomes the preferred one, for every next file. */
  selectTrack(type: 'audio' | 'sub', id: number | 'no') {
    void invoke('mpv_set', { name: type === 'audio' ? 'aid' : 'sid', value: String(id) });
    const lang = id === 'no' ? null : (this.tracks.find((t) => t.type === type && t.id === id)?.lang ?? null);
    if (type === 'audio') {
      if (lang) library.setAudioLang(lang);
    } else library.setSubtitles(lang ?? library.subLang, id === 'no');
    void this.applyLanguages();
  }

  private async applyLanguages() {
    const list = (lang: string | null) => (lang ? lang : '');
    await invoke('mpv_set', { name: 'alang', value: list(library.audioLang) }).catch(() => {});
    await invoke('mpv_set', { name: 'slang', value: list(library.subLang) }).catch(() => {});
  }

  setVolume(volume: number) {
    library.setVolume(volume, false);
    void this.applyVolume();
    this.notify(`Volume ${library.volume}%`);
  }

  toggleMute() {
    library.setVolume(library.volume, !library.muted);
    void this.applyVolume();
    this.notify(library.muted ? 'Muted' : `Volume ${library.volume}%`);
  }

  private async applyVolume() {
    await invoke('mpv_set', { name: 'volume', value: String(library.volume) }).catch(() => {});
    await invoke('mpv_set', { name: 'mute', value: library.muted ? 'yes' : 'no' }).catch(() => {});
  }

  /** A short message at the bottom of the window. */
  notify(text: string) {
    this.osd = text;
    clearTimeout(this.osdTimer);
    this.osdTimer = setTimeout(() => (this.osd = null), 1200);
  }

  private handle(e: MpvEvent) {
    switch (e.event) {
      case 'playback-restart':
        this.loading = false;
        this.started = true;
        break;
      case 'end-file': {
        const p = this.current;
        if (!p || (e.reason !== 'eof' && e.reason !== 'error')) break;
        // A live stream never ends on its own: the server dropped it, so tune again, a bounded number of times.
        if (p.live && this.started && this.retunes < 3) {
          this.retunes++;
          this.load(p);
        } else if (e.reason === 'error' || p.live) {
          this.loading = false;
          this.error = e.error ?? 'Stream ended.';
        } else {
          // Watched to the end: tell it so, then the next episode if there is one.
          if (p.onprogress && this.duration > 0) p.onprogress(this.duration, this.duration, true);
          this.started = false;
          if (p.next) p.next.play();
          else this.stop();
        }
        break;
      }
      case 'property':
        this.property(e.name, e.value);
        break;
    }
  }

  private property(name: string, value: unknown) {
    const n = typeof value === 'number' ? value : 0;
    switch (name) {
      case 'pause':
        this.paused = value === true;
        break;
      case 'paused-for-cache':
        this.buffering = value === true;
        break;
      case 'time-pos':
        this.time = n;
        this.report(false);
        break;
      case 'duration':
        this.duration = n;
        break;
      case 'width':
        this.width = n;
        break;
      case 'height':
        this.height = n;
        break;
      case 'track-list':
        this.tracks = Array.isArray(value) ? (value as Track[]) : [];
        break;
    }
  }
}

export const player = new Player();

export function trackName(t: Track): string {
  const channels = t['demux-channel-count'] ?? 0;
  return [t.lang?.toUpperCase(), t.title, t.codec?.toUpperCase(), channels > 2 ? `${channels}ch` : undefined]
    .filter(Boolean)
    .join(' · ');
}
