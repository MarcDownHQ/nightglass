import type { LiveItem } from './models';

/** A one-field question (a list name), answered in `PromptDialog.svelte`. */
class Prompt {
  title = $state<string | null>(null);
  value = $state('');
  /** Typed characters show as dots: the PIN. */
  secret = $state(false);
  private resolve: ((value: string | null) => void) | null = null;

  ask(title: string, value = '', { secret = false } = {}): Promise<string | null> {
    this.resolve?.(null);
    this.title = title;
    this.value = value;
    this.secret = secret;
    return new Promise((resolve) => (this.resolve = resolve));
  }

  answer(value: string | null) {
    this.title = null;
    const resolve = this.resolve;
    this.resolve = null;
    resolve?.(value?.trim() ? value.trim() : null);
  }
}

export const prompt = new Prompt();

/** The Organize window: categories and favourites lists. */
class Organizer {
  open = $state(false);
}

export const organizer = new Organizer();

/** The channel being matched with a guide channel by hand (`EpgMatchDialog.svelte`). */
class EpgMatch {
  item = $state<LiveItem | null>(null);
}

export const epgMatch = new EpgMatch();

/** The Settings and keyboard-help windows. */
class Panels {
  settings = $state(false);
  help = $state(false);
}

export const panels = new Panels();

const FOCUSABLE = 'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), a[href], [tabindex]:not([tabindex="-1"])';

/** Action for a window over the app: the focus goes in, Tab stays in, and goes back where it was on close. */
export function dialogFocus(node: HTMLElement) {
  const back = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  const all = () => [...node.querySelectorAll<HTMLElement>(FOCUSABLE)];
  // After the content's own autofocus (the prompt's field); the close button only when nothing else takes the focus.
  queueMicrotask(() => node.contains(document.activeElement) || (all().find((el) => !el.closest('header')) ?? all()[0])?.focus());
  const onkeydown = (e: KeyboardEvent) => {
    const at = document.activeElement;
    // A window opened over this one (a prompt over Organize) keeps its own Tab.
    if (e.key !== 'Tab' || (!node.contains(at) && at?.closest('[role=dialog]'))) return;
    const list = all();
    const i = list.indexOf(at as HTMLElement);
    const next = e.shiftKey ? (i <= 0 ? list.at(-1) : null) : i === -1 || i === list.length - 1 ? list[0] : null;
    if (next) {
      e.preventDefault();
      next.focus();
    }
  };
  addEventListener('keydown', onkeydown, true);
  return {
    destroy() {
      removeEventListener('keydown', onkeydown, true);
      if (back?.isConnected) back.focus();
    },
  };
}
