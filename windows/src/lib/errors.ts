/**
 * What went wrong, said for the user: the raw message from the server, mpv or the network becomes
 * a cause and something to try. Unknown messages pass through unchanged.
 */
const RULES: [RegExp, string][] = [
  [/identifiants refus|auth.*fail|invalid credentials/i, 'The provider refused the username or password.'],
  [/\b(401|403|458|509)\b|forbidden|too many connections|max.?connections/i, 'The provider refused access: the account may have expired, or too many devices are watching at once.'],
  [/\b404\b|not found/i, 'The server has nothing at this address: the channel or the playlist may have moved.'],
  [/\b5\d\d\b|bad gateway|service unavailable/i, 'The provider’s server is failing. Try again in a moment.'],
  [/connexion impossible|dns|resolve|connection refused|timed? ?out|network|unreachable|error sending request/i, 'The server can’t be reached: check the address and the internet connection.'],
  [/r[ée]ponse inattendue|unexpected|expected value|bad response/i, 'The server answered, but not like an IPTV server: check the address.'],
  [/fichier illisible|cannot read|no such file|introuvable/i, 'The file can’t be read: check the path.'],
  [/unrecognized file format|no video or audio/i, 'This address doesn’t lead to a video stream.'],
  [/loading failed|failed to open|stream ended/i, 'The stream didn’t play: the channel may be offline, blocked in this country, or refused by the provider.'],
];

export function friendly(raw: string): string {
  const message = raw.replace(/^Error:\s*/, '');
  return RULES.find(([pattern]) => pattern.test(message))?.[1] ?? message;
}
