import { beforeEach, describe, expect, it, vi } from 'vitest';

const hooks = vi.hoisted(() => ({
  events: {} as Record<string, (event: { payload: unknown }) => void>,
  analysis: {} as Record<number, unknown>,
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
  invoke: vi.fn(async (command: string, args: { trackId?: number }) =>
    command === 'get_analysis_data' ? hooks.analysis[args.trackId as number] : undefined,
  ),
}));
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (name: string, callback: (event: { payload: unknown }) => void) => {
    hooks.events[name] = callback;
    return () => {};
  }),
}));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

import { library } from './library.svelte.ts';
import { invalidateAnalysisCache } from '../services/tauri.svelte.ts';
import { initPlayerAnalysisRefresh, loadPlayerTrack, player } from './player.svelte.ts';

function backendPayload(bpm: number, key: string) {
  return {
    waveform_preview: Array.from({ length: 400 }, (_, i) => 8 + (i % 20)),
    waveform_peaks: [],
    waveform_color: null,
    waveform_color_detail: null,
    waveform_overview: null,
    waveform_row_bands: null,
    bpm,
    key,
    bpm_locked: false,
    key_locked: false,
    energy: 5,
    beats: [{ time_ms: 0, bar_position: 1 }],
    cue_points: [],
    duration_ms: 300000,
    analysis_pending: false,
  };
}

function staleLoadStateRow(id: number) {
  return {
    id,
    source_path: `/music/t${id}.wav`,
    title: `t${id}`,
    artist: '',
    tempo: 0,
    key: '',
    duration_secs: 300,
    has_analysis: true,
    analysis_pending: false,
  };
}

function libraryRow(id: number, extra: Record<string, unknown> = {}) {
  return {
    id,
    title: `t${id}`,
    filePath: `/music/t${id}.wav`,
    analyzed: true,
    analysisPending: false,
    bpm: '—',
    key: '—',
    raw: staleLoadStateRow(id),
    ...extra,
  };
}

async function settle() {
  await vi.waitFor(() => expect(player.loading).toBe(false));
}

beforeEach(() => {
  invalidateAnalysisCache();
  library.tracks = [libraryRow(11), libraryRow(12)];
  hooks.analysis[11] = backendPayload(128, '8A');
  hooks.analysis[12] = backendPayload(0, '');
});

describe('player BPM and key on load', () => {
  it('shows the analysis values of an analysed track whose load-state row carries none', async () => {
    await loadPlayerTrack(libraryRow(11));
    await settle();
    expect(player.track?.bpm).toBe('128.00');
    expect(player.track?.key).toBe('8A');
  });

  it('shows the row values at once, before the analysis payload arrives', async () => {
    library.tracks = [libraryRow(11, { bpm: '127.9', bpmExact: 127.9, key: '5B' })];
    await loadPlayerTrack(library.tracks[0]);
    expect(player.track?.bpm).toBe('127.90');
    expect(player.track?.key).toBe('5B');
    await settle();
    expect(player.track?.bpm).toBe('128.00');
    expect(player.track?.key).toBe('8A');
  });

  it('keeps the row values when the analysis payload cannot be read', async () => {
    hooks.analysis[11] = undefined;
    library.tracks = [libraryRow(11, { bpm: '127.9', bpmExact: 127.9, key: '5B' })];
    await loadPlayerTrack(library.tracks[0]);
    await settle();
    expect(player.track?.bpm).toBe('127.90');
    expect(player.track?.key).toBe('5B');
  });

  it('updates the loaded track when a stage of the analysis finishes', async () => {
    initPlayerAnalysisRefresh();
    await loadPlayerTrack(libraryRow(12));
    await settle();
    expect(player.track?.bpm).toBe('—');
    hooks.analysis[12] = backendPayload(140, '2A');
    hooks.events['analysis-progress']({ payload: { current: 1, total: 1, message: 'x', track_id: 12 } });
    await vi.waitFor(() => expect(player.track?.bpm).toBe('140.00'));
    expect(player.track?.key).toBe('2A');
  });
});
