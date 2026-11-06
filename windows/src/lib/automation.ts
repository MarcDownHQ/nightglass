import { invoke } from '@tauri-apps/api/core';
import type { Catalog, Source } from './models';
import { nav, type Section } from './nav.svelte';
import { player } from './player.svelte';
import { store } from './store.svelte';

/**
 * Debug-only test hooks, driven by environment variables (the same names as the macOS app):
 * - `NG_SEED`: JSON `[Source]`, ids optional, added like the Sources screen does.
 * - `NG_SECTION`: LIVE / GUIDE / MOVIES / SERIES / SOURCES.
 * - `NG_ACTIVE`: name of the source to show.
 * - `NG_PLAY`: part of a channel name to tune; `NG_EXPAND=1` opens it fullscreen.
 * - `NG_KEYCHAIN_ACCOUNT`: read by the Rust side, a separate sources file for tests.
 */
export async function runAutomation() {
  const env = await invoke<Record<string, string>>('automation');
  const log = (message: string) => console.info(`[NG] ${message}`);
  if (env.NG_SECTION) nav.section = env.NG_SECTION.toLowerCase() as Section;
  if (env.NG_SEED) {
    for (const seed of JSON.parse(env.NG_SEED) as Partial<Source>[]) {
      if (store.sources.some((s) => s.name === seed.name)) continue;
      const source = { id: crypto.randomUUID(), ...seed } as Source;
      try {
        store.add(source, await invoke<Catalog>('catalog', { source }));
        log(`seeded ${source.name}`);
      } catch (e) {
        log(`seed ${source.name} failed: ${e}`);
      }
    }
  }
  if (env.NG_ACTIVE) store.setActive(store.sources.find((s) => s.name === env.NG_ACTIVE)?.id ?? null);
  if (env.NG_PLAY) {
    const name = env.NG_PLAY.toLowerCase();
    const find = () => store.channels.find((c) => c.channel.name.toLowerCase().includes(name));
    for (let i = 0; i < 60 && !find(); i++) await new Promise((r) => setTimeout(r, 1000));
    const item = find();
    if (!item) return log(`no channel matching ${env.NG_PLAY}`);
    nav.section = 'live';
    await store.tune(item);
    if (env.NG_EXPAND === '1') setTimeout(() => void player.expand(), 1000);
  }
}
