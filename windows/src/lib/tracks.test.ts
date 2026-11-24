import { describe, expect, it } from 'vitest';
import { qualityLabel, videoQualities, type Track } from './tracks';

const video = (id: number, extra: Partial<Track>): Track => ({ id, type: 'video', selected: false, ...extra });

describe('quality', () => {
  it('labels a video track by its title, else its height, frame rate above 30 and codec', () => {
    expect(qualityLabel(video(1, { title: 'Full HD', 'demux-h': 1080, codec: 'vp9' }))).toBe('Full HD');
    expect(qualityLabel(video(1, { 'demux-h': 1080, 'demux-fps': 59.94, codec: 'vp9' }))).toBe('1080p60 VP9');
    expect(qualityLabel(video(1, { 'demux-h': 720, 'demux-fps': 29.97, codec: 'h264' }))).toBe('720p H264');
    expect(qualityLabel(video(1, { 'demux-h': 480 }))).toBe('480p');
    expect(qualityLabel(video(1, { codec: 'av1' }))).toBe('AV1');
    expect(qualityLabel(video(1, {}))).toBe('');
  });

  it('lists the video tracks highest first, without cover art', () => {
    const tracks: Track[] = [
      video(1, { 'demux-h': 720 }),
      { id: 1, type: 'audio', selected: true },
      video(2, { 'demux-h': 1080, 'demux-fps': 30 }),
      video(3, { 'demux-h': 1080, 'demux-fps': 60 }),
      video(4, { image: true }),
      video(5, {}),
    ];
    expect(videoQualities(tracks).map((t) => t.id)).toEqual([3, 2, 1, 5]);
  });
});
