import { beforeEach, describe, expect, it, vi } from 'vitest';

const hooks = vi.hoisted(() => ({
  events: {} as Record<string, (event: { payload: unknown }) => void>,
}));

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
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (name: string, callback: (event: { payload: unknown }) => void) => {
    hooks.events[name] = callback;
    return () => {};
  }),
}));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

import { invoke } from '@tauri-apps/api/core';
import { library } from './library.svelte.ts';
import { initPlayerAnalysisRefresh, loadPlayerTrack, playFromStart } from './player.svelte.ts';

const invokeMock = vi.mocked(invoke);

function prioritizeCalls() {
  return invokeMock.mock.calls.filter(([command]) => command === 'prioritize_analysis');
}

function libraryTrack(id: number, analyzed: boolean) {
  return { id, title: `t${id}`, filePath: `/music/t${id}.wav`, analyzed, analysisLoaded: false };
}

function backendTrack(id: number, hasAnalysis: boolean) {
  return {
    id,
    source_path: `/music/t${id}.wav`,
    title: `t${id}`,
    artist: '',
    tempo: hasAnalysis ? 12800 : 0,
    key: '',
    duration_secs: 300,
    has_analysis: hasAnalysis,
  };
}

beforeEach(() => {
  invokeMock.mockClear();
  library.tracks = [libraryTrack(7, false), libraryTrack(8, true)];
  library.analyzingTrackIds = [];
});

describe('priority analysis from the player', () => {
  it('prioritises an unanalysed track when it is loaded', async () => {
    await loadPlayerTrack(backendTrack(7, false));
    expect(prioritizeCalls()).toEqual([['prioritize_analysis', { trackId: 7 }]]);
    expect(library.analyzingTrackIds).toContain(7);
  });

  it('prioritises an unanalysed track when it starts playing', async () => {
    await loadPlayerTrack(backendTrack(7, false));
    invokeMock.mockClear();
    await playFromStart();
    expect(prioritizeCalls()).toEqual([['prioritize_analysis', { trackId: 7 }]]);
  });

  it('puts a queued track first in the waiting list', async () => {
    library.analyzingTrackIds = [3, 4, 7];
    await loadPlayerTrack(backendTrack(7, false));
    expect(library.analyzingTrackIds).toEqual([7, 3, 4]);
    expect(prioritizeCalls()).toHaveLength(1);
  });

  it('leaves an analysed track alone', async () => {
    await loadPlayerTrack(backendTrack(8, true));
    await playFromStart();
    expect(prioritizeCalls()).toEqual([]);
  });

  it('redraws the player when the waveform of the loaded track arrives', async () => {
    initPlayerAnalysisRefresh();
    await loadPlayerTrack(backendTrack(7, false));
    invokeMock.mockClear();
    hooks.events['analysis-partial']({ payload: { track_id: 7 } });
    await vi.waitFor(() => expect(invokeMock.mock.calls.map(([command]) => command)).toContain('get_analysis_data'));
  });

  it('ignores a waveform for a track that is not loaded', async () => {
    initPlayerAnalysisRefresh();
    await loadPlayerTrack(backendTrack(7, false));
    invokeMock.mockClear();
    hooks.events['analysis-partial']({ payload: { track_id: 99 } });
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(invokeMock.mock.calls.map(([command]) => command)).not.toContain('get_analysis_data');
  });
});
