import { describe, expect, it } from 'vitest';
import { chapterAt, nextChapter, previousChapter } from './chapters';

const chapters = [
  { start: 0, title: 'Opening' },
  { start: 300, title: 'Heist' },
  { start: 900, title: 'Escape' },
];

describe('chapters', () => {
  it('finds the chapter playing', () => {
    expect(chapterAt(chapters, 0)).toBe(0);
    expect(chapterAt(chapters, 299.9)).toBe(0);
    expect(chapterAt(chapters, 300)).toBe(1);
    expect(chapterAt(chapters, 5000)).toBe(2);
    expect(chapterAt(chapters.slice(1), 10)).toBe(-1);
    expect(chapterAt([], 10)).toBe(-1);
  });

  it('goes back to the start of the chapter, or to the one before in its first seconds', () => {
    expect(previousChapter(chapters, 400)?.title).toBe('Heist');
    expect(previousChapter(chapters, 301)?.title).toBe('Opening');
    expect(previousChapter(chapters.slice(1), 100)).toBeUndefined();
  });

  it('goes to the next chapter, even from a seek that landed just short of a start', () => {
    expect(nextChapter(chapters, 10)?.title).toBe('Heist');
    expect(nextChapter(chapters, 299.8)?.title).toBe('Escape');
    expect(nextChapter(chapters, 950)).toBeUndefined();
    expect(nextChapter(chapters.slice(1), 10)?.title).toBe('Heist');
  });
});
