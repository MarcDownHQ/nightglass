import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

/** How far the download of Google's Widevine module has got (`widevine-progress`). */
export type Progress = { stage: 'checking' } | { stage: 'downloading'; received: number; total: number } | { stage: 'verifying' };

/**
 * The "Protected content" setting: on while Google's Widevine module is installed. The window over the app explains it,
 * downloads it with its progress, and says why a protected item didn't play.
 */
class Widevine {
  version = $state<string | null>(null);
  /** The window: `blocked` when a protected item was started with the setting off, `explain` from the settings. */
  step = $state<'blocked' | 'explain' | 'installing' | 'failed' | null>(null);
  /** What was started, in `blocked`. */
  title = $state('');
  progress = $state<Progress | null>(null);
  error = $state<string | null>(null);
  /** Played once the module is in. */
  private then: (() => void) | null = null;

  get enabled(): boolean {
    return this.version !== null;
  }

  async load() {
    try {
      this.version = await invoke<string | null>('widevine_status');
    } catch (e) {
      console.warn(`widevine status: ${e}`);
    }
  }

  /** A protected item started with the setting off: why, with a button that turns it on and then plays `then`. */
  blocked(title: string, then: () => void) {
    this.title = title;
    this.then = then;
    this.step = 'blocked';
  }

  /** The setting ticked: what it means, before anything is downloaded. */
  explain() {
    this.then = null;
    this.step = 'explain';
  }

  async install() {
    this.step = 'installing';
    this.error = null;
    this.progress = null;
    const unlisten = await listen<Progress>('widevine-progress', (e) => (this.progress = e.payload));
    try {
      this.version = await invoke<string>('widevine_install');
      if (this.step !== 'installing') return;
      this.step = null;
      const then = this.then;
      this.then = null;
      then?.();
    } catch (e) {
      if (this.step !== 'installing') return;
      this.error = String(e);
      this.step = 'failed';
    } finally {
      unlisten();
    }
  }

  /** The setting unticked: a download stops, and the module leaves the disk. */
  async remove() {
    this.then = null;
    this.step = null;
    try {
      await invoke('widevine_remove');
    } finally {
      await this.load();
    }
  }

  /** The window closed: a download under way stops. */
  close() {
    if (this.step === 'installing') void this.remove();
    this.then = null;
    this.step = null;
  }
}

export const widevine = new Widevine();

/** Greyed out: protected, while the setting is off. */
export function locked(item: { protected?: boolean }): boolean {
  return !!item.protected && !widevine.enabled;
}
