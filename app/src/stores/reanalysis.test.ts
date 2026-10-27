import { beforeEach, describe, expect, it, vi } from 'vitest';

const hooks = vi.hoisted(() => ({
  events: {} as Record<string, (event: { payload: unknown }) => void>,
  analysis: {} as Record<number, unknown>,
  loadState: { tracks: [] as unknown[], playlists: [] as unknown[] },
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
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (command: string, args: { trackId?: number }) => {
    if (command === 'load_state') return hooks.loadState;
    if (command === 'get_analysis_data') return hooks.analysis[args.trackId as number];
    if (command === 'get_app_version') return '0.0.0';
    if (command === 'get_library_path') return '/lib';
    if (command === 'get_mounted_volumes') return [];
    if (command === 'resume_analysis') return 0;
    return undefined;
  }),
}));
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (name: string, callback: (event: { payload: unknown }) => void) => {
    hooks.events[name] = callback;
    return () => {};
  }),
}));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

import { invoke } from '@tauri-apps/api/core';
import { analyzeTrackIds, initLibrary, isTrackAnalyzing, library } from './library.svelte.ts';
import { invalidateAnalysisCache } from '../services/tauri.svelte.ts';
import { initPlayerAnalysisRefresh, loadPlayerTrack, player } from './player.svelte.ts';

const ALL_STAGES = 7;
const WAVEFORM_AND_KEY = 3;
const BANDS = btoa(String.fromCharCode(...Array.from({ length: 30 }, (_, i) => 10 + i)));

function backendTrack(id: number) {
  return {
    id,
    source_path: `/music/t${id}.wav`,
    title: `t${id}`,
    artist: '',
    album: '',
    genre: '',
    tempo: 12800,
    key: '8A',
    duration_secs: 300,
    bitrate: 320,
    file_size: 1,
    has_artwork: false,
    has_cues: false,
    has_analysis: true,
    analysis_pending: false,
    waveform_bands: BANDS,
  };
}

function payload(bpm: number, key: string, stages: number, locks: { bpm?: boolean; key?: boolean } = {}) {
  return {
    waveform_preview: Array.from({ length: 400 }, (_, i) => 8 + (i % 20)),
    waveform_peaks: [],
    waveform_color: null,
    waveform_color_detail: null,
    waveform_overview: null,
    waveform_row_bands: BANDS,
    bpm,
    key,
    bpm_locked: locks.bpm === true,
    key_locked: locks.key === true,
    energy: 5,
    beats: [{ time_ms: 0, bar_position: 1 }],
    cue_points: [],
    duration_ms: 300000,
    analysis_pending: stages !== ALL_STAGES,
    analysis_stages: stages,
  };
}

function row(id: number) {
  return library.tracks.find((t) => t.id === id)!;
}

function emit(name: string, body: unknown) {
  hooks.events[name]({ payload: body });
}

beforeEach(async () => {
  invalidateAnalysisCache();
  hooks.loadState = { tracks: [backendTrack(1), backendTrack(2)], playlists: [] };
  hooks.analysis[1] = payload(128, '8A', ALL_STAGES);
  hooks.analysis[2] = payload(128, '8A', ALL_STAGES);
  await initLibrary();
  library.analyzingTrackIds = [];
});

describe('re-analysis of analysed tracks', () => {
  it('starts like a first analysis: waveform, BPM and key cleared, animation on', async () => {
    expect(row(1).bpm).toBe('128.0');
    expect(row(1).bands).toBeTruthy();

    await analyzeTrackIds([1]);

    expect(vi.mocked(invoke)).toHaveBeenCalledWith('analyze_track_ids', { ids: [1] });
    expect(isTrackAnalyzing(1)).toBe(true);
    expect(isTrackAnalyzing(2)).toBe(false);
    expect(row(1).bpm).toBe('—');
    expect(row(1).key).toBe('—');
    expect(row(1).bands).toBeFalsy();
    expect(row(1).analysisPending).toBe(true);
    expect(row(2).bpm).toBe('128.0');
  });

  it('keeps hand-set BPM and key', async () => {
    row(1).bpmLocked = true;
    row(2).keyLocked = true;

    await analyzeTrackIds([1, 2]);

    expect([row(1).bpm, row(1).key]).toEqual(['128.0', '—']);
    expect([row(2).bpm, row(2).key]).toEqual(['—', '8A']);
  });

  it('refills per stage: waveform and key first, BPM with the grid', async () => {
    await analyzeTrackIds([1]);
    hooks.analysis[1] = payload(0, '9A', WAVEFORM_AND_KEY);
    emit('analysis-partial', { track_id: 1 });
    await vi.waitFor(() => expect(row(1).key).toBe('9A'));
    expect(row(1).bpm).toBe('—');
    expect(row(1).bands).toBeTruthy();
    expect(isTrackAnalyzing(1)).toBe(true);

    hooks.analysis[1] = payload(126, '9A', ALL_STAGES);
    emit('analysis-progress', { current: 1, total: 1, message: 'Analyzed t1.wav (1/1)', track_id: 1 });
    await vi.waitFor(() => expect(row(1).bpm).toBe('126.0'));
    expect(isTrackAnalyzing(1)).toBe(false);
    expect(row(1).analysisPending).toBe(false);
  });

  it('does not show stale stored values while stages are still missing', async () => {
    await analyzeTrackIds([1]);
    hooks.analysis[1] = payload(128, '8A', 0);
    emit('analysis-partial', { track_id: 1 });
    await vi.waitFor(() => expect(row(1).analysisLoaded).toBe(true));
    expect([row(1).bpm, row(1).key]).toEqual(['—', '—']);
    expect(row(1).bands).toBeFalsy();
  });

  it('shows a hand-set value from the payload even before its stage is stored', async () => {
    row(1).bpmLocked = true;
    await analyzeTrackIds([1]);
    hooks.analysis[1] = payload(127.5, '9A', WAVEFORM_AND_KEY, { bpm: true });
    emit('analysis-partial', { track_id: 1 });
    await vi.waitFor(() => expect(row(1).key).toBe('9A'));
    expect(row(1).bpm).toBe('127.5');
  });

  it('clears and refills the loaded player track the same way', async () => {
    initPlayerAnalysisRefresh();
    await loadPlayerTrack(row(1));
    await vi.waitFor(() => expect(player.loading).toBe(false));
    expect([player.track?.bpm, player.track?.key]).toEqual(['128.00', '8A']);
    expect(player.track?.bands).toBeTruthy();

    await analyzeTrackIds([1]);
    expect([player.track?.bpm, player.track?.key]).toEqual(['—', '—']);
    expect(player.track?.bands).toBeNull();

    hooks.analysis[1] = payload(0, '9A', WAVEFORM_AND_KEY);
    emit('analysis-partial', { track_id: 1 });
    await vi.waitFor(() => expect(player.track?.key).toBe('9A'));
    expect(player.track?.bpm).toBe('—');
    expect(player.track?.bands).toBeTruthy();

    hooks.analysis[1] = payload(126, '9A', ALL_STAGES);
    emit('analysis-progress', { current: 1, total: 1, message: 'Analyzed t1.wav (1/1)', track_id: 1 });
    await vi.waitFor(() => expect(player.track?.bpm).toBe('126.00'));
  });
});
