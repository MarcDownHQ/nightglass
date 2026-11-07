import { t } from './i18n.svelte';
import { invoke } from '@tauri-apps/api/core';
import { ask, open, save } from '@tauri-apps/plugin-dialog';
import type { Source } from './models';
import { player } from './player.svelte';
import { store } from './store.svelte';

const filters = () => [{ name: t('Nightglass settings'), extensions: ['json'] }];

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
  const ok = await ask(t('The file will hold your sources with their passwords in clear. Keep it somewhere private.'), {
    title: t('Export the settings'),
    kind: 'warning',
    okLabel: t('Export'),
  });
  if (!ok) return;
  const path = await save({ defaultPath: 'nightglass-settings.json', filters: filters() });
  if (!path) return;
  try {
    await invoke('settings_export', { path, prefs: pagePrefs() });
    player.notify(t('Settings exported'));
  } catch (e) {
    player.notify(t('Export failed: {error}', { error: String(e) }), 4000);
  }
}

/** Sources (added, or replaced when they are the same) and settings (replaced) from such a file; the app reloads. */
export async function importSettings() {
  const path = await open({ multiple: false, directory: false, filters: filters() });
  if (!path) return;
  try {
    const [sources, prefs] = await invoke<[Source[], Record<string, string> | null]>('settings_import', { path });
    const ok = await ask(t('{n} source(s) and your favourites, lists, hidden categories and progress will replace what is here.', { n: sources.length }), {
      title: t('Import the settings'),
      kind: 'warning',
      okLabel: t('Import'),
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
    player.notify(t('Import failed: {error}', { error: String(e) }), 4000);
  }
}
