import type { SeriesItem } from './models';

export type Section = 'live' | 'guide' | 'movies' | 'series' | 'sources';

/** The screens of the title bar, in order: Ctrl+1…5 picks them. */
export const SECTIONS: { id: Section; title: string }[] = [
  { id: 'live', title: 'Live' },
  { id: 'guide', title: 'Guide' },
  { id: 'movies', title: 'Movies' },
  { id: 'series', title: 'Series' },
  { id: 'sources', title: 'Sources' },
];

class Nav {
  section = $state<Section>('live');
  /** The search palette is open. */
  searching = $state(false);
  /** A channel picked in the guide or the search, for the Live screen to select and tune. */
  focusChannel = $state<string | null>(null);
  /** A channel to scroll to and mark in the Guide screen. */
  guideChannel = $state<string | null>(null);
  /** The series whose page is open in the Series screen. */
  openSeries = $state<SeriesItem | null>(null);
}

export const nav = new Nav();

/** Typing a channel number shows big digits, then tunes after a pause. */
class Dial {
  digits = $state('');
  private timer: ReturnType<typeof setTimeout> | undefined;

  type(digit: string, tune: (n: number) => void) {
    this.digits = (this.digits + digit).slice(-4);
    clearTimeout(this.timer);
    this.timer = setTimeout(() => {
      const n = Number(this.digits);
      this.digits = '';
      if (n) tune(n);
    }, 1100);
  }
}

export const dial = new Dial();

/** Keys typed into a field belong to the field, not to the screen's shortcuts. */
export function typing(e: KeyboardEvent): boolean {
  const t = e.target as HTMLElement | null;
  return !!t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.isContentEditable);
}
