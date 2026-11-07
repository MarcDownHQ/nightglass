import { invoke } from '@tauri-apps/api/core';

export interface HoleRect {
  x: number;
  y: number;
  w: number;
  h: number;
  radius: number;
}

/** The part of the window where the page lets the video through. */
class VideoHole {
  rect = $state<HoleRect | null>(null);
  private owner: HTMLElement | null = null;
  private sent = '';

  set(owner: HTMLElement | null, rect: HoleRect | null) {
    this.owner = owner;
    this.rect = rect;
    if (!rect) return;
    // mpv places the video with margins relative to its window, which is the whole client area.
    const W = innerWidth;
    const H = innerHeight;
    const margins = [rect.x / W, rect.y / H, (W - rect.x - rect.w) / W, (H - rect.y - rect.h) / H];
    const key = margins.map((m) => m.toFixed(4)).join();
    if (key === this.sent) return;
    this.sent = key;
    void invoke('mpv_video_rect', { margins });
  }

  release(owner: HTMLElement) {
    if (this.owner === owner) this.set(null, null);
  }
}

export const hole = new VideoHole();

/** Action: the element becomes the window onto the video. */
export function videoHole(node: HTMLElement, radius = 0) {
  const update = () => {
    const r = node.getBoundingClientRect();
    hole.set(node, { x: r.left, y: r.top, w: r.width, h: r.height, radius });
  };
  const observer = new ResizeObserver(update);
  observer.observe(node);
  addEventListener('resize', update);
  update();
  return {
    destroy() {
      observer.disconnect();
      removeEventListener('resize', update);
      hole.release(node);
    },
  };
}

/** CSS clip-path for the backdrop: the whole window minus the (rounded) holes: the player's, or the multi-view tiles'. */
export function backdropClip(W: number, H: number, ...rects: (HoleRect | null)[]): string {
  const holes = rects.filter((r): r is HoleRect => !!r);
  if (!holes.length) return 'none';
  return `path(evenodd, "M0 0H${W}V${H}H0Z ${holes.map(roundedRect).join(' ')}")`;
}

function roundedRect({ x, y, w, h, radius }: HoleRect): string {
  const r = Math.min(radius, w / 2, h / 2);
  const a = (dx: number, dy: number) => `A${r} ${r} 0 0 1 ${dx} ${dy}`;
  return `M${x + r} ${y}H${x + w - r}${a(x + w, y + r)}V${y + h - r}${a(x + w - r, y + h)}H${x + r}${a(x, y + h - r)}V${y + r}${a(x + r, y)}Z`;
}
