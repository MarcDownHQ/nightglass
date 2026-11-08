import { t } from './i18n.svelte';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { library } from './library.svelte';
import type { LiveItem } from './models';
import { hole } from './video.svelte';

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
  /** Start of what the cache holds: how far back the picture can go. */
  cacheStart = $state(0);
  /** The file a recording goes to, while it records. */
  recording = $state<string | null>(null);
  /** Picture in picture: the video in its own small window above the others, the app free behind it. */
  pip = $state(false);
  /** A short message over the picture: the volume as it changes. */
  osd = $state<string | null>(null);
  private osdTimer: ReturnType<typeof setTimeout> | undefined;
  private started = false;
  private retunes = 0;
  /** Live TV dropped and is being tuned again: the attempt number, 0 otherwise. */
  reconnecting = $state(0);
  private retryTimer: ReturnType<typeof setTimeout> | undefined;
  private stallTimer: ReturnType<typeof setTimeout> | undefined;
  private onOnline: (() => void) | null = null;
  private wasFullscreen = false;

  constructor() {
    void listen<MpvEvent>('mpv', ({ payload }) => this.handle(payload));
    // mpv outlives the page: after a reload, start from silence rather than an orphan stream.
    void invoke('mpv_command', { args: ['stop'] });
    void this.applyVolume();
    void this.applyLanguages();
    // The PiP window outlives the page too.
    void invoke('pip_close');
    void listen<'pause' | 'expand'>('pip-action', ({ payload }) => (payload === 'pause' ? this.togglePause() : void this.expand()));
    void listen('pip-closed', () => {
      // Back in the main window, the video's place must be sent again.
      hole.forget();
      this.pip = false;
    });
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
    this.cancelReconnect();
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
    this.cacheStart = 0;
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
    this.cancelReconnect();
    this.report(true);
    void this.stopRecording();
    void invoke('mpv_command', { args: ['stop'] });
    this.current = null;
    this.loading = false;
    this.error = null;
    this.tracks = [];
    void (this.pip ? invoke('pip_close') : this.leaveFullscreen());
  }

  /** Fullscreen playback: the whole screen, unless the window already filled it. */
  async expand() {
    if (this.pip) {
      await invoke('pip_close');
      await win.setFocus();
    }
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

  /** Again, after an error: the same stream from the start. */
  retry() {
    const p = this.current;
    if (!p) return;
    this.cancelReconnect();
    this.retunes = 0;
    this.load(p);
  }

  /**
   * A live stream that played and dropped (or stalled) is tuned again after 1, 2, 4… up to 30 s, for
   * about two and a half minutes; offline, it waits for the network to come back. False when it gave up.
   */
  private reconnect(p: Playable): boolean {
    const delays = [1, 2, 4, 8, 15, 30, 30, 30];
    if (this.retunes >= delays.length) return false;
    const delay = delays[this.retunes++];
    this.reconnecting = this.retunes;
    this.loading = true;
    this.error = null;
    this.cancelTimers();
    const go = () => {
      this.onOnline = null;
      if (this.current === p) this.load(p);
    };
    if (!navigator.onLine) {
      this.onOnline = go;
      addEventListener('online', go, { once: true });
    } else this.retryTimer = setTimeout(go, delay * 1000);
    return true;
  }

  private cancelTimers() {
    clearTimeout(this.retryTimer);
    clearTimeout(this.stallTimer);
    if (this.onOnline) removeEventListener('online', this.onOnline);
    this.onOnline = null;
  }

  private cancelReconnect() {
    this.cancelTimers();
    this.reconnecting = 0;
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
      this.notify(t('Recording'));
    } catch (e) {
      this.notify(t('Recording failed: {error}', { error: String(e) }));
    }
  }

  private async stopRecording() {
    const path = this.recording;
    if (!path) return;
    this.recording = null;
    await invoke('mpv_set', { name: 'stream-record', value: '' }).catch(() => {});
    this.notify(t('Saved to {path}', { path }), 4000);
  }

  /** The video in a small window above the others (PiP), the app free behind it; again to bring it back. */
  async togglePip() {
    // Closing it comes back as `pip-closed`.
    if (this.pip) return void (await invoke('pip_close'));
    if (!this.current) return;
    await this.leaveFullscreen();
    this.pip = true;
    try {
      await invoke('pip_open');
    } catch (e) {
      this.pip = false;
      this.notify(String(e));
    }
  }

  /** Live TV: where following live plays, in the timeline of `time`. */
  get liveEdge(): number {
    return this.cacheTime - this.edgeLag;
  }

  /** Films: anywhere. Live TV: within the cache; near the live edge is back to live. */
  seekTo(seconds: number) {
    if (this.current?.live) {
      if (seconds >= this.liveEdge - 3) return this.goLive();
      seconds = Math.max(seconds, this.cacheStart);
      this.atLive = false;
    }
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
    this.notify(t('Volume {n}%', { n: library.volume }));
  }

  toggleMute() {
    library.setVolume(library.volume, !library.muted);
    void this.applyVolume();
    this.notify(library.muted ? t('Muted') : t('Volume {n}%', { n: library.volume }));
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
        // Playing again: the next drop gets the whole series of attempts.
        this.retunes = 0;
        this.reconnecting = 0;
        break;
      case 'end-file': {
        const p = this.current;
        if (!p || (e.reason !== 'eof' && e.reason !== 'error')) break;
        // A live stream never ends on its own: the server dropped it, so tune again, a bounded number of times.
        if (p.live && (this.started || this.reconnecting) && this.reconnect(p)) {
          // Waiting for the next attempt.
        } else if (p.live && p.fallback) {
          this.reconnecting = 0;
          p.fallback();
        } else if (e.reason === 'error' || p.live) {
          this.loading = false;
          this.reconnecting = 0;
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
        clearTimeout(this.stallTimer);
        // A live stream stuck buffering for 20 s is as good as dropped: tune again.
        if (this.buffering && this.current?.live) {
          const p = this.current;
          this.stallTimer = setTimeout(() => {
            if (this.current === p && this.buffering && !this.reconnect(p)) this.error = 'Stream ended.';
          }, 20_000);
        }
        break;
      case 'time-pos':
        this.time = n;
        this.report(false);
        this.trackEdge();
        break;
      case 'duration':
        this.duration = n;
        break;
      case 'cache-start':
        this.cacheStart = n;
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

export function trackName(track: Track): string {
  const channels = track['demux-channel-count'] ?? 0;
  return [track.lang?.toUpperCase(), track.title, track.codec?.toUpperCase(), channels > 2 ? `${channels}ch` : undefined]
    .filter(Boolean)
    .join(' · ');
}
