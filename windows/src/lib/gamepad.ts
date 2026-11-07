/**
 * A gamepad (Xbox layout, the browser's "standard" mapping) drives the app through the keys every
 * screen already handles: the D-pad is the arrows, A is Enter, B is Escape…
 */
type Press = { key: string; ctrlKey?: boolean };

/** Standard mapping button index → key. */
export const BUTTONS: Record<number, Press> = {
  0: { key: 'Enter' }, // A
  1: { key: 'Escape' }, // B
  2: { key: ' ' }, // X: pause
  3: { key: 'f' }, // Y: fullscreen and back (in Live, Enter expands)
  4: { key: 'ArrowLeft' }, // LB: previous tab / −10 s
  5: { key: 'ArrowRight' }, // RB: next tab / +10 s
  6: { key: '-' }, // LT: volume down
  7: { key: '+' }, // RT: volume up
  8: { key: 'm' }, // View: mute
  9: { key: 'k', ctrlKey: true }, // Menu: search
  12: { key: 'ArrowUp' },
  13: { key: 'ArrowDown' },
  14: { key: 'ArrowLeft' },
  15: { key: 'ArrowRight' },
};

/** Held arrows repeat, like a keyboard. */
const REPEAT_DELAY = 400;
const REPEAT_EVERY = 110;
const REPEATS = new Set(['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight']);

export function press({ key, ctrlKey = false }: Press) {
  // Focused buttons (the D-pad moved focus) take Enter as a click.
  const target = key === 'Enter' && document.activeElement instanceof HTMLButtonElement ? document.activeElement : window;
  target.dispatchEvent(new KeyboardEvent('keydown', { key, ctrlKey, bubbles: true, cancelable: true }));
}

/** Polls the pads while one is connected; returns the stop function. */
export function startGamepad(): () => void {
  const held = new Map<number, number>();
  let frame = 0;
  const poll = () => {
    const now = performance.now();
    for (const pad of navigator.getGamepads?.() ?? []) {
      if (!pad || pad.mapping !== 'standard') continue;
      // The left stick works as a D-pad.
      const [x, y] = pad.axes;
      const stick: Record<number, boolean> = { 12: y < -0.6, 13: y > 0.6, 14: x < -0.6, 15: x > 0.6 };
      for (const [i, mapping] of Object.entries(BUTTONS)) {
        const index = Number(i);
        const down = pad.buttons[index]?.pressed || stick[index] || false;
        const id = pad.index * 100 + index;
        const since = held.get(id);
        if (!down) {
          held.delete(id);
        } else if (since === undefined) {
          held.set(id, now);
          press(mapping);
        } else if (REPEATS.has(mapping.key) && now - since > REPEAT_DELAY) {
          held.set(id, now - REPEAT_DELAY + REPEAT_EVERY);
          press(mapping);
        }
      }
    }
    frame = requestAnimationFrame(poll);
  };
  const connected = () => {
    if (!frame) frame = requestAnimationFrame(poll);
  };
  addEventListener('gamepadconnected', connected);
  // A pad already plugged in before the page loaded announces itself on its first button press.
  if ((navigator.getGamepads?.() ?? []).some(Boolean)) connected();
  return () => {
    removeEventListener('gamepadconnected', connected);
    cancelAnimationFrame(frame);
  };
}
