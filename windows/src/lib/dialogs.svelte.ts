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
