import { ROW_HEIGHT } from './rowWindow.ts';

const DOM_DELTA_LINE = 1;
const DOM_DELTA_PAGE = 2;

export interface WheelLike {
  deltaX: number;
  deltaY: number;
  deltaMode: number;
  shiftKey: boolean;
  ctrlKey: boolean;
}

/** A wheel event in CSS pixels; a plain mouse wheel with Shift scrolls sideways, as browsers do. */
export function wheelPixels(e: WheelLike, pageHeight: number, lineHeight = ROW_HEIGHT): { x: number; y: number } {
  const unit = e.deltaMode === DOM_DELTA_LINE ? lineHeight : e.deltaMode === DOM_DELTA_PAGE ? pageHeight : 1;
  const x = e.deltaX * unit;
  const y = e.deltaY * unit;
  return e.shiftKey && x === 0 ? { x: y, y: 0 } : { x, y };
}

/**
 * Wheel distance waiting for the next frame. `take` hands out whole pixels and keeps the fraction,
 * so slow trackpad scrolls (deltas below 1 px) still add up.
 */
export function createWheelQueue() {
  let x = 0;
  let y = 0;
  return {
    add(delta: { x: number; y: number }) {
      x += delta.x;
      y += delta.y;
    },
    take(): { x: number; y: number } {
      const out = { x: Math.trunc(x), y: Math.trunc(y) };
      x -= out.x;
      y -= out.y;
      return out;
    },
    clear() {
      x = 0;
      y = 0;
    },
  };
}
