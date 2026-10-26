import { describe, expect, it, vi } from 'vitest';
import { createGridPersister, shiftBeats, shiftToPlaceDownbeatAt, type BeatMarker } from './beatGrid.ts';

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
  it('rounds to whole milliseconds', () => {
    expect(shiftBeats(grid(), 2.6)[0].time_ms).toBe(103);
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
