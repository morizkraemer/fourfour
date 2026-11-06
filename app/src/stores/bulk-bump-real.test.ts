import { Session } from 'node:inspector';
import { readFileSync, writeFileSync } from 'node:fs';
import { describe, it, vi } from 'vitest';

const env = (globalThis as unknown as { process: { env: Record<string, string | undefined> } }).process.env;
const STATE = env.FF_PERF_STATE;
const OUT = env.FF_PERF_OUT;
const SIZES = (env.FF_PERF_SIZES ?? '2000,10000,50000').split(',').map(Number);
const REPEATS = Number(env.FF_PERF_REPEATS ?? 3);
const PROFILE = env.FF_PERF_PROFILE;
const PROFILE_OUT = env.FF_PERF_PROFILE_OUT;

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
  invoke: vi.fn(async (command: string) => (command === 'load_state' ? JSON.parse(readFileSync(STATE!, 'utf8')) : undefined)),
}));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));

import { analyzeTrackIds, bumpLibraryTracks, filterTracks, library, libraryPlainRows, loadState } from './library.svelte.ts';
import { clearSort, setSort, sortTracks } from './table-columns.svelte.ts';

type Hash = [number, number];
const SEED: Hash = [0x811c9dc5, 0x9e3779b9];

function fnv(text: string, seed: Hash = SEED): Hash {
  let [a, b] = seed;
  for (let i = 0; i < text.length; i++) {
    const c = text.charCodeAt(i);
    a = Math.imul(a ^ c, 0x01000193) >>> 0;
    b = Math.imul(b + c + 1, 0x85ebca6b) >>> 0;
    b ^= b >>> 13;
  }
  return [a, b];
}
const hex = ([a, b]: Hash) => (a >>> 0).toString(16).padStart(8, '0') + (b >>> 0).toString(16).padStart(8, '0');

function serialize(value: unknown): string {
  return JSON.stringify(value, (_key, v) =>
    ArrayBuffer.isView(v) ? { type: v.constructor.name, bytes: Buffer.from(v.buffer, v.byteOffset, v.byteLength).toString('base64') } : v,
  );
}

function plainHash(): string {
  let state = SEED;
  for (const row of libraryPlainRows()) state = fnv(serialize(row), state);
  return hex(state);
}

function idsHash(rows: { id: number }[]): string {
  let state = SEED;
  for (const row of rows) state = fnv(String(row.id) + ',', state);
  return `${rows.length}/${hex(state)}`;
}

const SORT_KEYS = ['title', 'artist', 'album', 'genre', 'label', 'bpm', 'key', 'time', 'rating', 'color', 'index', 'year'];
const QUERIES = [...'abcdefghijklmnopqrstuvwxyz0123456789'.split(''), 'house', 'techno 8a', 'smoke', 'analyz', '—', 'relinked', 'regenre'];

function orderHash(): string {
  let state = SEED;
  for (const key of SORT_KEYS) {
    for (const times of [1, 2]) {
      clearSort('library');
      for (let n = 0; n < times; n++) setSort('library', key);
      state = fnv(idsHash(sortTracks(library.tracks, 'library')), state);
    }
  }
  clearSort('library');
  return hex(state);
}

function haystackHash(): string {
  let state = SEED;
  for (const query of QUERIES) state = fnv(idsHash(filterTracks(library.tracks, query)), state);
  return hex(state);
}

function pick(size: number): number[] {
  const rows = library.tracks;
  const stride = Math.max(1, Math.floor(rows.length / size));
  const ids: number[] = [];
  for (let i = 0; i < size && i * stride < rows.length; i++) ids.push(rows[i * stride].id);
  return ids;
}

const rowsById = () => new Map<number, any>(library.tracks.map((t: any) => [t.id, t]));

type Scenario = { name: string; write: (ids: number[]) => void; bump: boolean };

function previewBytes(id: number): Uint8Array {
  return Uint8Array.from({ length: 400 }, (_, i) => (i * 7 + id) % 256);
}

const SCENARIOS: Scenario[] = [
  { name: 'analyze selected (analyzeTrackIds)', write: () => {}, bump: false },
  {
    name: 'analysis results arrive',
    bump: true,
    write(ids) {
      const index = rowsById();
      for (const id of ids) {
        const t = index.get(id);
        t.bpm = (100 + (id % 90)).toFixed(1);
        t.bpmExact = 100 + (id % 90);
        t.key = `${(id % 12) + 1}${id % 2 ? 'A' : 'B'}`;
        t.preview = previewBytes(id);
        t.bandsPreview = undefined;
        t.analyzed = true;
        t.analysisPending = false;
        t.analysisLoaded = true;
      }
    },
  },
  {
    name: 'rating + colour',
    bump: true,
    write(ids) {
      const index = rowsById();
      for (const id of ids) {
        const t = index.get(id);
        t.rating = id % 6;
        t.color = id % 3 ? ['red', 'blue', 'green'][id % 3] : null;
      }
    },
  },
  {
    name: 'genre + raw fields',
    bump: true,
    write(ids) {
      const index = rowsById();
      for (const id of ids) {
        const t = index.get(id);
        t.genre = id % 2 ? 'Regenre House' : 'Regenre Techno';
        t.raw.genre = t.genre;
        t.raw.has_artwork = true;
      }
    },
  },
  {
    name: 'relink paths',
    bump: true,
    write(ids) {
      const index = rowsById();
      for (const id of ids) {
        const t = index.get(id);
        t.filePath = `/relinked/${id}.mp3`;
        t.raw.source_path = t.filePath;
      }
    },
  },
];

const median = (values: number[]) => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];

describe.skipIf(!STATE)('bulk bump on the real payload', () => {
  it('prints timings and identity hashes per step', async () => {
    const report: Record<string, any> = {};
    for (const size of SIZES) {
      const timings = new Map<string, { write: number[]; bump: number[] }>();
      const hashes = new Map<string, { plain: string; order: string; haystack: string }>();
      let rowCount = 0;
      for (let repeat = 0; repeat < REPEATS; repeat++) {
        await loadState();
        const ids = pick(size);
        rowCount = ids.length;
        for (const scenario of SCENARIOS) {
          let writeMs = 0;
          let bumpMs: number;
          if (!scenario.bump) {
            const t0 = performance.now();
            const call = analyzeTrackIds(ids);
            bumpMs = performance.now() - t0;
            await call;
          } else {
            const t0 = performance.now();
            scenario.write(ids);
            writeMs = performance.now() - t0;
            const profiled = PROFILE === scenario.name;
            const session = profiled ? new Session() : null;
            if (session) {
              session.connect();
              session.post('Profiler.enable');
              session.post('Profiler.setSamplingInterval', { interval: 200 });
              session.post('Profiler.start');
            }
            const t1 = performance.now();
            bumpLibraryTracks(ids);
            bumpMs = performance.now() - t1;
            session?.post('Profiler.stop', (_err, result) => writeFileSync(PROFILE_OUT!, JSON.stringify(result.profile)));
            session?.disconnect();
          }
          const entry = timings.get(scenario.name) ?? timings.set(scenario.name, { write: [], bump: [] }).get(scenario.name)!;
          entry.write.push(writeMs);
          entry.bump.push(bumpMs);
          if (repeat === 0) hashes.set(scenario.name, { plain: plainHash(), order: orderHash(), haystack: haystackHash() });
        }
      }
      report[size] = SCENARIOS.map((s) => ({
        step: s.name,
        rows: rowCount,
        writeMs: Math.round(median(timings.get(s.name)!.write) * 10) / 10,
        bumpMs: Math.round(median(timings.get(s.name)!.bump) * 10) / 10,
        ...hashes.get(s.name),
      }));
    }
    const text = JSON.stringify(report, null, 1);
    if (OUT) writeFileSync(OUT, text);
    else console.log(text);
  }, 3_600_000);
});
