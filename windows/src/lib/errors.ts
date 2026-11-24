import { t } from './i18n.svelte';
import type { Prompt } from './models';

/**
 * What went wrong, said for the user: the raw message from the server, mpv or the network becomes
 * a cause and something to try. Unknown messages pass through unchanged.
 */
const RULES: [RegExp, string][] = [
  [/^missing or invalid setting/i, 'A setting of this source is missing or invalid: add it again.'],
  [/^no provider .* is installed/i, 'This source comes from a plugin that isn’t installed any more.'],
  [/^the plugin failed/i, 'The plugin of this source failed: try again, or update the plugin.'],
  [/^invalid (url|path)/i, 'This address isn’t valid: it starts with http:// or https://, or it is the path of a file.'],
  [/identifiants refus|refused the credentials|auth.*fail|invalid credentials/i, 'The provider refused the username or password.'],
  [/\b(401|403|458|509)\b|forbidden|too many connections|max.?connections/i, 'The provider refused access: the account may have expired, or too many devices are watching at once.'],
  [/\b404\b|not found/i, 'The server has nothing at this address: the channel or the playlist may have moved.'],
  [/\b5\d\d\b|bad gateway|service unavailable/i, 'The provider’s server is failing. Try again in a moment.'],
  [/connexion impossible|connection failed|dns|resolve|connection refused|timed? ?out|network|unreachable|error sending request/i, 'The server can’t be reached: check the address and the internet connection.'],
  [/r[ée]ponse inattendue|unexpected|expected value|bad response/i, 'The server answered, but not like an IPTV server: check the address.'],
  [/fichier illisible|can't read the file|cannot read|no such file|introuvable/i, 'The file can’t be read: check the path.'],
  [/unrecognized file format|no video or audio/i, 'This address doesn’t lead to a video stream.'],
  [/loading failed|failed to open|stream ended/i, 'The stream didn’t play: the channel may be offline, blocked in this country, or refused by the provider.'],
];

export function friendly(raw: string): string {
  const message = raw.replace(/^Error:\s*/, '');
  const known = RULES.find(([pattern]) => pattern.test(message))?.[1];
  return known ? t(known) : message;
}

/** The prompt a command failed with, when a plugin waits for the user rather than failing. */
export function promptOf(e: unknown): Prompt | null {
  return typeof e === 'object' && e !== null && 'prompt' in e ? (e as { prompt: Prompt }).prompt : null;
}

/** A prompt in one line: what to do, where, and the code to enter. */
export function promptLine(p: Prompt): string {
  return [p.message, p.url, p.code && t('code {code}', { code: p.code })].filter(Boolean).join(' · ');
}

/** A command's error as text. */
export function errorText(e: unknown): string {
  const prompt = promptOf(e);
  return prompt ? promptLine(prompt) : String(e);
}
