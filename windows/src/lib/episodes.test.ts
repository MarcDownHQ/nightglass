import { describe, expect, it } from 'vitest';
import { episodeLabel, episodeLine } from './library.svelte';
import type { Episode } from './models';

const episode = (e: Partial<Episode>): Episode => ({ id: 'e', title: 'Pilot', season: null, number: null, stream: 's', plot: null, ...e });

describe('episodeLabel', () => {
  it('numbers episodes that have a season and a number', () => {
    expect(episodeLabel(episode({ season: 2, number: 5 }))).toBe('S2E5');
    expect(episodeLabel(episode({ number: 5 }))).toBe('E5');
  });

  it('dates a broadcast without a number', () => {
    const aired = Date.UTC(2026, 10, 21, 19) / 1000;
    expect(episodeLabel(episode({ aired }))).toBe(new Date(aired * 1000).toLocaleDateString([], { day: 'numeric', month: 'short', year: 'numeric' }));
  });

  it('leaves out a missing label', () => {
    expect(episodeLine(episode({ title: 'JT 20h' }))).toBe('JT 20h');
    expect(episodeLine(episode({ season: 1, number: 1 }))).toBe('S1E1 · Pilot');
  });
});
