import { FR } from './fr';
import { prefs } from './prefs';

export type Language = 'auto' | 'en' | 'fr';

/**
 * English is written in the code and is the key; French comes from `fr.ts`. The language follows
 * Windows (the webview's) unless the user picked one in Settings.
 */
class I18n {
  choice = $state<Language>(prefs.json('language', 'auto'));
  lang = $derived<'en' | 'fr'>(this.choice === 'auto' ? (navigator.language.toLowerCase().startsWith('fr') ? 'fr' : 'en') : this.choice);

  set(choice: Language) {
    this.choice = choice;
    prefs.setJson('language', choice);
    if (typeof document !== 'undefined') document.documentElement.lang = this.lang;
  }
}

export const i18n = new I18n();

/** macOS: ⌘ stands for Ctrl in shortcuts, and the window keeps its own buttons (traffic lights). */
export const MAC = typeof navigator !== 'undefined' && navigator.userAgent.includes('Mac OS');
/** Android TV: the app is the whole screen, with no window of its own to move, resize or take fullscreen. */
export const ANDROID = typeof navigator !== 'undefined' && navigator.userAgent.includes('Android');

/** The shortcut modifier pressed: Ctrl, or ⌘ on macOS. */
export function mod(e: KeyboardEvent): boolean {
  return e.ctrlKey || (MAC && e.metaKey);
}

/** `text` in the user's language, with `{name}` placeholders filled from `vars`. */
export function t(text: string, vars?: Record<string, string | number>): string {
  let out = (i18n.lang === 'fr' && FR[text]) || text;
  // Shortcut labels are written for Windows: "Ctrl K", "Ctrl+.", "Alt ↑ ↓", "F11" (macOS keeps it; its View menu has ⌃⌘F).
  if (MAC) out = out.replace(/\bCtrl[ +]?/g, '⌘').replace(/\bAlt /g, '⌥').replace(/\bF11\b/g, '⌃⌘F');
  return vars ? out.replace(/\{(\w+)\}/g, (m, k: string) => (k in vars ? String(vars[k]) : m)) : out;
}

/** "1 channel", "3 channels": English and French plural the same way above one. */
export function plural(n: number, one: string, many: string): string {
  return t(n === 1 || (i18n.lang === 'fr' && n === 0) ? one : many, { n });
}
