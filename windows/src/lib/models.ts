// The JSON shapes of nightglass-core (core/src/models.rs).

export interface Channel {
  id: string;
  name: string;
  number: number | null;
  group: string | null;
  logo: string | null;
  epgId: string | null;
  stream: string;
  headers: Record<string, string>;
}

export interface Movie {
  id: string;
  name: string;
  group: string | null;
  poster: string | null;
  stream: string;
  rating: number | null;
  plot: string | null;
}

export interface Series {
  id: string;
  name: string;
  group: string | null;
  poster: string | null;
  rating: number | null;
  plot: string | null;
}

export interface Episode {
  id: string;
  title: string;
  season: number;
  number: number;
  stream: string;
  plot: string | null;
}

/** Times are Unix seconds. */
export interface Programme {
  channel: string;
  start: number;
  end: number;
  title: string;
  desc: string | null;
}

export interface Catalog {
  channels: Channel[];
  movies: Movie[];
  series: Series[];
  epgUrl: string | null;
}

export type SourceKind =
  | { m3u: { url: string; epg: string | null } }
  | { xtream: { server: string; username: string; password: string } }
  | { stalker: { portal: string; mac: string } };

export interface Source {
  id: string;
  name: string;
  kind: SourceKind;
}

export function kindLabel(kind: SourceKind): 'M3U' | 'Xtream' | 'Stalker' {
  return 'm3u' in kind ? 'M3U' : 'xtream' in kind ? 'Xtream' : 'Stalker';
}

/** A channel and the source it came from (Stalker needs the source to resolve its link). */
export interface LiveItem {
  id: string;
  source: Source;
  channel: Channel;
}

export interface MovieItem {
  id: string;
  source: Source;
  movie: Movie;
}

export interface SeriesItem {
  id: string;
  source: Source;
  series: Series;
}

/** Same rule as `xmltv::epg_key`: iptv-org ids carry a feed suffix (`6ter.fr@SD`) that guides don't. */
export function epgKey(id: string): string {
  return id.split('@')[0].toLowerCase();
}
