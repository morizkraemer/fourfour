import { describe, expect, it } from 'vitest';
import { formatClock, isSeekKey, seekTargetMs } from './waveSeek.ts';

const BEAT = 500;
const beats = Array.from({ length: 100 }, (_, i) => ({ time_ms: 200 + i * BEAT, bar_position: (i % 4) + 1 }));
const ctx = (positionMs: number, grid = beats) => ({ positionMs, durationMs: 60000, beats: grid });

describe('seekTargetMs with a grid', () => {
  it('Right goes to the next beat, Left to the previous one', () => {
    expect(seekTargetMs('ArrowRight', false, ctx(1000))).toBe(1200);
    expect(seekTargetMs('ArrowLeft', false, ctx(1000))).toBe(700);
  });

  it('on a beat, Left goes to the beat before and Right to the beat after', () => {
    expect(seekTargetMs('ArrowRight', false, ctx(1200))).toBe(1700);
    expect(seekTargetMs('ArrowLeft', false, ctx(1200))).toBe(700);
  });

  it('Shift moves four bars, which is sixteen beats', () => {
    expect(seekTargetMs('ArrowRight', true, ctx(1200))).toBe(1200 + 16 * BEAT);
    expect(seekTargetMs('ArrowLeft', true, ctx(20200))).toBe(20200 - 16 * BEAT);
  });

  it('extrapolates past the grid ends and clamps to the track', () => {
    expect(seekTargetMs('ArrowLeft', false, ctx(300))).toBe(200);
    expect(seekTargetMs('ArrowLeft', false, ctx(100))).toBe(0);
    expect(seekTargetMs('ArrowRight', true, ctx(59000))).toBe(60000);
    expect(seekTargetMs('ArrowRight', false, ctx(55100))).toBe(55200);
    expect(seekTargetMs('ArrowLeft', false, ctx(55100))).toBe(54700);
    expect(seekTargetMs('ArrowLeft', true, ctx(2000))).toBe(0);
  });

  it('before a late first beat Left never moves forward', () => {
    const late = beats.map((b) => ({ ...b, time_ms: b.time_ms + 3800 }));
    expect(seekTargetMs('ArrowLeft', false, ctx(150, late))).toBe(0);
    expect(seekTargetMs('ArrowRight', false, ctx(150, late))).toBe(500);
  });

  it('before the first beat Right lands on the first beat', () => {
    expect(seekTargetMs('ArrowRight', false, ctx(50))).toBe(200);
  });
});

describe('seekTargetMs without a grid', () => {
  it('steps one second, Shift ten seconds', () => {
    expect(seekTargetMs('ArrowRight', false, ctx(5000, []))).toBe(6000);
    expect(seekTargetMs('ArrowLeft', false, ctx(5000, []))).toBe(4000);
    expect(seekTargetMs('ArrowRight', true, ctx(5000, []))).toBe(15000);
    expect(seekTargetMs('ArrowLeft', true, ctx(5000, []))).toBe(0);
  });

  it('a single beat is not a grid', () => {
    expect(seekTargetMs('ArrowRight', false, ctx(5000, [beats[0]]))).toBe(6000);
  });
});

describe('Home, End and helpers', () => {
  it('Home is the start and End the end', () => {
    expect(seekTargetMs('Home', false, ctx(5000))).toBe(0);
    expect(seekTargetMs('End', false, ctx(5000))).toBe(60000);
  });

  it('knows its keys and formats m:ss', () => {
    expect(isSeekKey('ArrowLeft')).toBe(true);
    expect(isSeekKey('ArrowUp')).toBe(false);
    expect(formatClock(83999)).toBe('1:23');
    expect(formatClock(-5)).toBe('0:00');
  });
});
