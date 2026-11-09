import { invoke } from '@tauri-apps/api/core';

export interface HoleRect {
  x: number;
  y: number;
  w: number;
  h: number;
  radius: number;
  /** The side a scroller cuts: a straight edge there, like the page content it scrolls with. */
  square?: 'top' | 'bottom';
}

/** The part of the window where the page lets the video through. */
class VideoHole {
  rect = $state<HoleRect | null>(null);
  private owner: HTMLElement | null = null;
  private sent = '';

  /**
   * `rect` is the part in view; `cut`, the element's whole height when its scroller cut it, and on which side.
   * The video then keeps its full size and slides out of view, instead of shrinking into what is left.
   */
  set(owner: HTMLElement | null, rect: HoleRect | null, cut?: { full: number; top: boolean }) {
    this.owner = owner;
    this.rect = rect;
    if (!rect || rect.h < 1) return;
    // mpv places the video with margins relative to its window, which is the whole client area.
    const W = innerWidth;
    const H = innerHeight;
    const margins = [rect.x / W, rect.y / H, (W - rect.x - rect.w) / W, (H - rect.y - rect.h) / H];
    // mpv fits the video in the part in view: zoomed back to the whole element's size, aligned on the edge still in view.
    // shortcut: assumes the video is no wider than the element (16:9 TV in a 16:9 preview), else it zooms too far.
    const zoom = cut && rect.h < cut.full ? Math.log2(cut.full / rect.h) : 0;
    const align = zoom ? (cut!.top ? 1 : -1) : 0;
    const key = [...margins, zoom, align].map((m) => m.toFixed(4)).join();
    if (key === this.sent) return;
    this.sent = key;
    void invoke('mpv_video_rect', { margins });
    void invoke('mpv_command', { args: ['set', 'video-zoom', String(zoom)] });
    void invoke('mpv_command', { args: ['set', 'video-align-y', String(align)] });
  }

  /** The video's window moved (PiP and back): send the place again, even if unchanged. */
  forget() {
    this.sent = '';
  }

  release(owner: HTMLElement) {
    if (this.owner === owner) this.set(null, null);
  }
}

export const hole = new VideoHole();

/** Action: the element becomes the window onto the video. */
export function videoHole(node: HTMLElement, radius = 0) {
  let last = '';
  // The video must not show past the edges of the area that scrolls it (under the title bar).
  let scroller = node.parentElement;
  while (scroller && !/auto|scroll/.test(getComputedStyle(scroller).overflowY)) scroller = scroller.parentElement;
  const update = () => {
    const r = node.getBoundingClientRect();
    const box = scroller?.getBoundingClientRect();
    const top = Math.max(r.top, box?.top ?? -Infinity);
    const bottom = Math.min(r.bottom, box?.bottom ?? Infinity);
    // Scrolling anywhere calls this: only a real move goes further.
    const key = [r.left, r.top, r.width, r.height, top, bottom].join();
    if (key === last && hole.rect) return;
    last = key;
    const h = Math.max(0, bottom - top);
    const square = top > r.top ? 'top' : bottom < r.bottom ? 'bottom' : undefined;
    hole.set(node, { x: r.left, y: top, w: r.width, h, radius, square }, square ? { full: r.height, top: square === 'top' } : undefined);
  };
  const observer = new ResizeObserver(update);
  observer.observe(node);
  addEventListener('resize', update);
  // Capture: a scrolling page (Home) moves the element without resizing it.
  addEventListener('scroll', update, true);
  update();
  return {
    destroy() {
      observer.disconnect();
      removeEventListener('resize', update);
      removeEventListener('scroll', update, true);
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

function roundedRect({ x, y, w, h, radius, square }: HoleRect): string {
  const r = Math.min(radius, w / 2, h / 2);
  const t = square === 'top' ? 0 : r;
  const b = square === 'bottom' ? 0 : r;
  const a = (r: number, dx: number, dy: number) => `A${r} ${r} 0 0 1 ${dx} ${dy}`;
  return `M${x + t} ${y}H${x + w - t}${a(t, x + w, y + t)}V${y + h - b}${a(b, x + w - b, y + h)}H${x + b}${a(b, x, y + h - b)}V${y + t}${a(t, x + t, y)}Z`;
}
