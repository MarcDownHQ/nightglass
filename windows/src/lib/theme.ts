import type { Programme } from './models';

const NEONS = ['var(--magenta)', 'var(--cyan)', 'var(--acid)'];

/** Stable neon for a name, the same hash as the macOS app so a channel keeps its colour everywhere. */
export function neon(name: string): string {
  let h = 5381n;
  for (const ch of name) h = BigInt.asIntN(64, h * 33n + BigInt(ch.codePointAt(0)!));
  return NEONS[Number((h < 0n ? -h : h) % 3n)];
}

/** "Alpe d'Huez TV (720p) [Not 24/7]" → "Alpe d'Huez TV". The quality comes back as a tag via `qualityTag`. */
export function displayName(name: string): string {
  return name.replace(/\s*(\((\d{3,4}p|4K|HD|FHD|SD)\)|\[[^\]]*\])/gi, '').trim();
}

export function qualityTag(name: string): string | null {
  const m = name.match(/\((\d{3,4})p\)/);
  if (!m) return null;
  const n = Number(m[1]);
  return n >= 2160 ? '4K' : n >= 1080 ? 'FHD' : n >= 720 ? 'HD' : 'SD';
}

export function initials(name: string): string {
  return displayName(name)
    .split(/\s+/)
    .map((w) => w[0] ?? '')
    .slice(0, 2)
    .join('')
    .toUpperCase();
}

/** Hours and minutes of a Unix time, in the user's locale. */
export function hm(seconds: number): string {
  return new Date(seconds * 1000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
}

export function progress(p: Programme, now: number): number {
  return Math.min(1, Math.max(0, (now - p.start) / (p.end - p.start)));
}

export function clock(s: number): string {
  if (!Number.isFinite(s) || s < 0) return '--:--';
  const i = Math.floor(s);
  const two = (n: number) => String(n).padStart(2, '0');
  return i >= 3600 ? `${Math.floor(i / 3600)}:${two(Math.floor(i / 60) % 60)}:${two(i % 60)}` : `${two(Math.floor(i / 60))}:${two(i % 60)}`;
}
