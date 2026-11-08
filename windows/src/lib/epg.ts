import type { Programme } from './models';

/**
 * One programme at a time per channel: sorted by start, and one that starts before the previous kept
 * one ends (a second guide for the channel, or a feed with overlapping entries) is dropped, else the
 * grid stacks them on top of each other. A minute of overlap is tolerated: guides round their times.
 */
export function withoutOverlaps(list: Programme[]): Programme[] {
  list.sort((a, b) => a.start - b.start);
  const kept: Programme[] = [];
  for (const p of list) {
    const last = kept[kept.length - 1];
    if (!last || p.start >= last.end - 60) kept.push(p);
  }
  return kept;
}
