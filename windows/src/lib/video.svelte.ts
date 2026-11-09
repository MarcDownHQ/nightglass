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
   * `rect`: the part in view, which the backdrop lets through. `full`: the whole element, where the video window
   * goes; past the edges of what scrolls it, the backdrop covers it.
   */
  set(owner: HTMLElement | null, rect: HoleRect | null, full: HoleRect | null = rect) {
    this.owner = owner;
    this.rect = rect;
    if (!full) return;
    const k = devicePixelRatio;
    const place = [full.x, full.y, full.w, full.h].map((v) => Math.round(v * k));
    const key = place.join();
    if (key === this.sent) return;
    this.sent = key;
    void invoke('video_place', { rect: place });
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

/** The `update` of each element showing the video: what a scroll moved must follow in the same frame. */
const followers = new Set<() => void>();

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
    hole.set(node, { x: r.left, y: top, w: r.width, h, radius, square }, { x: r.left, y: r.top, w: r.width, h: r.height, radius });
  };
  followers.add(update);
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
      followers.delete(update);
      hole.release(node);
    },
  };
}

/**
 * Action: the wheel scrolls this element from script, and the video window follows in the same frame. Scrolled by
 * the browser, the page moves at once and the scroll event, so the video, comes a frame or two after.
 */
export function wheelScroll(node: HTMLElement) {
  let target = 0;
  let at = 0;
  let frame = 0;
  const step = () => {
    at = Math.abs(target - at) < 0.5 ? target : at + (target - at) * 0.3;
    node.scrollTop = at;
    for (const follow of followers) follow();
    frame = at === target ? 0 : requestAnimationFrame(step);
  };
  const onwheel = (e: WheelEvent) => {
    // The preview's own wheel (volume), and zoom, stay as they are.
    if (e.defaultPrevented || e.ctrlKey) return;
    e.preventDefault();
    if (!frame) target = at = node.scrollTop;
    const max = node.scrollHeight - node.clientHeight;
    target = Math.max(0, Math.min(max, target + e.deltaY * (e.deltaMode === 1 ? 40 : 1)));
    if (!frame) frame = requestAnimationFrame(step);
  };
  node.addEventListener('wheel', onwheel, { passive: false });
  return {
    destroy() {
      node.removeEventListener('wheel', onwheel);
      cancelAnimationFrame(frame);
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
