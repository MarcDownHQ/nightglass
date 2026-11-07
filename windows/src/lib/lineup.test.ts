import { describe, expect, it } from 'vitest';
import { categoryKey, categoryLabel, channelKey, groupDuplicates, qualityRank, splitCountry } from './lineup';

describe('splitCountry', () => {
  it.each([
    ['FR | TF1', 'FR', 'TF1'],
    ['|FR| TF1 HD', 'FR', 'TF1 HD'],
    ['[FR] Polar', 'FR', 'Polar'],
    ['FR: France 2', 'FR', 'France 2'],
    ['UK - BBC One', 'UK', 'BBC One'],
    ['GB | Sky News', 'UK', 'Sky News'],
    ['AR | MBC', 'AR', 'MBC'],
    ['FRA | Canal+', 'FRA', 'Canal+'],
    ['EX-YU | RTS', 'EX-YU', 'RTS'],
  ])('%s', (name, country, rest) => {
    expect(splitCountry(name)).toEqual({ country, rest });
  });

  it.each(['TF1', 'Arte', 'TV5 Monde', 'TV | Something', 'HD | Film', 'BFM TV', 'M6', 'France 24', 'NRJ 12'])('keeps %s', (name) => {
    expect(splitCountry(name)).toEqual({ country: null, rest: name });
  });
});

describe('channelKey', () => {
  it('ignores prefixes, quality and case', () => {
    const keys = ['TF1', 'TF1 HD', 'FR | TF1 FHD', '|FR| tf1 (1080p)', 'TF1 [Not 24/7]', 'TF1 HEVC', 'TF1 4K'].map(channelKey);
    expect(new Set(keys)).toEqual(new Set(['tf1']));
  });

  it('keeps different channels apart', () => {
    expect(channelKey('France 2')).not.toBe(channelKey('France 3'));
    expect(channelKey('Canal+ Sport')).not.toBe(channelKey('Canal+'));
    expect(channelKey('Sport 1')).not.toBe(channelKey('Sport 2'));
  });

  it('folds accents', () => {
    expect(channelKey('Ciné+ Émotion')).toBe(channelKey('CINE+ EMOTION HD'));
  });
});

describe('qualityRank', () => {
  it('orders 4K, FHD, HD, SD', () => {
    expect(['TF1 SD', 'TF1', 'TF1 HD', 'TF1 FHD', 'TF1 4K'].map(qualityRank)).toEqual([1, 2, 2, 3, 4]);
    expect(qualityRank('TF1 (1080p)')).toBe(3);
  });
});

describe('categories', () => {
  it('merges prefixed names', () => {
    expect(categoryKey('FR | Sport')).toBe(categoryKey('Sport '));
    expect(categoryKey('|FR| CINÉMA')).toBe(categoryKey('Cinéma'));
    expect(categoryLabel('FR | Sport')).toBe('Sport');
  });
});

describe('groupDuplicates', () => {
  it('leads with the best feed, at the first position', () => {
    const groups = groupDuplicates(['Info', 'TF1 SD', 'Arte', 'FR | TF1 FHD', 'TF1 HD'], (s) => s);
    expect(groups.map((g) => g.lead)).toEqual(['Info', 'FR | TF1 FHD', 'Arte']);
    expect(groups[1].variants).toEqual(['FR | TF1 FHD', 'TF1 HD', 'TF1 SD']);
  });
});
