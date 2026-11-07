import { describe, expect, it } from 'vitest';
import { FR } from './fr';
import { i18n, t } from './i18n.svelte';

/** The interface's source files, as text. */
const SOURCES = import.meta.glob(['./*.svelte', './*.ts', '../App.svelte', '!./*.test.ts'], { query: '?raw', import: 'default', eager: true }) as Record<string, string>;

/** Every string the interface passes to t() as a literal. */
function usedKeys(): Map<string, string> {
  const keys = new Map<string, string>();
  for (const [f, src] of Object.entries(SOURCES)) {
    for (const m of src.matchAll(/\bt\(\s*'((?:[^'\\]|\\.)*)'/g)) keys.set(m[1].replace(/\\\\/g, '\\'), f);
    for (const m of src.matchAll(/\bt\(\s*"((?:[^"\\]|\\.)*)"/g)) keys.set(m[1], f);
  }
  return keys;
}

describe('French', () => {
  it('has every string the interface uses', () => {
    const missing = [...usedKeys()].filter(([k]) => !(k in FR)).map(([k, f]) => `${f}: ${k}`);
    expect(missing).toEqual([]);
  });

  it('keeps the placeholders of each string', () => {
    const names = (s: string) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();
    const broken = Object.entries(FR).filter(([en, fr]) => names(en).join() !== names(fr).join());
    expect(broken).toEqual([]);
  });

  it('fills placeholders and follows the language', () => {
    i18n.set('fr');
    expect(t('Loading {name}…', { name: 'Arte' })).toBe('Chargement de Arte…');
    i18n.set('en');
    expect(t('Loading {name}…', { name: 'Arte' })).toBe('Loading Arte…');
  });
});
