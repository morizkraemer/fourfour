import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

import {
  getAnalysisData,
  invalidateAnalysisCache,
  peekAnalysisData,
  prefetchAnalysisData,
} from './tauri.svelte.ts';

const payload = (id: number) => ({
  id,
  waveform_preview: Array.from({ length: 400 }, () => 0x7f),
  waveform_color: null,
});

function backend() {
  invokeMock.mockImplementation(async (cmd: string, args: { trackId: number }) => {
    if (cmd !== 'get_analysis_data') throw new Error(`unexpected ${cmd}`);
    return payload(args.trackId);
  });
}

const fetchedIds = () =>
  invokeMock.mock.calls
    .filter(([cmd]) => cmd === 'get_analysis_data')
    .map(([, args]) => args.trackId as number);

beforeEach(() => {
  invokeMock.mockReset();
  invalidateAnalysisCache();
  backend();
});

afterEach(() => {
  vi.useRealTimers();
});

describe('analysis cache', () => {
  it('serves the second read from cache', async () => {
    await getAnalysisData(1);
    await getAnalysisData(1);
    expect(fetchedIds()).toEqual([1]);
    expect(peekAnalysisData(1)?.id).toBe(1);
  });
  it('dedupes concurrent requests for one track', async () => {
    const [a, b] = await Promise.all([getAnalysisData(2), getAnalysisData(2)]);
    expect(a).toBe(b);
    expect(fetchedIds()).toEqual([2]);
  });
  it('force bypasses the cache', async () => {
    await getAnalysisData(3);
    await getAnalysisData(3, true);
    expect(fetchedIds()).toEqual([3, 3]);
  });
  it('evicts the least recently used entry beyond 48', async () => {
    for (let id = 1; id <= 48; id++) await getAnalysisData(id);
    peekAnalysisData(1);
    await getAnalysisData(49);
    expect(peekAnalysisData(1)).toBeDefined();
    expect(peekAnalysisData(2)).toBeUndefined();
    expect(peekAnalysisData(49)).toBeDefined();
  });
  it('rejects and does not cache an invalid backend payload', async () => {
    invokeMock.mockResolvedValue({ waveform_preview: [] });
    await expect(getAnalysisData(5)).rejects.toThrow('Invalid analysis payload');
    expect(peekAnalysisData(5)).toBeUndefined();
  });
  it('clears the inflight entry after a failure so a retry refetches', async () => {
    invokeMock.mockRejectedValueOnce(new Error('boom'));
    await expect(getAnalysisData(6)).rejects.toThrow('boom');
    await expect(getAnalysisData(6)).resolves.toMatchObject({ id: 6 });
  });
  it('invalidates one track or everything', async () => {
    await getAnalysisData(7);
    await getAnalysisData(8);
    invalidateAnalysisCache(7);
    expect(peekAnalysisData(7)).toBeUndefined();
    expect(peekAnalysisData(8)).toBeDefined();
    invalidateAnalysisCache();
    expect(peekAnalysisData(8)).toBeUndefined();
  });
});

describe('prefetchAnalysisData', () => {
  it('waits for its delay then fetches only uncached tracks', async () => {
    vi.useFakeTimers();
    await getAnalysisData(10);
    invokeMock.mockClear();
    prefetchAnalysisData([10, 11, 12]);
    expect(fetchedIds()).toEqual([]);
    await vi.advanceTimersByTimeAsync(500);
    expect(fetchedIds()).toEqual([11, 12]);
  });
  it('skips tracks that are already inflight', async () => {
    vi.useFakeTimers();
    const pending = getAnalysisData(20);
    invokeMock.mockClear();
    prefetchAnalysisData([20]);
    await vi.advanceTimersByTimeAsync(500);
    await pending;
    expect(fetchedIds()).toEqual([]);
  });
  it('continues past a failing track', async () => {
    vi.useFakeTimers();
    invokeMock.mockImplementation(async (_cmd: string, args: { trackId: number }) => {
      if (args.trackId === 30) throw new Error('nope');
      return payload(args.trackId);
    });
    prefetchAnalysisData([30, 31]);
    await vi.advanceTimersByTimeAsync(500);
    expect(peekAnalysisData(31)).toBeDefined();
  });
});
