import { invoke } from '@tauri-apps/api/core';
import type { Info } from './models';

/** The provider gives a YouTube video id, or sometimes the whole address. */
export function trailerUrl(trailer: string): string {
  return /^https?:\/\//.test(trailer) ? trailer : `https://www.youtube.com/watch?v=${trailer}`;
}

/** Hands the trailer to the system: on a TV, the YouTube app. */
export function openTrailer(trailer: string): Promise<void> {
  return invoke<void>('open_url', { url: trailerUrl(trailer) });
}

/** "2024 · 1 h 52 · Action": the year, the length and the genre, when the provider says them. */
export function metaParts(info: Info | null): string[] {
  if (!info) return [];
  const year = info.release?.match(/\d{4}/)?.[0];
  const duration = info.duration ? lengthLabel(info.duration) : undefined;
  return [year, duration, info.genre].filter((p): p is string => !!p);
}

/** "01:52:10" is 1 h 52; "112 min" and bare minutes are kept as they are. */
function lengthLabel(duration: string): string {
  const clock = duration.match(/^(\d+):(\d{2})(?::\d{2})?$/);
  if (clock) {
    const h = Number(clock[1]);
    const m = Number(clock[2]);
    return h ? `${h} h ${String(m).padStart(2, '0')}` : `${m} min`;
  }
  return /^\d+$/.test(duration) ? `${duration} min` : duration;
}
