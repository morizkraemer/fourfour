import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const engine = vi.hoisted(() => ({ played: [] as number[], positions: [] as number[] }));

vi.mock('../services/playback.svelte.ts', () => ({
  playbackPause: vi.fn(async () => 0),
  playbackPlay: vi.fn(async (_path: string, ms: number) => {
    engine.played.push(ms);
  }),
  playbackSeek: vi.fn(),
  playbackSetPosition: vi.fn(async (ms: number) => {
    engine.positions.push(ms);
  }),
  playbackStop: vi.fn(async () => undefined),
  subscribePlaybackError: vi.fn(async () => () => {}),
  subscribePlaybackTick: vi.fn(async () => () => {}),
}));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(async () => undefined) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

import { invoke } from '@tauri-apps/api/core';
import { invalidateAnalysisCache } from '../services/tauri.svelte.ts';
import { cuePreviewStart, cuePreviewStop, loadPlayerTrack, player, resetPlayerTransport } from './player.svelte.ts';
import { addHotCueAtPlayhead, moveHotCue } from './cue-actions.ts';
import { cuesOf, flushCueEdits, loadCuesFromPayload } from './cues.svelte.ts';
import { QUANTIZE_STORAGE_KEY, quantize, setQuantize } from './quantize.svelte.ts';

const invokeMock = vi.mocked(invoke);

const trackInfo = {
  id: 7,
  source_path: '/music/seven.wav',
  title: 'Seven',
  artist: '',
  tempo: 12000,
  key: '1A',
  duration_secs: 20,
  has_analysis: true,
};

const savedCues = () => invokeMock.mock.calls.filter(([command]) => command === 'set_cues');

beforeEach(async () => {
  vi.useFakeTimers();
  const storage = new Map<string, string>();
  vi.stubGlobal('localStorage', {
    getItem: (key: string) => storage.get(key) ?? null,
    setItem: (key: string, value: string) => void storage.set(key, value),
  });
  invalidateAnalysisCache();
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (command: string): Promise<any> => {
    if (command === 'get_analysis_data') {
      return {
        waveform_preview: Array.from({ length: 400 }, (_, i) => 1 + (i % 30)),
        bpm: 120,
        key: '1A',
        beats: [1, 2, 3, 4, 5].map((n) => ({ time_ms: 500 * n, bar_position: ((n - 1) % 4) + 1 })),
        duration_ms: 20_000,
      };
    }
    return undefined;
  });
  engine.played.length = 0;
  engine.positions.length = 0;
  setQuantize(false);
  await loadPlayerTrack(trackInfo);
  await vi.advanceTimersByTimeAsync(0);
  loadCuesFromPayload(7, []);
  invokeMock.mockClear();
});

afterEach(() => {
  resetPlayerTransport();
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe('quantize', () => {
  it('is remembered across sessions', () => {
    setQuantize(true);
    expect(quantize.on).toBe(true);
    expect(localStorage.getItem(QUANTIZE_STORAGE_KEY)).toBe('1');
    setQuantize(false);
    expect(localStorage.getItem(QUANTIZE_STORAGE_KEY)).toBe('0');
  });

  it('snaps a hot cue set at the playhead to the nearest beat when on', () => {
    setQuantize(true);
    player.progress = 1740 / 20_000;
    addHotCueAtPlayhead(1);
    expect(cuesOf(7).map((c) => c.timeMs)).toEqual([1500]);
  });

  it('sets a hot cue exactly at the playhead when off', () => {
    player.progress = 1740 / 20_000;
    addHotCueAtPlayhead(1);
    expect(cuesOf(7).map((c) => c.timeMs)).toEqual([1740]);
  });

  it('plays the CUE preview from the nearest beat when on, and returns there', async () => {
    setQuantize(true);
    player.progress = 1740 / 20_000;
    await cuePreviewStart();
    expect(engine.played).toEqual([1500]);
    player.playing = false;
    await cuePreviewStop();
    expect(engine.positions).toEqual([1500]);
  });

  it('plays the CUE preview from the playhead when off', async () => {
    player.progress = 1740 / 20_000;
    await cuePreviewStart();
    expect(engine.played).toEqual([1740]);
  });
});

describe('moving a hot cue', () => {
  it('saves the moved cue like any other cue edit', async () => {
    addHotCueAtPlayhead(2);
    await flushCueEdits();
    invokeMock.mockClear();
    moveHotCue(7, 'h2', 2500);
    expect(cuesOf(7).map((c) => [c.slot, c.timeMs])).toEqual([[2, 2500]]);
    await vi.advanceTimersByTimeAsync(1000);
    expect(savedCues()).toHaveLength(1);
    expect(savedCues()[0][1]).toMatchObject({ trackId: 7, cues: [{ hot_cue_number: 2, time_ms: 2500 }] });
  });

  it('leaves memory cues and unknown keys alone', () => {
    const before = cuesOf(7);
    moveHotCue(7, 'h8', 100);
    moveHotCue(7, 'm0', 100);
    expect(cuesOf(7)).toBe(before);
  });
});
