import { describe, expect, it } from 'vitest';
import { DEFAULT_QUANTIZE_DIVISION, nearestGridStepMs, quantizedMs } from './quantize.ts';
import type { BeatMarker } from './beatGrid.ts';

const grid = (times: number[]): BeatMarker[] => times.map((time_ms, i) => ({ time_ms, bar_position: (i % 4) + 1 }));
const beats = grid([0, 500, 1000, 1500, 2000]);

describe('nearestGridStepMs', () => {
  it('snaps to sixteenths by default', () => {
    expect(DEFAULT_QUANTIZE_DIVISION).toBe(16);
    expect(nearestGridStepMs(beats, 740)).toBe(750);
    expect(nearestGridStepMs(beats, 860)).toBe(875);
    expect(nearestGridStepMs(beats, 990)).toBe(1000);
    expect(nearestGridStepMs(beats, 1060)).toBe(1000);
  });

  it('snaps to eighths or beats with a coarser division', () => {
    expect(nearestGridStepMs(beats, 740, 8)).toBe(750);
    expect(nearestGridStepMs(beats, 860, 8)).toBe(750);
    expect(nearestGridStepMs(beats, 880, 8)).toBe(1000);
    expect(nearestGridStepMs(beats, 740, 4)).toBe(500);
    expect(nearestGridStepMs(beats, 760, 4)).toBe(1000);
  });

  it('rounds ties to the earlier step', () => {
    expect(nearestGridStepMs(beats, 562.5)).toBe(500);
    expect(nearestGridStepMs(beats, 750, 4)).toBe(500);
  });

  it('splits each beat on its own when the tempo changes', () => {
    const changing = grid([1000, 1500, 2000, 2400, 2800, 3200]);
    expect(nearestGridStepMs(changing, 1620)).toBe(1625);
    expect(nearestGridStepMs(changing, 2090)).toBe(2100);
    expect(nearestGridStepMs(changing, 2160)).toBe(2200);
    expect(nearestGridStepMs(changing, 2290)).toBe(2300);
    expect(nearestGridStepMs(changing, 1990)).toBe(2000);
    expect(nearestGridStepMs(changing, 2010)).toBe(2000);
  });

  it('follows the first beat offset, and repeats the first span before it', () => {
    const offset = grid([123.4, 623.4, 1123.4, 1623.4]);
    expect(nearestGridStepMs(offset, 250)).toBeCloseTo(248.4, 6);
    expect(nearestGridStepMs(offset, 130)).toBeCloseTo(123.4, 6);
    expect(nearestGridStepMs(offset, 1700)).toBeCloseTo(1748.4, 6);
  });

  it('never lands before the start of the track', () => {
    expect(nearestGridStepMs(grid([50, 550, 1050]), 0)).toBe(50);
    expect(nearestGridStepMs(grid([50, 550, 1050]), 10)).toBe(50);
    expect(nearestGridStepMs(grid([125, 625]), 3)).toBe(0);
    expect(nearestGridStepMs(grid([200, 700]), 30)).toBe(75);
  });

  it('repeats the last span after the last beat', () => {
    expect(nearestGridStepMs(beats, 2140)).toBe(2125);
    expect(nearestGridStepMs(beats, 9010)).toBe(9000);
    expect(nearestGridStepMs(grid([0, 500, 1000, 1400]), 1590)).toBe(1600);
  });

  it('snaps to the only beat of a one-beat grid and leaves a missing grid alone', () => {
    expect(nearestGridStepMs(grid([1000]), 1400)).toBe(1000);
    expect(nearestGridStepMs([], 123)).toBe(123);
  });
});

describe('quantizedMs', () => {
  it('only snaps when quantize is on, to whole milliseconds', () => {
    expect(quantizedMs(beats, 740, false)).toBe(740);
    expect(quantizedMs(beats, 740, true)).toBe(750);
    expect(quantizedMs(beats, 740, true, 4)).toBe(500);
    expect(quantizedMs(grid([123.4, 623.4]), 250, true)).toBe(248);
  });

  it('leaves the position alone without a grid', () => {
    expect(quantizedMs([], 740, true)).toBe(740);
  });
});
