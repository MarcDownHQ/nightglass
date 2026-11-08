import { fold } from './lineup';

/**
 * Ctrl+K over hundreds of thousands of names without blocking typing: the names are folded once
 * (an index, kept while the lists don't change), then each search scans them in slices, giving the
 * page a turn between slices and stopping as soon as a newer search starts.
 */

/** Items and their folded names, side by side. */
export interface Column<T> {
  items: readonly T[];
  keys: string[];
}

/** The matches, up to a limit, and how many there are in all. */
export interface Found<T> {
  list: T[];
  count: number;
}

/** Slices of work end after this long, so a key press waits at most about that. */
const SLICE_MS = 8;

const nextTurn = () => new Promise<void>((r) => setTimeout(r));

/** True when the work should stop: a newer search began, or the palette closed. */
export type Stale = () => boolean;

/** Folds every name, a slice at a time; null if it went stale meanwhile. */
export async function column<T>(items: readonly T[], name: (item: T) => string, stale: Stale): Promise<Column<T> | null> {
  const keys = new Array<string>(items.length);
  let i = 0;
  while (i < items.length) {
    const end = performance.now() + SLICE_MS;
    do {
      const stop = Math.min(i + 500, items.length);
      for (; i < stop; i++) keys[i] = fold(name(items[i]));
    } while (i < items.length && performance.now() < end);
    if (i < items.length) {
      await nextTurn();
      if (stale()) return null;
    }
  }
  return { items, keys };
}

/** The items whose folded name contains `q` (already folded) and pass `keep`; null if it went stale. */
export async function find<T>(col: Column<T>, q: string, limit: number, stale: Stale, keep?: (item: T) => boolean): Promise<Found<T> | null> {
  const list: T[] = [];
  let count = 0;
  const { items, keys } = col;
  let i = 0;
  while (i < keys.length) {
    const end = performance.now() + SLICE_MS;
    do {
      const stop = Math.min(i + 5000, keys.length);
      for (; i < stop; i++) {
        if (!keys[i].includes(q) || (keep && !keep(items[i]))) continue;
        count++;
        if (list.length < limit) list.push(items[i]);
      }
    } while (i < keys.length && performance.now() < end);
    if (i < keys.length) {
      await nextTurn();
      if (stale()) return null;
    }
  }
  return { list, count };
}
