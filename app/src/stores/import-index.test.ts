import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { TrackInfo } from '../services/tauri.svelte.ts';

const hooks = vi.hoisted(() => ({
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
  invoke: vi.fn(async (command: string) => {
    if (command === 'load_state') return structuredClone(hooks.state);
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
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

import { bumpLibraryTracks, filterTracks, importFiles, initLibrary, library, loadState, tracksForSidebarSource } from './library.svelte.ts';
import { clearSort, setSort, sortTracks, startLibrarySortWarmup } from './table-columns.svelte.ts';

function generator(seed: number) {
  let state = seed;
  return () => {
    state = (state * 1664525 + 1013904223) >>> 0;
    return state / 2 ** 32;
  };
}

const WORDS = ['house', 'Dub', 'amo', 'Ärger', 'ßeta', 'İstanbul', 'Σίσυφος', '日本語', 'mix', 'a b', '12', 'Zed', 'zed', 'Émile', 'x'];
const KEYS = ['', '1A', '8A', '12B', '4B', '9A'];
const QUERIES = ['', 'a', 'h', 'ho', 'hou', 'house', 'dub mix', 'amo', 'a b', 'é', 'ı', 'σ', '日本', '12', '1', '124', 'imp', 'track1', 'zed', 'zzzz'];
const SORTS: { key: string | null; twice: boolean }[] = [
  { key: null, twice: false },
  { key: 'title', twice: false },
  { key: 'title', twice: true },
  { key: 'artist', twice: false },
  { key: 'artist', twice: true },
  { key: 'album', twice: false },
  { key: 'genre', twice: true },
  { key: 'bpm', twice: false },
  { key: 'bpm', twice: true },
  { key: 'key', twice: false },
  { key: 'time', twice: false },
  { key: 'label', twice: false },
];

let random = generator(1);
let nextId = 1;

function word() {
  return WORDS[Math.floor(random() * WORDS.length)];
}

function makeTrack(extra: Partial<TrackInfo> = {}): TrackInfo {
  const id = nextId++;
  const tempo = random() < 0.3 ? 0 : 10000 + Math.floor(random() * 80) * 50;
  return {
    id,
    source_path: `/music/${word()}/track${Math.floor(random() * 3000)}.mp3`,
    title: random() < 0.05 ? '' : random() < 0.5 ? `${word()} ${Math.floor(random() * 40)}` : `Title ${id}`,
    artist: random() < 0.1 ? '' : word(),
    album: random() < 0.1 ? '' : `${word()} album`,
    genre: random() < 0.2 ? '' : word(),
    tempo,
    key: KEYS[Math.floor(random() * KEYS.length)],
    duration_secs: 100 + Math.floor(random() * 400),
    bitrate: 320,
    file_size: 1000 + id,
    has_artwork: false,
    has_cues: false,
    has_analysis: tempo > 0,
    analysis_pending: false,
    ...extra,
  };
}

const makeTracks = (count: number) => Array.from({ length: count }, () => makeTrack());

function hashOf(rows: { id: number }[]): string {
  let a = 0x811c9dc5;
  let b = 0x9e3779b9;
  for (const row of rows) {
    a = Math.imul(a ^ row.id, 0x01000193) >>> 0;
    b = Math.imul(b + row.id + 1, 0x85ebca6b) >>> 0;
    b ^= b >>> 13;
  }
  return `${rows.length}/${a.toString(16)}${b.toString(16)}`;
}

function everyView(): Record<string, string> {
  const hashes: Record<string, string> = {};
  for (const sort of SORTS) {
    clearSort('library');
    if (sort.key) {
      setSort('library', sort.key);
      if (sort.twice) setSort('library', sort.key);
    }
    for (const query of QUERIES) {
      const filtered = filterTracks(tracksForSidebarSource('All Tracks'), query);
      hashes[`${sort.key ?? 'none'}${sort.twice ? ' desc' : ''} "${query}"`] = hashOf(sortTracks(filtered, 'library'));
    }
  }
  clearSort('library');
  return hashes;
}

function drainWarmup() {
  const warmup = startLibrarySortWarmup(['title', 'artist', 'album', 'label', 'genre', 'bpm', 'key', 'time']);
  while (!warmup.step(50));
}

let all: TrackInfo[] = [];

async function importStep(added: TrackInfo[], warm = false) {
  hooks.added = added;
  await importFiles(['/new']);
  all = [...all, ...added];
  if (warm) drainWarmup();
}

async function fullRebuild(): Promise<Record<string, string>> {
  hooks.state = { tracks: all, playlists: [] };
  await loadState();
  return everyView();
}

async function expectSameAsFullRebuild() {
  const incremental = everyView();
  const rebuilt = await fullRebuild();
  expect(incremental).toEqual(rebuilt);
  expect(Object.keys(rebuilt).length).toBe(SORTS.length * QUERIES.length);
}

async function startWith(tracks: TrackInfo[]) {
  all = tracks;
  hooks.state = { tracks: all, playlists: [] };
  await initLibrary();
  everyView();
}

beforeEach(() => {
  random = generator(7);
  nextId = 1;
});

describe('importing into a library that already has its search text and sort classes', () => {
  it('gives every query and sort of a full rebuild after steps of 1, 1,000 and none, into an empty library first', async () => {
    await startWith([]);
    await importStep(makeTracks(5));
    await expectSameAsFullRebuild();
    await startWith(makeTracks(2000));
    await importStep(makeTracks(1));
    await expectSameAsFullRebuild();
    await importStep(makeTracks(1000));
    await expectSameAsFullRebuild();
    await importStep([]);
    await expectSameAsFullRebuild();
    await importStep(makeTracks(1000), true);
    await expectSameAsFullRebuild();
  }, 120_000);

  it('does the same with duplicates of existing titles, artists, paths and values', async () => {
    await startWith(makeTracks(1500));
    const copies = all.slice(0, 300).map((t) => makeTrack({ title: t.title, artist: t.artist, album: t.album, genre: t.genre, source_path: t.source_path, tempo: t.tempo, key: t.key, duration_secs: t.duration_secs }));
    await importStep(copies);
    await expectSameAsFullRebuild();
    await importStep(copies.map((t) => makeTrack({ ...t, id: nextId })), true);
    await expectSameAsFullRebuild();
  }, 120_000);

  it('does the same after renamed and moved tracks were bumped before and after the import', async () => {
    await startWith(makeTracks(1500));
    const rename = (count: number, offset: number) => {
      const ids: number[] = [];
      for (let k = 0; k < count; k++) {
        const info = all[(offset + k * 37) % all.length];
        const row = library.tracks.find((t) => t.id === info.id)!;
        info.title = `Renamed ${k} ${word()}`;
        info.source_path = `/moved/${word()}/${k}.mp3`;
        info.tempo = 12000 + k;
        row.title = info.title;
        row.filePath = info.source_path;
        row.bpm = (info.tempo / 100).toFixed(1);
        row.raw.title = info.title;
        row.raw.source_path = info.source_path;
        row.raw.tempo = info.tempo;
        ids.push(info.id);
      }
      bumpLibraryTracks(ids);
    };
    rename(3, 5);
    everyView();
    await importStep(makeTracks(400));
    await expectSameAsFullRebuild();
    rename(5, 11);
    await importStep(makeTracks(200), true);
    rename(2, 21);
    await expectSameAsFullRebuild();
    await importStep(makeTracks(50));
    rename(40, 3);
    await expectSameAsFullRebuild();
  }, 120_000);

  it('does the same after tracks were removed and the library reloaded', async () => {
    await startWith(makeTracks(1500));
    await importStep(makeTracks(300));
    all = all.filter((_, i) => i % 7 !== 3);
    hooks.state = { tracks: all, playlists: [] };
    await loadState();
    everyView();
    await importStep(makeTracks(200));
    await expectSameAsFullRebuild();
    await importStep(makeTracks(100), true);
    await expectSameAsFullRebuild();
  }, 120_000);

  it('does the same when an import lands while the sort of a column holds values of two kinds', async () => {
    await startWith(makeTracks(800));
    await importStep(makeTracks(100).map((t) => ({ ...t, key: t.key || '8A' })));
    await expectSameAsFullRebuild();
  }, 120_000);
});
