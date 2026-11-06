export type Section = 'live' | 'sources';

class Nav {
  section = $state<Section>('live');
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
