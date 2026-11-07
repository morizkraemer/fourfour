import { describe, expect, it, vi } from 'vitest';

const hooks = vi.hoisted(() => ({
  events: {} as Record<string, ((event: { payload: unknown }) => void)[]>,
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
    (hooks.events[name] ??= []).push(callback);
    return () => {};
  }),
}));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

import { analyzeTrackIds, filterTracks, initLibrary, library, tracksForSidebarSource } from './library.svelte.ts';
import { clearSort, setSort, sortTracks } from './table-columns.svelte.ts';
import { invalidateAnalysisCache } from '../services/tauri.svelte.ts';

const TRACKS = 12;
const ALL_STAGES = 7;
const BANDS = btoa(String.fromCharCode(...Array.from({ length: 30 }, (_, i) => 10 + i)));
const APPLY_INTERVAL_MS = 250;
const SETTLE_MS = 650;

function backendTrack(id: number) {
  return {
    id,
    source_path: `/music/t${id}.wav`,
    title: `t${id}`,
    artist: id % 2 ? 'odd' : 'even',
    album: '',
    genre: '',
    tempo: 12000 + id * 10,
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

function payload(bpm: number, key: string, stages: number) {
  return {
    waveform_preview: Array.from({ length: 400 }, (_, i) => 8 + (i % 20)),
    waveform_peaks: [],
    waveform_color: null,
    waveform_color_detail: null,
    waveform_overview: null,
    waveform_row_bands: BANDS,
    bpm,
    key,
    bpm_locked: false,
    key_locked: false,
    energy: 5,
    beats: [{ time_ms: 0, bar_position: 1 }],
    cue_points: [],
    duration_ms: 300000,
    analysis_pending: stages !== ALL_STAGES,
    analysis_stages: stages,
  };
}

const KEYS = ['1A', '2B', '8A', '11B', '5A'];

const FRAMES: { id: number; finished: boolean }[][] = [
  [{ id: 1, finished: false }, { id: 2, finished: true }, { id: 3, finished: false }, { id: 4, finished: true }],
  [{ id: 3, finished: true }, { id: 5, finished: false }, { id: 6, finished: true }],
  [{ id: 1, finished: true }, { id: 2, finished: false }, { id: 7, finished: true }, { id: 8, finished: false }, { id: 9, finished: true }],
  [{ id: 10, finished: true }],
];

function emitEvent(event: { id: number; finished: boolean }, frame: number) {
  hooks.analysis[event.id] = payload(100 + event.id * 3 + frame, KEYS[(event.id + frame) % KEYS.length], event.finished ? ALL_STAGES : 3);
  const name = event.finished ? 'analysis-progress' : 'analysis-partial';
  const body = event.finished ? { current: event.id, total: TRACKS, message: `Analyzed t${event.id}.wav`, track_id: event.id } : { track_id: event.id };
  for (const callback of hooks.events[name] ?? []) callback({ payload: body });
}

const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

function view() {
  const rows = library.tracks.map((t) => ({
    id: t.id,
    bpm: t.bpm,
    bpmExact: t.bpmExact,
    key: t.key,
    analyzed: t.analyzed,
    analysisPending: t.analysisPending,
    analysisLoaded: t.analysisLoaded,
    bands: t.bandsPreview?.length ?? t.preview?.length ?? 0,
  }));
  const ids = (rows: { id: number }[]) => rows.map((r) => r.id);
  return {
    rows,
    analyzing: [...library.analyzingTrackIds],
    trackCount: library.trackCount,
    sidebarCount: library.sidebarData.library.rows[0].count,
    search: ['t1', '8a', '1a', '11b', 'odd e'].map((query) => ids(filterTracks(library.tracks, query))),
    sorted: (['bpm', 'key', 'title'] as const).flatMap((column) => {
      clearSort('library');
      setSort('library', column);
      const asc = ids(sortTracks(filterTracks(tracksForSidebarSource('All Tracks'), ''), 'library'));
      setSort('library', column);
      const desc = ids(sortTracks(filterTracks(tracksForSidebarSource('All Tracks'), 'e'), 'library'));
      clearSort('library');
      return [asc, desc];
    }),
  };
}

async function runFrames(batched: boolean) {
  invalidateAnalysisCache();
  hooks.loadState = { tracks: Array.from({ length: TRACKS }, (_, i) => backendTrack(i + 1)), playlists: [] };
  await initLibrary();
  library.analyzingTrackIds = [];
  await analyzeTrackIds(Array.from({ length: TRACKS }, (_, i) => i + 1));
  const afterFrames: ReturnType<typeof view>[] = [];
  for (const [frame, events] of FRAMES.entries()) {
    if (batched) {
      for (const event of events) emitEvent(event, frame);
      await wait(SETTLE_MS);
    } else {
      for (const event of events) {
        emitEvent(event, frame);
        await wait(APPLY_INTERVAL_MS + 60);
      }
      await wait(SETTLE_MS);
    }
    afterFrames.push(view());
  }
  return afterFrames;
}

describe('analysis events', () => {
  it('shows the same rows, order and values after every frame whether the events of a frame arrive together or one by one', async () => {
    const together = await runFrames(true);
    const oneByOne = await runFrames(false);

    expect(together.length).toBe(FRAMES.length);
    for (let frame = 0; frame < FRAMES.length; frame++) expect(together[frame], `frame ${frame}`).toStrictEqual(oneByOne[frame]);
    expect(together[FRAMES.length - 1].rows.find((r) => r.id === 10)?.analysisPending).toBe(false);
  }, 60_000);

  it('keeps the all-tracks count and the track count equal to the list length across bumps and replacements', async () => {
    hooks.loadState = { tracks: Array.from({ length: 5 }, (_, i) => backendTrack(i + 1)), playlists: [] };
    await initLibrary();
    expect([library.trackCount, library.sidebarData.library.rows[0].count]).toEqual([5, 5]);
    const before = library.sidebarData.library.rows[0];

    emitEvent({ id: 2, finished: true }, 0);
    await wait(SETTLE_MS);
    expect([library.trackCount, library.sidebarData.library.rows[0].count]).toEqual([5, 5]);
    expect(library.sidebarData.library.rows[0]).toMatchObject({ label: before.label, count: before.count });

    hooks.loadState = { tracks: Array.from({ length: 8 }, (_, i) => backendTrack(i + 1)), playlists: [] };
    await initLibrary();
    expect([library.trackCount, library.sidebarData.library.rows[0].count]).toEqual([8, 8]);
  }, 20_000);
});
