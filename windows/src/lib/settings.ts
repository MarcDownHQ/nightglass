import { invoke } from '@tauri-apps/api/core';
import { ask, open, save } from '@tauri-apps/plugin-dialog';
import type { Source } from './models';
import { player } from './player.svelte';
import { store } from './store.svelte';

const FILTERS = [{ name: 'Nightglass settings', extensions: ['json'] }];

/** Every setting kept in the page's storage: favourites, lists, hidden and locked categories, progress… */
function pagePrefs(): Record<string, string> {
  const out: Record<string, string> = {};
  try {
    for (let i = 0; i < localStorage.length; i++) {
      const key = localStorage.key(i);
      if (key) out[key] = localStorage.getItem(key) ?? '';
    }
  } catch {
    // Storage unavailable: only the sources go.
  }
  return out;
}

/** Sources and settings to a JSON file, for another PC. The file holds the passwords in clear. */
export async function exportSettings() {
  const ok = await ask('The file will hold your sources with their passwords in clear. Keep it somewhere private.', {
    title: 'Export the settings',
    kind: 'warning',
    okLabel: 'Export',
  });
  if (!ok) return;
  const path = await save({ defaultPath: 'nightglass-settings.json', filters: FILTERS });
  if (!path) return;
  try {
    await invoke('settings_export', { path, prefs: pagePrefs() });
    player.notify('Settings exported');
  } catch (e) {
    player.notify(`Export failed: ${e}`, 4000);
  }
}

/** Sources (added, or replaced when they are the same) and settings (replaced) from such a file; the app reloads. */
export async function importSettings() {
  const path = await open({ multiple: false, directory: false, filters: FILTERS });
  if (!path) return;
  try {
    const [sources, prefs] = await invoke<[Source[], Record<string, string> | null]>('settings_import', { path });
    const ok = await ask(`${sources.length} source(s) and your favourites, lists, hidden categories and progress will replace what is here.`, {
      title: 'Import the settings',
      kind: 'warning',
      okLabel: 'Import',
    });
    if (!ok) return;
    await store.importSources(sources);
    try {
      for (const [key, value] of Object.entries(prefs ?? {})) localStorage.setItem(key, value);
    } catch {
      // Storage unavailable: the sources are in, the rest is lost.
    }
    location.reload();
  } catch (e) {
    player.notify(`Import failed: ${e}`, 4000);
  }
}
