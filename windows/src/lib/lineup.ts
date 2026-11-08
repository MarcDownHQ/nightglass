// Pure rules for ordering a channel list: country prefixes, duplicate feeds, category names.
// No Svelte state here, so they are tested on their own (lineup.test.ts).

/** Two-letter codes that are not countries but sit in names the same way. */
const NOT_COUNTRIES = new Set(['TV', 'HD', 'SD', 'FM', 'VIP', 'PPV', 'VOD', 'UHD', 'FHD', 'HQ', 'LQ', 'NO', 'MY']);
/** Three-letter codes panels use for languages or regions. */
const LONG_CODES = new Set(['AFR', 'ARA', 'ARB', 'DEU', 'ENG', 'ESP', 'FRA', 'GER', 'ITA', 'NLD', 'POL', 'POR', 'RUS', 'TUR', 'USA', 'LAT', 'EXYU']);

/**
 * "FR | TF1", "|FR| TF1", "[FR] TF1", "FR: TF1", "FR - TF1" → { country: "FR", rest: "TF1" }.
 * Only an upper-case code followed by a separator counts: "Arte" or "TV5 Monde" keep their name.
 */
export function splitCountry(name: string): { country: string | null; rest: string } {
  const m = name.match(/^\s*[|[(]?\s*([A-Z]{2,4}(?:-[A-Z]{2})?)\s*(?:[|\])]\s*[:|-]?|:|\s-\s|\s\|)\s*(.+)$/);
  if (m) {
    const code = m[1];
    if ((code.length === 2 && !NOT_COUNTRIES.has(code)) || LONG_CODES.has(code.replace('-', '')) || /^[A-Z]{2}-[A-Z]{2}$/.test(code)) {
      return { country: code === 'GB' ? 'UK' : code, rest: m[2].trim() };
    }
  }
  return { country: null, rest: name.trim() };
}

const QUALITY = /\b(4k|8k|uhd|fhd|hd|sd|hq|lq|hevc|h\.?26[45]|\d{3,4}p|\d{2}\s?fps|backup|raw)\b/gi;

/** "TF1 HD", "FR | TF1 FHD", "TF1 (1080p)" → "tf1": what makes two feeds the same channel. */
export function channelKey(name: string): string {
  return fold(splitCountry(name).rest.replace(/\[[^\]]*\]/g, ' ').replace(QUALITY, ' ').replace(/[()|:+*_.\-–—]/g, ' '))
    .replace(/\s+/g, ' ')
    .trim();
}

/**
 * How good a feed is, from its name: 4K 4, FHD 3, HD or nothing said 2, SD 1. HEVC feeds lose half a
 * point, behind their equals: providers use them to save bandwidth, sound included (17 kbit/s seen).
 */
export function qualityRank(name: string): number {
  const n = name.normalize('NFKC').toLowerCase();
  const hevc = /\b(hevc|h\.?265)\b/.test(n) ? 0.5 : 0;
  if (/\b(4k|uhd|2160p)\b/.test(n)) return 4 - hevc;
  if (/\b(fhd|1080p)\b/.test(n)) return 3 - hevc;
  if (/\b(sd|480p|576p|360p|lq)\b/.test(n)) return 1 - hevc;
  return 2 - hevc;
}

/** "FR | Sport", "|FR| SPORT ", "Sport" → "sport": categories of several sources and prefixes become one. */
export function categoryKey(group: string): string {
  return fold(splitCountry(group).rest).replace(/\s+/g, ' ').trim();
}

/** The name to show for a category: without its country prefix. */
export function categoryLabel(group: string): string {
  return splitCountry(group).rest;
}

/** Case and accents don't matter: "ete" finds "Été". */
export function fold(s: string): string {
  return s.normalize('NFD').replace(/\p{Diacritic}/gu, '').toLowerCase();
}

export type SortMode = 'playlist' | 'name' | 'number' | 'watched';

/**
 * Groups feeds of the same channel: the best one leads, the others follow as variants, in quality
 * order. The group sits where its first feed was in the playlist.
 */
export function groupDuplicates<T>(items: T[], name: (item: T) => string): { lead: T; variants: T[] }[] {
  const groups = new Map<string, T[]>();
  const order: string[] = [];
  for (const item of items) {
    const key = channelKey(name(item)) || name(item);
    const list = groups.get(key);
    if (list) list.push(item);
    else {
      groups.set(key, [item]);
      order.push(key);
    }
  }
  return order.map((key) => {
    // Stable sort: equal quality keeps the playlist order.
    const variants = [...groups.get(key)!].sort((a, b) => qualityRank(name(b)) - qualityRank(name(a)));
    return { lead: variants[0], variants };
  });
}
