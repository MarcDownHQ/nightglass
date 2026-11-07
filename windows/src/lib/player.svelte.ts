import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize, currentMonitor, getCurrentWindow } from '@tauri-apps/api/window';
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
  /** Live TV: another feed of the same channel, tried when this one fails. */
  fallback?: () => void;
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
  /** End of what the cache holds, in the timeline of `time`: on live TV, the live edge. */
  cacheTime = $state(0);
  /** The file a recording goes to, while it records. */
  recording = $state<string | null>(null);
  /** The small always-on-top window. */
  mini = $state(false);
  private bounds: { position: PhysicalPosition; size: PhysicalSize; fullscreen: boolean } | null = null;
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

  /** Live TV behind its live edge, in seconds: after a pause or a rewind. */
  get behind(): number {
    if (!this.current?.live || !this.started || this.atLive) return 0;
    const b = this.cacheTime - this.time - this.edgeLag;
    return b > 3 ? b : 0;
  }

  /**
   * How far the cache normally runs ahead of the picture on this stream (HLS reads a few segments
   * ahead): the largest gap seen while following live. Being behind is the gap beyond it.
   */
  private edgeLag = $state(0);
  /** Following live: no pause or rewind since the stream started or went back to live. */
  private atLive = $state(true);

  private trackEdge() {
    if (this.atLive && this.current?.live && this.started && this.cacheTime > 0) {
      this.edgeLag = Math.max(this.edgeLag, this.cacheTime - this.time);
    }
  }

  play(p: Playable) {
    if (p.id === this.current?.id && !this.error) return;
    this.report(true);
    void this.stopRecording();
    this.current = p;
    this.retunes = 0;
    this.load(p);
  }

  private load(p: Playable) {
    this.error = null;
    this.loading = true;
    this.started = false;
    this.time = 0;
    this.cacheTime = 0;
    this.edgeLag = 0;
    this.atLive = true;
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
    void this.stopRecording();
    void invoke('mpv_command', { args: ['stop'] });
    this.current = null;
    this.loading = false;
    this.error = null;
    this.tracks = [];
    void (this.mini ? this.leaveMini() : this.leaveFullscreen());
  }

  /** Fullscreen playback: the whole screen, unless the window already filled it. */
  async expand() {
    if (this.mini) await this.leaveMini();
    if (!this.current || this.expanded) return;
    this.wasFullscreen = await win.isFullscreen();
    this.expanded = true;
    if (!this.wasFullscreen) await win.setFullscreen(true);
  }

  /** Back to the list: live TV keeps playing in the preview, a movie stops. */
  async collapse() {
    if (this.mini) await this.leaveMini();
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

  /** Films: anywhere. Live TV: within what the cache holds, never past the live edge. */
  seek(by: number) {
    if (!this.current) return;
    // Without `exact`, mpv lands on the next keyframe, which can be 10 s further.
    if (!this.current.live) void invoke('mpv_command', { args: ['seek', String(by), 'relative+exact'] });
    else if (by < 0 || this.behind > 0) {
      const step = Math.min(by, this.behind);
      // A rewind leaves live; catching all the way up comes back to it.
      this.atLive = by > 0 && step >= this.behind;
      void invoke('mpv_command', { args: ['seek', String(step), 'relative'] });
    }
  }

  /** Back to the live edge after a pause or a rewind. */
  goLive() {
    if (this.current?.live && this.behind > 0) {
      void invoke('mpv_command', { args: ['seek', String(this.cacheTime - this.edgeLag), 'absolute'] });
    }
    this.atLive = true;
    if (this.paused) this.togglePause();
  }

  /** Records the stream as it comes, to `Videos\\Nightglass`. */
  async toggleRecording() {
    if (this.recording) return this.stopRecording();
    if (!this.current) return;
    const d = new Date();
    const two = (n: number) => String(n).padStart(2, '0');
    const stamp = `${d.getFullYear()}-${two(d.getMonth() + 1)}-${two(d.getDate())} ${two(d.getHours())}h${two(d.getMinutes())}`;
    try {
      const path = await invoke<string>('record_path', { name: `${this.current.title} ${stamp}` });
      await invoke('mpv_set', { name: 'stream-record', value: path });
      this.recording = path;
      this.notify('Recording');
    } catch (e) {
      this.notify(`Recording failed: ${e}`);
    }
  }

  private async stopRecording() {
    const path = this.recording;
    if (!path) return;
    this.recording = null;
    await invoke('mpv_set', { name: 'stream-record', value: '' }).catch(() => {});
    this.notify(`Saved to ${path}`, 4000);
  }

  /** A small window above the others, in the screen's corner; again to go back. */
  async toggleMini() {
    if (this.mini) return this.leaveMini();
    if (!this.current) return;
    this.bounds = { position: await win.outerPosition(), size: await win.outerSize(), fullscreen: await win.isFullscreen() };
    if (this.bounds.fullscreen) await win.setFullscreen(false);
    this.expanded = true;
    this.mini = true;
    await win.setMinSize(null);
    await win.setSize(new LogicalSize(480, 270));
    const monitor = await currentMonitor();
    if (monitor) {
      const scale = monitor.scaleFactor;
      const x = (monitor.position.x + monitor.size.width) / scale - 480 - 24;
      const y = (monitor.position.y + monitor.size.height) / scale - 270 - 64;
      await win.setPosition(new LogicalPosition(x, y));
    }
    await win.setAlwaysOnTop(true);
  }

  private async leaveMini() {
    if (!this.mini) return;
    this.mini = false;
    this.expanded = false;
    await win.setAlwaysOnTop(false);
    await win.setMinSize(new LogicalSize(1024, 640));
    if (this.bounds) {
      await win.setSize(this.bounds.size);
      await win.setPosition(this.bounds.position);
      if (this.bounds.fullscreen) await win.setFullscreen(true);
    }
    this.bounds = null;
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
  notify(text: string, ms = 1600) {
    this.osd = text;
    clearTimeout(this.osdTimer);
    this.osdTimer = setTimeout(() => (this.osd = null), ms);
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
        } else if (p.live && p.fallback) {
          p.fallback();
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
        if (this.paused && this.current?.live) this.atLive = false;
        break;
      case 'paused-for-cache':
        this.buffering = value === true;
        break;
      case 'time-pos':
        this.time = n;
        this.report(false);
        this.trackEdge();
        break;
      case 'duration':
        this.duration = n;
        break;
      case 'demuxer-cache-time':
        this.cacheTime = n;
        this.trackEdge();
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
