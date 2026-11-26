import type { Prompt } from './models';

/**
 * A plugin waiting for the user when something was started: the prompt stays on screen, asked again every `retryIn`
 * seconds or on Retry, until the playback starts, fails otherwise, is cancelled, or something else is started.
 */
class PlayPrompt {
  /** What was started: a channel, a movie, an episode or a replay. */
  key = $state<string | null>(null);
  title = $state('');
  prompt = $state<Prompt | null>(null);
  busy = $state(false);
  private again: (() => Promise<unknown>) | null = null;
  /** Bumped on cancel: an answer started before then comes too late. */
  private generation = 0;

  /** `key` is being started: a prompt for something else is dropped. What `wanted` checks once the answer is in. */
  starting(key: string): number {
    if (this.key !== null && this.key !== key) this.cancel();
    return this.generation;
  }

  /** Whether an answer started at `token` still matters: nothing was cancelled or started elsewhere since. */
  wanted(token: number): boolean {
    return token === this.generation;
  }

  show(key: string, title: string, prompt: Prompt, again: () => Promise<unknown>) {
    this.key = key;
    this.title = title;
    this.prompt = prompt;
    this.again = again;
  }

  /** `key` played, or failed for another reason. */
  done(key: string) {
    if (this.key === key) this.clear();
  }

  cancel() {
    this.generation++;
    this.clear();
  }

  async retry() {
    const again = this.again;
    if (!again || this.busy) return;
    const token = this.generation;
    this.busy = true;
    try {
      await again();
    } finally {
      if (this.wanted(token)) this.busy = false;
    }
  }

  private clear() {
    this.key = null;
    this.prompt = null;
    this.again = null;
    this.busy = false;
  }
}

export const playPrompt = new PlayPrompt();
