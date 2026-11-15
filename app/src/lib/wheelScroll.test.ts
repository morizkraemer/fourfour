import { describe, expect, it } from 'vitest';
import { createWheelQueue, wheelPixels } from './wheelScroll.ts';

const wheel = (deltaY: number, more: Partial<{ deltaX: number; deltaMode: number; shiftKey: boolean }> = {}) => ({
  deltaX: 0,
  deltaY,
  deltaMode: 0,
  shiftKey: false,
  ctrlKey: false,
  ...more,
});

describe('wheelPixels', () => {
  it('passes pixel deltas through', () => {
    expect(wheelPixels(wheel(37.5, { deltaX: -4 }), 800)).toEqual({ x: -4, y: 37.5 });
  });

  it('turns lines into rows and pages into the page height', () => {
    expect(wheelPixels(wheel(3, { deltaMode: 1 }), 800)).toEqual({ x: 0, y: 84 });
    expect(wheelPixels(wheel(-1, { deltaMode: 2 }), 800)).toEqual({ x: 0, y: -800 });
  });

  it('scrolls sideways for Shift with a plain wheel, but keeps a real horizontal delta', () => {
    expect(wheelPixels(wheel(100, { shiftKey: true }), 800)).toEqual({ x: 100, y: 0 });
    expect(wheelPixels(wheel(5, { shiftKey: true, deltaX: 20 }), 800)).toEqual({ x: 20, y: 5 });
  });
});

describe('createWheelQueue', () => {
  it('hands out whole pixels and keeps the fraction for the next frame', () => {
    const queue = createWheelQueue();
    queue.add({ x: 0, y: 0.6 });
    expect(queue.take()).toEqual({ x: 0, y: 0 });
    queue.add({ x: 0, y: 0.6 });
    expect(queue.take()).toEqual({ x: 0, y: 1 });
    queue.add({ x: -2.5, y: -3.4 });
    const out = queue.take();
    expect(out.x).toBe(-2);
    expect(out.y).toBe(-3);
  });

  it('adds up every event of a frame and starts empty after clear', () => {
    const queue = createWheelQueue();
    queue.add({ x: 0, y: 40 });
    queue.add({ x: 0, y: 55 });
    expect(queue.take()).toEqual({ x: 0, y: 95 });
    queue.add({ x: 3, y: 3 });
    queue.clear();
    expect(queue.take()).toEqual({ x: 0, y: 0 });
  });
});
