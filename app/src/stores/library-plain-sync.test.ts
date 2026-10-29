// @vitest-environment client-runtime
import { beforeEach, describe, expect, it, vi } from 'vitest';

const hooks = vi.hoisted(() => ({
  events: {} as Record<string, ((event: { payload: unknown }) => void)[]>,
  analysis: {} as Record<number, unknown>,
  state: { tracks: [] as unknown[], playlists: [] as unknown[] },
  added: [] as unknown[],
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
  invoke: vi.fn(async (command: string, args: Record<string, any> = {}) => {
    if (command === 'load_state') return structuredClone(hooks.state);
    if (command === 'get_analysis_data') return hooks.analysis[args.trackId];
    if (command === 'update_track_fields') return { title: 'Renamed', artist: 'Someone', album: 'Al', genre: 'Techno', ...args.edits };
    if (command === 'locate_track_file') {
      return {
        relinked: { track_id: args.trackId, source_path: args.path },
        health: { missing: [], error: [], relocated: [], total: 3, checked_at_ms: 1, duration_ms: 1, running: false },
      };
    }
    if (command === 'plan_import') {
      return {
        token: 1,
        new_count: hooks.added.length,
        already_in_library: 0,
        duplicates: [],
        skipped: { unsupported: 0, unsupported_exts: [], unreadable: 0 },
      };
    }
    if (command === 'commit_import') return { added: structuredClone(hooks.added), skipped_ids: [] };
    if (command === 'get_app_version') return '0.0.0';
    if (command === 'get_library_path') return '/lib';
    if (command === 'get_mounted_volumes') return [];
    if (command === 'resume_analysis') return 0;
    return undefined;
  }),
}));
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (name: string, callback: (event: { payload: unknown }) => void) => {
    (hooks.events[name] ??= []).push(callback);
    return () => {};
  }),
}));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn(async () => '/moved/found.mp3') }));

import {
  analyzeTrackIds,
  ensureTrackPeaksLoaded,
  filterTracks,
  importFiles,
  initLibrary,
  library,
  toggleFavoriteSlot,
  unanalyzedTrackCount,
} from './library.svelte.ts';
import { invalidateAnalysisCache } from '../services/tauri.svelte.ts';
import { commitTrackFields, commitTrackKey, commitTrackRating } from './track-edits.svelte.ts';
import { locateTrack } from './file-health.svelte.ts';
import { sortTracks, tableLayouts } from './table-columns.svelte.ts';

Object.assign(globalThis, { window: { __TAURI_INTERNALS__: {} } });

const ALL_STAGES = 7;
const BANDS = btoa(String.fromCharCode(...Array.from({ length: 30 }, (_, i) => 10 + i)));

function backendTrack(id: number, analysed: boolean) {
  return {
    id,
    source_path: `/music/t${id}.mp3`,
    title: `Title ${id}`,
    artist: 'Artist',
    album: 'Album',
    genre: 'House',
    tempo: analysed ? 12800 : 0,
    key: analysed ? '8A' : '',
    duration_secs: 300,
    bitrate: 320,
    file_size: 1,
    has_artwork: false,
    has_cues: false,
    has_analysis: analysed,
    analysis_pending: false,
    waveform_bands: analysed ? BANDS : undefined,
  };
}

function payload(bpm: number, key: string) {
  return {
    waveform_preview: Array.from({ length: 400 }, (_, i) => 8 + (i % 20)),
    waveform_peaks: [],
    waveform_row_bands: BANDS,
    bpm,
    key,
    bpm_locked: false,
    key_locked: false,
    beats: [{ time_ms: 0, bar_position: 1 }],
    cue_points: [],
    duration_ms: 300000,
    analysis_pending: false,
    analysis_stages: ALL_STAGES,
  };
}

function emit(name: string, body: unknown) {
  for (const callback of hooks.events[name] ?? []) callback({ payload: body });
}

function row(id: number) {
  return library.tracks.find((t) => t.id === id)!;
}

function search(query: string) {
  return filterTracks(library.tracks, query).map((t) => t.id);
}

function warmSearchIndex() {
  search('zzz-warm');
}

beforeEach(async () => {
  invalidateAnalysisCache();
  hooks.state = { tracks: [backendTrack(1, true), backendTrack(2, false), backendTrack(3, true)], playlists: [] };
  hooks.added = [backendTrack(4, false)];
  hooks.analysis = { 1: payload(128, '8A'), 2: payload(124, '5A'), 3: payload(128, '8A') };
  await initLibrary();
  library.analyzingTrackIds = [];
});

it('runs against the client runtime, where rows are reactive proxies', () => {
  expect(import.meta.env.SSR).toBe(false);
  expect(() => structuredClone(row(1))).toThrow();
});

describe.each([
  ['a cold search index', () => {}],
  ['a warm search index', warmSearchIndex],
])('track field writes reach search and counts, with %s', (_name, prepare) => {
  it('tag edits', async () => {
    prepare();
    await commitTrackFields(1, { title: 'Zebra Crossing' });
    expect(search('zebra')).toEqual([1]);
    expect(search('title')).toEqual([2, 3]);
  });

  it('key edits', async () => {
    prepare();
    await commitTrackKey(3, '11B');
    expect(search('11b')).toEqual([3]);
  });

  it('file relocation', async () => {
    prepare();
    await locateTrack(2);
    expect(search('found.mp3')).toEqual([2]);
  });

  it('analysis results', async () => {
    prepare();
    expect(unanalyzedTrackCount()).toBe(1);
    await analyzeTrackIds([2]);
    emit('analysis-progress', { current: 1, total: 1, message: 'Analyzed t2.mp3 (1/1)', track_id: 2 });
    await vi.waitFor(() => expect(row(2).analyzed).toBe(true));
    expect(unanalyzedTrackCount()).toBe(0);
    expect(search('5a')).toEqual([2]);
  });

  it('re-analysis start', async () => {
    prepare();
    await analyzeTrackIds([1]);
    expect(row(1).bpm).toBe('—');
    expect(search('128.0')).toEqual([3]);
  });

  it('a stored analysis that turns out unreadable', async () => {
    prepare();
    hooks.analysis[3] = undefined;
    await ensureTrackPeaksLoaded(row(3));
    expect(row(3).analyzed).toBe(false);
    expect(unanalyzedTrackCount()).toBe(2);
  });
});

describe('track field writes survive landing an import', () => {
  it('keeps tag, rating, key, favourite and analysis state on the existing rows', async () => {
    const proxyBefore = row(1);
    await commitTrackFields(1, { title: 'Zebra Crossing' });
    await commitTrackRating(1, 4);
    await commitTrackKey(3, '11B');
    await toggleFavoriteSlot(1, [3]);
    await analyzeTrackIds([1]);

    await importFiles(['/new']);

    expect(library.tracks.map((t) => t.id)).toEqual([1, 2, 3, 4]);
    expect(row(1)).toBe(proxyBefore);
    expect([row(1).title, row(1).rating, row(1).key, row(1).analysisPending]).toEqual(['Zebra Crossing', 4, '—', true]);
    expect([row(3).key, row(3).fav]).toEqual(['11B', 1]);
    expect(search('zebra')).toEqual([1]);
    expect(search('11b')).toEqual([3]);
  });
});

describe('sort keys follow field writes', () => {
  it('re-sorts by rating and title after edits', async () => {
    const sortedIds = () => sortTracks(library.tracks, 'library').map((t) => t.id);
    tableLayouts.library = { ...tableLayouts.library, sortKey: 'rating', sortDir: 'desc' };
    expect(sortedIds()).toEqual([1, 2, 3]);
    await commitTrackRating(2, 5);
    expect(sortedIds()).toEqual([2, 1, 3]);

    tableLayouts.library = { ...tableLayouts.library, sortKey: 'title', sortDir: 'asc' };
    expect(sortedIds()).toEqual([1, 2, 3]);
    await commitTrackFields(1, { title: 'Zebra Crossing' });
    expect(sortedIds()).toEqual([2, 3, 1]);
  });
});
