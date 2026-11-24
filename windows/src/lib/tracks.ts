/** One entry of mpv's `track-list`. */
export interface Track {
  id: number;
  type: 'video' | 'audio' | 'sub';
  lang?: string;
  title?: string;
  codec?: string;
  selected: boolean;
  'demux-channel-count'?: number;
}

export function trackName(track: Track): string {
  const channels = track['demux-channel-count'] ?? 0;
  return [track.lang?.toUpperCase(), track.title, track.codec?.toUpperCase(), channels > 2 ? `${channels}ch` : undefined]
    .filter(Boolean)
    .join(' · ');
}
