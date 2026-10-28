import { describe, expect, it, vi } from 'vitest';
import { BEAT_NUDGE_MS, createGridPersister, gridForStorage, gridFromStorage, gridNudgeMs, rescaleBeats, shiftBeats, shiftToPlaceDownbeatAt, type BeatMarker } from './beatGrid.ts';

const SAMPLE_MS = 1000 / 44100;

const grid = (): BeatMarker[] => [
  { time_ms: 100, bar_position: 1 },
  { time_ms: 600, bar_position: 2 },
  { time_ms: 1100, bar_position: 3 },
  { time_ms: 1600, bar_position: 4 },
  { time_ms: 2100, bar_position: 1 },
];

describe('shiftBeats', () => {
  it('moves every beat and keeps the bar positions', () => {
    const out = shiftBeats(grid(), 10);
    expect(out.map((b) => b.time_ms)).toEqual([110, 610, 1110, 1610, 2110]);
    expect(out.map((b) => b.bar_position)).toEqual([1, 2, 3, 4, 1]);
  });
  it('does not change its input', () => {
    const input = grid();
    shiftBeats(input, -50);
    expect(input[0].time_ms).toBe(100);
  });
  it('drops beats that would start before the track', () => {
    const out = shiftBeats(grid(), -150);
    expect(out[0]).toEqual({ time_ms: 450, bar_position: 2 });
    expect(out).toHaveLength(4);
  });
  it('moves by fractions of a millisecond exactly', () => {
    expect(shiftBeats(grid(), 2.6)[0].time_ms).toBeCloseTo(102.6, 9);
  });
});

describe('gridNudgeMs', () => {
  it('moves the grid by one pixel of the zoom view, exactly', () => {
    expect(gridNudgeMs(4.4, false)).toBe(4.4);
    expect(gridNudgeMs(0.37, false)).toBe(0.37);
  });
  it('never goes below one sample at deep zoom', () => {
    expect(gridNudgeMs(0.004, false)).toBeCloseTo(SAMPLE_MS, 9);
    expect(gridNudgeMs(0.004, false, 1000 / 48000)).toBeCloseTo(1000 / 48000, 9);
  });
  it('keeps the old step when zoomed far out or before layout', () => {
    expect(gridNudgeMs(250, false)).toBe(BEAT_NUDGE_MS);
    expect(gridNudgeMs(0, false)).toBe(BEAT_NUDGE_MS);
  });
  it('is one sample with the fine modifier at any zoom', () => {
    expect(gridNudgeMs(250, true)).toBeCloseTo(SAMPLE_MS, 9);
    expect(gridNudgeMs(4.4, true, 1000 / 48000)).toBeCloseTo(1000 / 48000, 9);
  });
});

describe('gridForStorage / gridFromStorage', () => {
  it('stores whole ms beats and the shared fraction, and gets the exact grid back', () => {
    const exact = shiftBeats(grid(), 12.37);
    const stored = gridForStorage(exact);
    expect(stored.beats.map((b) => b.time_ms)).toEqual([112, 612, 1112, 1612, 2112]);
    expect(stored.offsetMs).toBeCloseTo(0.37, 9);
    const back = gridFromStorage(stored.beats, stored.offsetMs);
    back.forEach((b, i) => expect(b.time_ms).toBeCloseTo(exact[i].time_ms, 9));
  });
  it('rounds each stored beat to its nearest millisecond, whichever way the fraction goes', () => {
    const stored = gridForStorage(shiftBeats(grid(), -0.8));
    expect(stored.beats[0].time_ms).toBe(99);
    expect(stored.offsetMs).toBeCloseTo(0.2, 9);
  });
  it('reads a grid without an offset as it is', () => {
    expect(gridFromStorage(grid(), undefined)).toEqual(grid());
    expect(gridFromStorage(grid(), null)).toEqual(grid());
  });
});

describe('shiftToPlaceDownbeatAt', () => {
  it('puts the nearest downbeat on the playhead', () => {
    expect(shiftToPlaceDownbeatAt(grid(), 2180)).toBe(80);
    expect(shiftToPlaceDownbeatAt(grid(), 20)).toBe(-80);
  });
  it('lands exactly after applying the shift', () => {
    const delta = shiftToPlaceDownbeatAt(grid(), 1999)!;
    const moved = shiftBeats(grid(), delta);
    expect(moved.find((b) => b.time_ms === 1999)?.bar_position).toBe(1);
  });
  it('places a downbeat exactly at a playhead between two milliseconds', () => {
    const delta = shiftToPlaceDownbeatAt(grid(), 2100.6137)!;
    expect(shiftBeats(grid(), delta)[4].time_ms).toBeCloseTo(2100.6137, 9);
  });
  it('uses plain beats when the grid has no downbeat', () => {
    expect(shiftToPlaceDownbeatAt([{ time_ms: 500, bar_position: 2 }], 530)).toBe(30);
  });
  it('has nothing to do for an empty grid or a bad position', () => {
    expect(shiftToPlaceDownbeatAt([], 1000)).toBeNull();
    expect(shiftToPlaceDownbeatAt(grid(), NaN)).toBeNull();
  });
});

function fakeTimers() {
  let next = 0;
  const pending = new Map<number, () => void>();
  return {
    timers: {
      set: (fn: () => void) => {
        pending.set(++next, fn);
        return next;
      },
      clear: (handle: unknown) => void pending.delete(handle as number),
    },
    fire() {
      const [first] = [...pending.keys()];
      const fn = pending.get(first);
      pending.delete(first);
      fn?.();
    },
    count: () => pending.size,
  };
}

describe('createGridPersister', () => {
  it('writes only the newest grid of a burst of edits', async () => {
    const save = vi.fn().mockResolvedValue(undefined);
    const clock = fakeTimers();
    const persister = createGridPersister(save, vi.fn(), 400, clock.timers);
    persister.schedule(7, shiftBeats(grid(), 10));
    persister.schedule(7, shiftBeats(grid(), 20));
    persister.schedule(7, shiftBeats(grid(), 30));
    expect(clock.count()).toBe(1);
    expect(save).not.toHaveBeenCalled();
    clock.fire();
    await persister.flush();
    expect(save).toHaveBeenCalledTimes(1);
    expect(save.mock.calls[0][0]).toBe(7);
    expect(save.mock.calls[0][1][0].time_ms).toBe(130);
  });

  it('flush writes at once and leaves nothing for the timer', async () => {
    const save = vi.fn().mockResolvedValue(undefined);
    const clock = fakeTimers();
    const persister = createGridPersister(save, vi.fn(), 400, clock.timers);
    persister.schedule(3, grid());
    await persister.flush();
    expect(save).toHaveBeenCalledTimes(1);
    expect(clock.count()).toBe(0);
    await persister.flush();
    expect(save).toHaveBeenCalledTimes(1);
  });

  it('never runs two writes at the same time', async () => {
    const order: string[] = [];
    let release: () => void = () => {};
    const save = vi.fn((trackId: number) => {
      order.push(`start ${trackId}`);
      return new Promise<void>((resolve) => {
        release = () => {
          order.push(`end ${trackId}`);
          resolve();
        };
      });
    });
    const persister = createGridPersister(save, vi.fn(), 400, fakeTimers().timers);
    persister.schedule(1, grid());
    const first = persister.flush();
    persister.schedule(2, grid());
    const second = persister.flush();
    await Promise.resolve();
    expect(order).toEqual(['start 1']);
    release();
    await first;
    await Promise.resolve();
    await Promise.resolve();
    release();
    await second;
    expect(order).toEqual(['start 1', 'end 1', 'start 2', 'end 2']);
  });

  it('writes the waiting edit of one track when another track is edited', async () => {
    const save = vi.fn().mockResolvedValue(undefined);
    const clock = fakeTimers();
    const persister = createGridPersister(save, vi.fn(), 400, clock.timers);
    persister.schedule(1, grid());
    persister.schedule(2, grid());
    await persister.flush();
    expect(save.mock.calls.map((c) => c[0])).toEqual([1, 2]);
  });

  it('reports a failed write and keeps saving later edits', async () => {
    const failure = new Error('disk full');
    const save = vi.fn().mockRejectedValueOnce(failure).mockResolvedValue(undefined);
    const onError = vi.fn();
    const persister = createGridPersister(save, onError, 400, fakeTimers().timers);
    persister.schedule(1, grid());
    await persister.flush();
    expect(onError).toHaveBeenCalledWith(failure);
    persister.schedule(1, grid());
    await persister.flush();
    expect(save).toHaveBeenCalledTimes(2);
  });
});

describe('rescaleBeats (mirrors the library rescale_beat_grid)', () => {
  const TRACK_MS = 300_000;
  const bars = [
    { time_ms: 0, bar_position: 1 },
    { time_ms: 469, bar_position: 2 },
    { time_ms: 938, bar_position: 3 },
    { time_ms: 1406, bar_position: 4 },
  ];

  it('64 BPM from a downbeat at 0 gives the library result', () => {
    const out = rescaleBeats(bars, 64, TRACK_MS);
    expect(out.slice(0, 4).map((b) => b.time_ms)).toEqual([0, 938, 1875, 2813]);
    expect(out).toHaveLength(321);
  });

  it('128 BPM spaces beats 468.75 ms with the bar positions cycling', () => {
    const out = rescaleBeats(bars, 128, TRACK_MS);
    expect(out.slice(0, 5).map((b) => b.time_ms)).toEqual([0, 469, 938, 1406, 1875]);
    expect(out.slice(0, 5).map((b) => b.bar_position)).toEqual([1, 2, 3, 4, 1]);
  });

  it('keeps the downbeat and its sub-millisecond place, filling back to the track start', () => {
    const moved = bars.map((b) => ({ time_ms: b.time_ms + 1000 - 0.37, bar_position: b.bar_position }));
    const out = rescaleBeats(moved, 100, TRACK_MS);
    expect(out[0].time_ms).toBeCloseTo(400 - 0.37, 9);
    expect(out[0].bar_position).toBe(4);
    const anchor = out.find((b) => Math.abs(b.time_ms - (1000 - 0.37)) < 1e-9);
    expect(anchor?.bar_position).toBe(1);
    expect(out[1].time_ms - out[0].time_ms).toBeCloseTo(600, 9);
  });

  it('is what the library stores: saving then loading the grid changes nothing', () => {
    const moved = bars.map((b) => ({ time_ms: b.time_ms + 1000 - 0.37, bar_position: b.bar_position }));
    const local = rescaleBeats(moved, 127.35, TRACK_MS);
    const { beats, offsetMs } = gridForStorage(local);
    const reloaded = gridFromStorage(beats, offsetMs);
    expect(reloaded.map((b) => b.bar_position)).toEqual(local.map((b) => b.bar_position));
    reloaded.forEach((b, i) => expect(b.time_ms).toBeCloseTo(local[i].time_ms, 9));
  });
});
