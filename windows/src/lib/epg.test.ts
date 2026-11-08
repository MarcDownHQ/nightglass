import { describe, expect, it } from 'vitest';
import { withoutOverlaps } from './epg';

const p = (start: number, end: number, title = `${start}`) => ({ channel: 'c', start, end, title, desc: null });

describe('withoutOverlaps', () => {
  it('keeps back-to-back programmes, sorted', () => {
    expect(withoutOverlaps([p(3600, 7200), p(0, 3600)]).map((x) => x.start)).toEqual([0, 3600]);
  });
  it('drops a repeat of the same start and one that overlaps', () => {
    expect(withoutOverlaps([p(0, 3600, 'a'), p(0, 3600, 'b'), p(1800, 5400, 'c'), p(3600, 7200, 'd')]).map((x) => x.title)).toEqual(['a', 'd']);
  });
  it('tolerates a minute of overlap', () => {
    expect(withoutOverlaps([p(0, 3630), p(3600, 7200)]).length).toBe(2);
  });
});
