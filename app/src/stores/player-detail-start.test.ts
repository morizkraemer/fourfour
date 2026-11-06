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
import { loadPlayerTrack, player } from './player.svelte.ts';

const invokeMock = vi.mocked(invoke);

const detailCalls = () => invokeMock.mock.calls.filter(([command]) => command === 'get_waveform_detail').map(([, args]) => args);

function track(id: number) {
  return { id, source_path: `/music/${id}.wav`, title: `T${id}`, artist: '', tempo: 12000, key: '1A', duration_secs: 20, has_analysis: true };
}

beforeEach(() => {
  vi.useFakeTimers();
  invalidateAnalysisCache();
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (command: string): Promise<any> => {
    if (command === 'get_analysis_data') {
      return { waveform_preview: Array.from({ length: 400 }, (_, i) => 1 + (i % 30)), bpm: 120, key: '1A', beats: [], duration_ms: 20_000 };
    }
    return undefined;
  });
});

afterEach(() => vi.useRealTimers());

describe('the zoom series of a loaded track', () => {
  it('is requested after the first frame, and shows as pending from the load on', async () => {
    await loadPlayerTrack(track(21));
    expect(player.zoomDetailLoadingId).toBe(21);
    expect(detailCalls()).toEqual([]);

    await vi.advanceTimersByTimeAsync(200);
    expect(detailCalls()).toEqual([{ trackId: 21 }]);
  });

  it('is not requested for a track that was replaced before the frame', async () => {
    await loadPlayerTrack(track(31));
    await loadPlayerTrack(track(32));
    await vi.advanceTimersByTimeAsync(200);

    expect(detailCalls()).toEqual([{ trackId: 32 }]);
  });
});
