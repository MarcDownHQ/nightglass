import { describe, expect, it } from 'vitest';
import { column, find } from './search';

describe('search', () => {
  const names = ['RMC Sport 1', 'rmc story', 'TF1', 'Été en RMC Découverte', ...Array.from({ length: 20_000 }, (_, i) => `Filler ${i}`)];

  it('finds folded names across slices, keeps the first ones and counts them all', async () => {
    const col = (await column(names, (n) => n, () => false))!;
    const found = (await find(col, 'rmc', 2, () => false))!;
    expect(found.list).toEqual(['RMC Sport 1', 'rmc story']);
    expect(found.count).toBe(3);
    expect((await find(col, 'ete', 10, () => false))!.list).toEqual(['Été en RMC Découverte']);
    expect((await find(col, 'rmc', 10, () => false, (n) => n !== 'TF1' && n.startsWith('r')))!.count).toBe(1);
  });
});
