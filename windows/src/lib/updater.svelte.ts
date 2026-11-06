import { check, type Update } from '@tauri-apps/plugin-updater';

/**
 * New versions are offered, never forced: a check at launch, then a button in the title bar.
 * Installing runs the NSIS installer in passive mode, which closes the app and starts the new version.
 */
class Updater {
  available = $state<Update | null>(null);
  /** Download progress, 0…1, while installing. */
  progress = $state<number | null>(null);
  error = $state<string | null>(null);

  async check() {
    // Dev builds have no release to compare with.
    if (import.meta.env.DEV) return;
    try {
      this.available = await check();
    } catch (e) {
      // Offline, or no release published yet: try again next launch.
      console.warn(`update check: ${e}`);
    }
  }

  async install() {
    const update = this.available;
    if (!update || this.progress !== null) return;
    this.error = null;
    this.progress = 0;
    let total = 0;
    let received = 0;
    try {
      await update.downloadAndInstall((e) => {
        if (e.event === 'Started') total = e.data.contentLength ?? 0;
        else if (e.event === 'Progress') {
          received += e.data.chunkLength;
          this.progress = total ? received / total : 0;
        }
      });
    } catch (e) {
      this.error = String(e);
      this.progress = null;
    }
  }
}

export const updater = new Updater();
