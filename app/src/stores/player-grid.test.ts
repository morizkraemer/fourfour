import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('../services/playback.svelte.ts', () => ({
  playbackPause: vi.fn(),
  playbackPlay: vi.fn(async () => undefined),
  playbackSeek: vi.fn(),
  playbackSetPosition: vi.fn(),
  playbackStop: vi.fn(async () => undefined),
  subscribePlaybackError: vi.fn(async () => () => {}),
  subscribePlaybackTick: vi.fn(async () => () => {}),
}));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(async () => undefined) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

import { invoke } from '@tauri-apps/api/core';
import { playbackPause, subscribePlaybackTick } from '../services/playback.svelte.ts';
import { invalidateAnalysisCache } from '../services/tauri.svelte.ts';
import {
  beatGridNudgeLeft,
  beatGridNudgeRight,
  beatGridSetCurrent,
  loadPlayerTrack,
  player,
  resetPlayerTransport,
  seekToProgress,
  togglePlay,
} from './player.svelte.ts';

const invokeMock = vi.mocked(invoke);

let storedBeats: Array<{ time_ms: number; bar_position: number }>;
let storedOffsetMs = 0;

const hasDownbeatAt = (ms: number) => player.track!.beats.some((b) => b.bar_position === 1 && Math.abs(b.time_ms - ms) < 1e-6);

function backend() {
  invokeMock.mockImplementation(async (command: string, args?: unknown): Promise<any> => {
    if (command === 'get_analysis_data') {
      return {
        waveform_preview: Array.from({ length: 400 }, (_, i) => 1 + (i % 30)),
        bpm: 120,
        key: '1A',
        beats: storedBeats,
        grid_offset_ms: storedOffsetMs,
        duration_ms: 20_000,
      };
    }
    if (command === 'set_beat_grid') {
      storedBeats = (args as { beats: typeof storedBeats }).beats;
      storedOffsetMs = (args as { offsetMs: number }).offsetMs;
    }
    return undefined;
  });
}

const trackInfo = {
  id: 4,
  source_path: '/music/four.wav',
  title: 'Four',
  artist: '',
  tempo: 12000,
  key: '1A',
  duration_secs: 20,
  has_analysis: true,
};

const saveCalls = () => invokeMock.mock.calls.filter(([command]) => command === 'set_beat_grid');

beforeEach(async () => {
  vi.useFakeTimers();
  invalidateAnalysisCache();
  invokeMock.mockReset();
  storedOffsetMs = 0;
  storedBeats = [1, 2, 3, 4, 5].map((n) => ({ time_ms: 500 * n, bar_position: ((n - 1) % 4) + 1 }));
  backend();
  await loadPlayerTrack(trackInfo);
  await vi.advanceTimersByTimeAsync(0);
  invokeMock.mockClear();
});

afterEach(() => vi.useRealTimers());

describe('beat grid editing in the player', () => {
  it('shows a nudge at once as a new track object (the waveform only reacts to that)', () => {
    const before = player.track;
    beatGridNudgeRight();
    expect(player.track).not.toBe(before);
    expect(player.track!.beats.map((b) => b.time_ms)).toEqual([510, 1010, 1510, 2010, 2510]);
    expect(before!.beats[0].time_ms).toBe(500);
  });

  it('saves one grid after a burst of nudges, with the final positions', async () => {
    beatGridNudgeRight();
    beatGridNudgeRight();
    beatGridNudgeLeft();
    beatGridNudgeRight();
    expect(saveCalls()).toHaveLength(0);
    await vi.advanceTimersByTimeAsync(500);
    expect(saveCalls()).toHaveLength(1);
    expect(saveCalls()[0][1]).toEqual({
      trackId: 4,
      beats: [520, 1020, 1520, 2020, 2520].map((time_ms, i) => ({ time_ms, bar_position: (i % 4) + 1 })),
      offsetMs: 0,
    });
  });

  it('returns the stored grid when the track is loaded again', async () => {
    beatGridNudgeRight();
    beatGridNudgeRight();
    await vi.advanceTimersByTimeAsync(500);
    await loadPlayerTrack({ ...trackInfo, id: 5 });
    await loadPlayerTrack(trackInfo);
    await vi.advanceTimersByTimeAsync(0);
    expect(player.track!.beats[0].time_ms).toBe(520);
  });

  it('writes a pending edit before another track loads', async () => {
    beatGridNudgeRight();
    await loadPlayerTrack({ ...trackInfo, id: 5 });
    expect(saveCalls()).toHaveLength(1);
    expect(saveCalls()[0][1]).toMatchObject({ trackId: 4 });
  });

  it('nudges by the step it is given, down to one millisecond', () => {
    beatGridNudgeRight(1);
    beatGridNudgeRight(1);
    beatGridNudgeLeft(3);
    expect(player.track!.beats.map((b) => b.time_ms)).toEqual([499, 999, 1499, 1999, 2499]);
  });

  it('sets the downbeat exactly where the playhead is, between two milliseconds', () => {
    player.progress = 1287.4 / 20_000;
    beatGridSetCurrent();
    expect(hasDownbeatAt(1287.4)).toBe(true);
  });

  it('saves a grid set between milliseconds as whole ms beats plus its offset, and loads it back exactly', async () => {
    player.progress = 1287.4 / 20_000;
    beatGridSetCurrent();
    await vi.advanceTimersByTimeAsync(500);
    const saved = saveCalls()[0][1] as { beats: Array<{ time_ms: number }>; offsetMs: number };
    expect(saved.beats.every((b) => Number.isInteger(b.time_ms))).toBe(true);
    expect(saved.beats.some((b) => b.time_ms === 1287)).toBe(true);
    expect(saved.offsetMs).toBeCloseTo(0.4, 6);
    await loadPlayerTrack({ ...trackInfo, id: 5 });
    await loadPlayerTrack(trackInfo);
    await vi.advanceTimersByTimeAsync(0);
    expect(hasDownbeatAt(1287.4)).toBe(true);
  });

  it('puts the nearest downbeat on the playhead with Set', async () => {
    player.progress = 1300 / 20_000;
    beatGridSetCurrent();
    expect(player.track!.beats.find((b) => b.bar_position === 1 && b.time_ms === 1300)).toBeTruthy();
    await vi.advanceTimersByTimeAsync(500);
    expect(saveCalls()[0][1]).toMatchObject({ trackId: 4 });
  });

  it('keeps a paused seek between two milliseconds as the playhead though the backend reports the whole ms', async () => {
    let tick: ((t: any) => void) | null = null;
    resetPlayerTransport();
    vi.mocked(subscribePlaybackTick).mockImplementation(async (cb: any) => {
      tick = cb;
      return () => {};
    });
    await seekToProgress(1287.4 / 20_000);
    tick!({ position_ms: 1287, duration_ms: 20_000, playing: false });
    expect(player.progress * 20_000).toBeCloseTo(1287.4, 6);
    beatGridSetCurrent();
    expect(hasDownbeatAt(1287.4)).toBe(true);
    tick!({ position_ms: 1500, duration_ms: 20_000, playing: false });
    expect(player.progress * 20_000).toBeCloseTo(1500, 6);
  });

  it('sets the grid at the exact paused position, not the last 50 ms tick, and it survives a save and reload', async () => {
    let tick: ((t: any) => void) | null = null;
    resetPlayerTransport();
    vi.mocked(subscribePlaybackTick).mockImplementation(async (cb: any) => {
      tick = cb;
      return () => {};
    });
    await togglePlay();
    expect(player.playing).toBe(true);
    for (const position_ms of [1200, 1250, 1300]) tick!({ position_ms, duration_ms: 20_000, playing: true });
    vi.mocked(playbackPause).mockImplementationOnce(async () => {
      tick!({ position_ms: 1300, duration_ms: 20_000, playing: false });
      return 1337;
    });
    await togglePlay();
    expect(player.playing).toBe(false);
    expect(player.progress * 20_000).toBeCloseTo(1337, 6);

    beatGridSetCurrent();
    beatGridNudgeRight(1);
    beatGridNudgeRight(1);
    const downbeat = 1339;
    expect(hasDownbeatAt(downbeat)).toBe(true);
    await vi.advanceTimersByTimeAsync(500);
    expect(storedBeats.some((b) => b.bar_position === 1 && b.time_ms === downbeat)).toBe(true);

    await loadPlayerTrack({ ...trackInfo, id: 5 });
    await loadPlayerTrack(trackInfo);
    await vi.advanceTimersByTimeAsync(0);
    expect(hasDownbeatAt(downbeat)).toBe(true);
  });
});
