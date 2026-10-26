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
import { invalidateAnalysisCache } from '../services/tauri.svelte.ts';
import {
  beatGridNudgeLeft,
  beatGridNudgeRight,
  beatGridSetCurrent,
  loadPlayerTrack,
  player,
} from './player.svelte.ts';

const invokeMock = vi.mocked(invoke);

let storedBeats: Array<{ time_ms: number; bar_position: number }>;

function backend() {
  invokeMock.mockImplementation(async (command: string, args?: unknown): Promise<any> => {
    if (command === 'get_analysis_data') {
      return {
        waveform_preview: Array.from({ length: 400 }, (_, i) => 1 + (i % 30)),
        bpm: 120,
        key: '1A',
        beats: storedBeats,
        duration_ms: 20_000,
      };
    }
    if (command === 'set_beat_grid') {
      storedBeats = (args as { beats: typeof storedBeats }).beats;
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

  it('puts the nearest downbeat on the playhead with Set', async () => {
    player.progress = 1300 / 20_000;
    beatGridSetCurrent();
    expect(player.track!.beats.find((b) => b.bar_position === 1 && b.time_ms === 1300)).toBeTruthy();
    await vi.advanceTimersByTimeAsync(500);
    expect(saveCalls()[0][1]).toMatchObject({ trackId: 4 });
  });
});
