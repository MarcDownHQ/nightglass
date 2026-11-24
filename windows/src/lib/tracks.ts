/** One entry of mpv's `track-list`. */
export interface Track {
  id: number;
  type: 'video' | 'audio' | 'sub';
  lang?: string;
  title?: string;
  codec?: string;
  selected: boolean;
  'demux-channel-count'?: number;
  'demux-h'?: number;
  'demux-fps'?: number;
  /** Cover art, not a video. */
  image?: boolean;
}

export function trackName(track: Track): string {
  const channels = track['demux-channel-count'] ?? 0;
  return [track.lang?.toUpperCase(), track.title, track.codec?.toUpperCase(), channels > 2 ? `${channels}ch` : undefined]
    .filter(Boolean)
    .join(' · ');
}

/** "1080p60 VP9": the track's title, else its height, its frame rate when above 30, and its codec. */
export function qualityLabel(track: Track): string {
  if (track.title) return track.title;
  const height = track['demux-h'];
  const fps = track['demux-fps'] ?? 0;
  return [height ? `${height}p${fps > 30 ? Math.round(fps) : ''}` : undefined, track.codec?.toUpperCase()].filter(Boolean).join(' ');
}

/** The video tracks to choose between, highest first. */
export function videoQualities(tracks: Track[]): Track[] {
  return tracks
    .filter((t) => t.type === 'video' && !t.image)
    .sort((a, b) => (b['demux-h'] ?? 0) - (a['demux-h'] ?? 0) || (b['demux-fps'] ?? 0) - (a['demux-fps'] ?? 0));
}
