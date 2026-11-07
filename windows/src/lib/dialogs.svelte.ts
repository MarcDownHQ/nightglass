/** A one-field question (a list name), answered in `PromptDialog.svelte`. */
class Prompt {
  title = $state<string | null>(null);
  value = $state('');
  private resolve: ((value: string | null) => void) | null = null;

  ask(title: string, value = ''): Promise<string | null> {
    this.resolve?.(null);
    this.title = title;
    this.value = value;
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
