import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createDetailPrefetcher } from './detail-prefetch.ts';

describe('createDetailPrefetcher', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  function deferredWarm() {
    const calls: number[] = [];
    const finish: (() => void)[] = [];
    const warm = (id: number) => {
      calls.push(id);
      return new Promise<void>((done) => finish.push(done));
    };
    return { calls, finish, warm };
  }

  it('waits for the selection to settle and warms only the newest request', async () => {
    const { calls, warm } = deferredWarm();
    const prefetcher = createDetailPrefetcher(warm, 150);
    prefetcher.request([1, 2]);
    await vi.advanceTimersByTimeAsync(100);
    prefetcher.request([3, 4]);
    await vi.advanceTimersByTimeAsync(149);
    expect(calls).toEqual([]);
    await vi.advanceTimersByTimeAsync(1);
    expect(calls).toEqual([3]);
  });

  it('measures one track at a time, lets a running one finish and skips tracks already warmed', async () => {
    const { calls, finish, warm } = deferredWarm();
    const prefetcher = createDetailPrefetcher(warm, 150);
    prefetcher.request([1, 2, 3]);
    await vi.advanceTimersByTimeAsync(150);
    prefetcher.request([1, 5]);
    await vi.advanceTimersByTimeAsync(150);
    expect(calls).toEqual([1]);
    finish[0]();
    await vi.advanceTimersByTimeAsync(0);
    expect(calls).toEqual([1, 5]);
    finish[1]();
    await vi.advanceTimersByTimeAsync(0);
    prefetcher.request([1, 5]);
    await vi.advanceTimersByTimeAsync(150);
    expect(calls).toEqual([1, 5]);
  });

  it('warms a pressed row at once and urgent, beside a running background warm, and only once', async () => {
    const calls: [number, boolean][] = [];
    const finish: (() => void)[] = [];
    const prefetcher = createDetailPrefetcher((id, urgent) => {
      calls.push([id, urgent]);
      return new Promise<void>((done) => finish.push(done));
    }, 150);
    prefetcher.now(7);
    expect(calls).toEqual([[7, true]]);
    prefetcher.now(7);
    prefetcher.request([7, 8]);
    await vi.advanceTimersByTimeAsync(150);
    expect(calls).toEqual([[7, true], [8, false]]);
    prefetcher.now(8);
    prefetcher.now(9);
    expect(calls).toEqual([[7, true], [8, false], [8, true], [9, true]]);
    finish.forEach((done) => done());
    await vi.advanceTimersByTimeAsync(0);
    prefetcher.now(7);
    prefetcher.now(9);
    prefetcher.request([7, 8, 9]);
    await vi.advanceTimersByTimeAsync(150);
    expect(calls).toHaveLength(4);
  });

  it('goes on with the next track when one fails', async () => {
    const calls: number[] = [];
    const prefetcher = createDetailPrefetcher(async (id) => {
      calls.push(id);
      if (id === 1) throw new Error('no audio');
    }, 150);
    prefetcher.request([1, 2]);
    await vi.advanceTimersByTimeAsync(150);
    expect(calls).toEqual([1, 2]);
  });
});
