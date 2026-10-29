import { describe, expect, it } from 'vitest';
import { nearestBeatMs, quantizedMs } from './quantize.ts';

const beats = [0, 500, 1000, 1500, 2000].map((time_ms, i) => ({ time_ms, bar_position: (i % 4) + 1 }));

describe('nearestBeatMs', () => {
  it('snaps to the closest beat', () => {
    expect(nearestBeatMs(beats, 740)).toBe(500);
    expect(nearestBeatMs(beats, 760)).toBe(1000);
    expect(nearestBeatMs(beats, 1990)).toBe(2000);
  });
  it('clamps before the first and after the last beat', () => {
    expect(nearestBeatMs(beats, -300)).toBe(0);
    expect(nearestBeatMs(beats, 9000)).toBe(2000);
  });
  it('rounds ties down to the earlier beat', () => {
    expect(nearestBeatMs(beats, 750)).toBe(500);
  });
  it('leaves the position alone without a grid', () => {
    expect(nearestBeatMs([], 123)).toBe(123);
  });
});

describe('quantizedMs', () => {
  it('only snaps when quantize is on, to whole milliseconds', () => {
    expect(quantizedMs(beats, 740, false)).toBe(740);
    expect(quantizedMs(beats, 740, true)).toBe(500);
    expect(quantizedMs([{ time_ms: 1000.4, bar_position: 1 }], 900, true)).toBe(1000);
  });
  it('leaves the position alone without a grid', () => {
    expect(quantizedMs([], 740, true)).toBe(740);
  });
});
