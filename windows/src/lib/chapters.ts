import type { Chapter } from './models';

/** The chapter playing at `time` in `chapters` sorted by start: its index, -1 before the first. */
export function chapterAt(chapters: Chapter[], time: number): number {
  let i = -1;
  while (i + 1 < chapters.length && chapters[i + 1].start <= time) i++;
  return i;
}

/** Back to the start of the chapter playing, or to the one before in its first seconds. */
export function previousChapter(chapters: Chapter[], time: number): Chapter | undefined {
  return chapters[chapterAt(chapters, time - 3)];
}

/** The chapter after the one playing; a seek that landed just short of a start counts as there. */
export function nextChapter(chapters: Chapter[], time: number): Chapter | undefined {
  return chapters[chapterAt(chapters, time + 0.5) + 1];
}
