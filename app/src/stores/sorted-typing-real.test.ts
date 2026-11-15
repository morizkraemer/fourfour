import { readFileSync, writeFileSync } from 'node:fs';
import { describe, it, vi } from 'vitest';

const env = (globalThis as unknown as { process: { env: Record<string, string | undefined> } }).process.env;
const STATE = env.FF_PERF_STATE;
const OUT = env.FF_PERF_OUT;
const REPEATS = Number(env.FF_PERF_REPEATS ?? 5);

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

import { bumpLibraryTracks, filterTracks, library, loadState, tracksForSidebarSource } from './library.svelte.ts';
import { clearSort, setSort, sortTracks, startLibrarySortWarmup } from './table-columns.svelte.ts';

const SORTS: { key: string | null; twice: boolean }[] = [
  { key: null, twice: false },
  { key: 'title', twice: false },
  { key: 'title', twice: true },
  { key: 'artist', twice: false },
  { key: 'artist', twice: true },
  { key: 'bpm', twice: false },
  { key: 'bpm', twice: true },
  { key: 'key', twice: false },
  { key: 'label', twice: false },
];
const QUERIES = ['h', 'ho', 'hou', 'hous', 'house', 'hous', 'hou', 'ho', 'h', '', 'a', 'a ', 'a b', 'a ', 'a', '', 'am', 'amo', 'am', 'a', 'house dub', 'house', 'dub', ''];

function ids(rows: { id: number }[]): string {
  let a = 0x811c9dc5;
  let b = 0x9e3779b9;
  for (const row of rows) {
    a = Math.imul(a ^ row.id, 0x01000193) >>> 0;
    b = Math.imul(b + row.id + 1, 0x85ebca6b) >>> 0;
    b ^= b >>> 13;
  }
  return a.toString(16).padStart(8, '0') + b.toString(16).padStart(8, '0');
}

const median = (values: number[]) => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];

function setUpSort(sort: { key: string | null; twice: boolean }) {
  clearSort('library');
  if (!sort.key) return;
  setSort('library', sort.key);
  if (sort.twice) setSort('library', sort.key);
}

function drainWarmup() {
  const warmup = startLibrarySortWarmup(['title', 'artist', 'album', 'label', 'genre', 'bpm', 'key', 'time']);
  while (!warmup.step(50));
}

describe.skipIf(!STATE)('typing in a sorted view on the real payload', () => {
  it('prints the filter and sort cost of every key and the hash of every result', async () => {
    const hashes: Record<string, string> = {};
    const filterMs = new Map<string, number[]>();
    const sortMs = new Map<string, number[]>();
    const order: string[] = [];
    for (const bumps of [false, true]) {
      for (let repeat = 0; repeat < REPEATS; repeat++) {
        let bumped = 0;
        for (const sort of SORTS) {
          await loadState();
          drainWarmup();
          setUpSort(sort);
          const label = `${sort.key ?? 'none'}${sort.twice ? ' desc' : ''}${bumps ? ' +bumps' : ''}`;
          filterTracks(tracksForSidebarSource('All Tracks'), 'zzzz');
          sortTracks(tracksForSidebarSource('All Tracks'), 'library');
          for (let i = 0; i < QUERIES.length; i++) {
            if (bumps) {
              const rows = [0, 1].map((k) => library.tracks[((bumped++ * 2 + k) * 7919) % library.tracks.length]);
              for (const row of rows) row.bpm = (90 + (bumped % 80)).toFixed(1);
              bumpLibraryTracks(rows.map((row) => row.id));
            }
            const t0 = performance.now();
            const filtered = filterTracks(tracksForSidebarSource('All Tracks'), QUERIES[i]);
            const t1 = performance.now();
            const sorted = sortTracks(filtered, 'library');
            const t2 = performance.now();
            const name = `${label} ${i} "${QUERIES[i]}"`;
            if (!order.includes(name)) order.push(name);
            (filterMs.get(name) ?? filterMs.set(name, []).get(name)!).push(t1 - t0);
            (sortMs.get(name) ?? sortMs.set(name, []).get(name)!).push(t2 - t1);
            if (repeat === 0) hashes[name] = `${sorted.length}/${ids(sorted)}`;
          }
        }
        clearSort('library');
      }
    }
    const rows = order.map((name) => ({ name, filter: median(filterMs.get(name)!), sort: median(sortMs.get(name)!), hash: hashes[name] }));
    if (OUT) writeFileSync(OUT, JSON.stringify({ library: library.tracks.length, rows }, null, 1));
    const line = (r: { name: string; filter: number; sort: number }) => `${r.name.padEnd(36)} filter ${r.filter.toFixed(1).padStart(6)} sort ${r.sort.toFixed(1).padStart(6)}`;
    console.log(rows.filter((r) => r.filter + r.sort > 4).map(line).join('\n'));
  }, 600_000);
});
