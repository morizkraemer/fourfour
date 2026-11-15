import { describe, expect, it, vi } from 'vitest';

const loaded = vi.hoisted(() => ({ tracks: [] as Record<string, unknown>[] }));

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
  invoke: vi.fn(async (command: string) => (command === 'load_state' ? { tracks: loaded.tracks, playlists: [] } : undefined)),
}));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));

import { snapshotOf } from './bump-reference.svelte.ts';
import { bumpLibraryTracks, filterTracks, library, libraryPlainRows, loadState } from './library.svelte.ts';

function generator(seed: number) {
  let state = seed;
  return () => {
    state = (state * 1664525 + 1013904223) >>> 0;
    return state / 2 ** 32;
  };
}

const WORDS = ['alpha', 'beta', 'gamma', 'dub', 'deep', 'house', 'Zed', 'amor', 'Éclair', '日本', '😀 a', ''];
const SCALAR_FIELDS = ['title', 'artist', 'album', 'genre', 'key', 'bpm', 'time', 'label', 'filePath', 'rating', 'color', 'analysisPending', 'analyzed', 'bpmExact', 'comment'];

function randomInfo(random: () => number, id: number) {
  const pick = () => WORDS[Math.floor(random() * WORDS.length)];
  return {
    id,
    source_path: `/m/${id}.mp3`,
    title: `${pick()} ${pick()}`.trim(),
    artist: pick(),
    album: pick(),
    genre: pick(),
    tempo: (90 + Math.floor(random() * 60)) * 100,
    key: `${1 + Math.floor(random() * 12)}${random() < 0.5 ? 'A' : 'B'}`,
    duration_secs: 60 + Math.floor(random() * 500),
    bitrate: 320,
    file_size: 1,
    has_artwork: random() < 0.5,
    has_cues: false,
    has_analysis: true,
    analysis_pending: false,
    ...(random() < 0.4 ? { waveform_bands: Array.from({ length: 12 }, (_, i) => (i * 5 + id) % 90) } : {}),
    ...(random() < 0.3 ? { waveform_preview: Array.from({ length: 400 }, (_, i) => (i + id) % 31) } : {}),
  };
}

const isContainer = (value: unknown) => value !== null && typeof value === 'object' && (Array.isArray(value) || Object.getPrototypeOf(value) === Object.prototype);

function randomValue(random: () => number, field: string): unknown {
  const pick = () => WORDS[Math.floor(random() * WORDS.length)];
  if (field === 'rating') return Math.floor(random() * 6);
  if (field === 'color') return random() < 0.4 ? null : pick();
  if (field === 'analysisPending' || field === 'analyzed') return random() < 0.5;
  if (field === 'bpmExact') return 90 + Math.floor(random() * 60);
  return random() < 0.1 ? undefined : pick() + String(Math.floor(random() * 4));
}

const held = new Map<number, any>();

function writeRandom(random: () => number, row: any) {
  const roll = random();
  const pick = () => WORDS[Math.floor(random() * WORDS.length)];
  if (random() < 0.08) held.set(row.id, row.raw);
  if (held.has(row.id) && random() < 0.3) held.get(row.id).heldWrite = pick();
  if (roll < 0.35) {
    const field = SCALAR_FIELDS[Math.floor(random() * SCALAR_FIELDS.length)];
    row[field] = randomValue(random, field);
  } else if (roll < 0.45) {
    row.preview = random() < 0.3 ? undefined : Uint8Array.from({ length: 8 }, () => Math.floor(random() * 256));
    row.bandsPreview = random() < 0.5 ? undefined : Uint8Array.from({ length: 6 }, () => Math.floor(random() * 256));
  } else if (roll < 0.55) {
    row.raw[['genre', 'title', 'artist', 'source_path', 'key', 'has_artwork'][Math.floor(random() * 6)]] = random() < 0.2 ? true : pick();
  } else if (roll < 0.62) {
    row.raw['extra' + Math.floor(random() * 3)] = { nested: [pick(), { deep: pick() }] };
  } else if (roll < 0.68) {
    if (Array.isArray(row.raw.waveform_bands)) row.raw.waveform_bands.push(Math.floor(random() * 100));
    else row.raw.waveform_bands = [1, 2, 3];
  } else if (roll < 0.73) {
    row.raw = { ...row.raw, title: pick(), replaced: true };
  } else if (roll < 0.78) {
    if (Array.isArray(row.tags)) row.tags.push(pick());
    else row.tags = [pick(), pick()];
  } else if (roll < 0.82) {
    if (row.tags) row.tags[0] = { a: pick() };
  } else if (roll < 0.86) {
    row['added' + Math.floor(random() * 3)] = pick();
  } else if (roll < 0.9) {
    delete row['added' + Math.floor(random() * 3)];
  } else if (roll < 0.93) {
    row.bandsWire = random() < 0.5 ? undefined : [1, 2, 3, Math.floor(random() * 9)];
  } else if (roll < 0.96) {
    Object.assign(row, { rating: Math.floor(random() * 6), color: pick() });
  } else {
    row.cover = pick();
  }
}

function referenceCopy(refRows: Record<string, any>[], rows: any[], ids: number[]) {
  for (const id of ids) {
    const i = rows.findIndex((r) => r.id === id);
    if (i < 0) continue;
    const row = rows[i];
    for (const key of Object.keys(row)) refRows[i][key] = isContainer(row[key]) ? snapshotOf(row[key]) : row[key];
  }
}

function haystackOf(row: Record<string, any>): string {
  const text = (v: unknown) => (v == null ? '' : String(v));
  const raw = row.raw ?? {};
  const same = (v: unknown, shown: string) => (text(v) === shown ? '' : ' ' + text(v));
  const title = text(row.title);
  const artist = text(row.artist);
  const album = text(row.album);
  const genre = text(row.genre);
  const key = text(row.key);
  const filePath = text(row.filePath);
  return (
    title + ' ' + artist + ' ' + album + ' ' + text(row.label) + ' ' + genre + ' ' + key + ' ' + text(row.bpm) + ' ' + text(row.time) + ' ' + filePath + ' ' + text(row.index) +
    same(raw.title, title) + same(raw.artist, artist) + same(raw.album, album) + same(raw.genre, genre) + same(raw.key, key) + same(raw.source_path, filePath)
  ).toLowerCase();
}

const FUZZ_SEEDS = (globalThis as unknown as { process: { env: Record<string, string | undefined> } }).process.env.FF_FUZZ_FULL ? [1, 2, 3, 4] : [1];

describe('bulk bump copies row fields to the plain tracks like the snapshotting reference', () => {
  for (const seed of FUZZ_SEEDS) {
    it(`seed ${seed}: plain rows and search results stay equal over random writes and bumps`, async () => {
      held.clear();
      const random = generator(seed * 7919);
      const count = 40;
      loaded.tracks = Array.from({ length: count }, (_, i) => randomInfo(random, i + 1));
      await loadState();
      const rows = library.tracks;
      const refRows = libraryPlainRows().map((row) => structuredClone(row)) as Record<string, any>[];
      expect(libraryPlainRows()).toStrictEqual(refRows);

      for (let step = 0; step < 120; step++) {
        const bulk = random() < 0.25;
        const targets = bulk
          ? rows.filter(() => random() < 0.7)
          : Array.from({ length: 1 + Math.floor(random() * 4) }, () => rows[Math.floor(random() * rows.length)]);
        for (const row of targets) {
          const writes = 1 + Math.floor(random() * 4);
          for (let w = 0; w < writes; w++) writeRandom(random, row);
        }
        if (random() < 0.8) {
          const ids = targets.map((row) => row.id);
          if (random() < 0.3) ids.push(...ids.slice(0, 3), 99999, rows[Math.floor(random() * rows.length)].id);
          bumpLibraryTracks(ids);
          referenceCopy(refRows, rows, ids);
        }
        expect(libraryPlainRows()).toStrictEqual(refRows);
        for (const query of [WORDS[Math.floor(random() * WORDS.length)] + ' ' + String(Math.floor(random() * 4)), WORDS[Math.floor(random() * WORDS.length)]]) {
          const terms = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
          const expected = refRows.filter((r) => terms.every((t) => haystackOf(r).includes(t))).map((r) => r.id);
          expect(filterTracks(library.tracks, query).map((r: any) => r.id)).toEqual(expected);
        }
      }
    }, 120_000);
  }
});
