import { splitCountry } from './lineup';
import type { Programme } from './models';

const NEONS = ['var(--magenta)', 'var(--cyan)', 'var(--acid)'];

/** Stable neon for a name, the same hash as the macOS app so a channel keeps its colour everywhere. */
export function neon(name: string): string {
  let h = 5381n;
  for (const ch of name) h = BigInt.asIntN(64, h * 33n + BigInt(ch.codePointAt(0)!));
  return NEONS[Number((h < 0n ? -h : h) % 3n)];
}

/**
 * "FR | Alpe d'Huez TV HD (720p) [Not 24/7]" → "Alpe d'Huez TV": without the country prefix and the
 * quality, which comes back as a tag via `qualityTag`.
 */
export function displayName(name: string): string {
  const bare = splitCountry(name)
    .rest.replace(/\s*(\((\d{3,4}p|4K|HD|FHD|UHD|SD)\)|\[[^\]]*\])/gi, '')
    .replace(/(\s+(4K|UHD|FHD|HD|SD|HEVC|H\.?265))+\s*$/i, '')
    .trim();
  return bare || name.trim();
}

/** The quality a name announces, as a tag: 4K, FHD, HD or SD; nothing when it says nothing. */
export function qualityTag(name: string): string | null {
  const m = name.match(/\((\d{3,4})p\)|\b(\d{3,4})p\b/);
  if (m) {
    const n = Number(m[1] ?? m[2]);
    return n >= 2160 ? '4K' : n >= 1080 ? 'FHD' : n >= 720 ? 'HD' : 'SD';
  }
  const t = name.match(/\b(4K|UHD|FHD|HD|SD)\b/i)?.[1].toUpperCase();
  return t === 'UHD' ? '4K' : (t ?? null);
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
