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
  /** Seconds of past programmes the server can replay; null without replay. */
  catchupWindow?: number | null;
}

/** What a player opens: `stream_url`, `catchup_url`, `movie_url` and `episode_url` answer with it. */
export interface Playback {
  url: string;
  headers: Record<string, string>;
  /** The stream's own address when `url` is the app's relay or joins several tracks. */
  link?: string;
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
  /** Dated broadcasts (news, magazines) have neither season nor number. */
  season: number | null;
  number: number | null;
  stream: string;
  plot: string | null;
  /** Unix seconds. */
  aired?: number | null;
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
  account?: Account | null;
  /** Guide entries sent with the catalog, matched to channels by `epgId`. */
  programmes?: Programme[];
}

/** An IPTV subscription as the server describes it (Xtream). Times are Unix seconds. */
export interface Account {
  status: string | null;
  expires: number | null;
  maxConnections: number | null;
  activeConnections: number | null;
  trial: boolean;
}

export type FieldKind = 'text' | 'secret' | 'url' | 'location' | 'mac';

/** One setting a provider asks for when a source is added. `location` also takes a local file. */
export interface Field {
  key: string;
  label: string;
  kind: FieldKind;
  required: boolean;
  placeholder: string | null;
  default: string | null;
}

export interface Preset {
  label: string;
  name: string;
  config: Record<string, string>;
}

/** A source type: built in (M3U, Xtream, Stalker) or added by a plugin. */
export interface ProviderInfo {
  id: string;
  name: string;
  fields: Field[];
  /** Links are short-lived: `stream_url` is asked at play time. */
  resolvesStreams: boolean;
  presets: Preset[];
}

/** A file of the plugins folder: the provider it added, or why it didn't load. */
export interface PluginStatus {
  file: string;
  provider: string | null;
  error: string | null;
}

export interface Source {
  id: string;
  name: string;
  /** A `ProviderInfo.id`. */
  provider: string;
  /** Settings keyed by `Field.key`; empty fields are absent. */
  config: Record<string, string>;
}

/** A channel and the source it came from (Stalker needs the source to resolve its link). */
export interface LiveItem {
  id: string;
  source: Source;
  channel: Channel;
  /** The feeds of the same channel when duplicates are merged, best first (this item among them). */
  variants?: LiveItem[];
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
