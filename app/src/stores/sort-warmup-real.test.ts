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

import { filterTracks, library, loadState, tracksForSidebarSource } from './library.svelte.ts';
import { clearSort, setSort, sortTracks, startLibrarySortWarmup } from './table-columns.svelte.ts';

const DEFAULT_KEYS = ['title', 'artist', 'label', 'bpm', 'key', 'time'];
const KEYS = ['title', 'artist', 'album', 'label', 'genre', 'year', 'rating', 'color', 'bpm', 'key', 'time'];

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

function view(query: string) {
  return sortTracks(filterTracks(tracksForSidebarSource('All Tracks'), query), 'library');
}

function hashesOfEveryOrder(): Record<string, string> {
  const out: Record<string, string> = {};
  for (const key of KEYS) {
    for (const query of ['', 'house', 'a']) {
      clearSort('library');
      setSort('library', key);
      const asc = view(query);
      out[`${key} asc "${query}"`] = `${asc.length}/${ids(asc)}`;
      setSort('library', key);
      const desc = view(query);
      out[`${key} desc "${query}"`] = `${desc.length}/${ids(desc)}`;
    }
  }
  clearSort('library');
  return out;
}

const cpuMs = () => {
  const usage = (globalThis as unknown as { process: { cpuUsage(): { user: number; system: number } } }).process.cpuUsage();
  return (usage.user + usage.system) / 1000;
};
const median = (values: number[]) => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];

describe.skipIf(!STATE)('sort warm-up on the real payload', () => {
  it('hashes every sorted order cold and warmed, and times the warm-up per column', async () => {
    const timings = new Map<string, number[]>();
    let warm: Record<string, string> = {};
    for (let repeat = 0; repeat < REPEATS; repeat++) {
      await loadState();
      clearSort('library');
      for (const [label, keys] of [['default six columns', DEFAULT_KEYS], ['all eleven columns', KEYS]] as const) {
        if (label === 'all eleven columns') {
          await loadState();
          clearSort('library');
        }
        const warmup = startLibrarySortWarmup(keys);
        const cpu0 = cpuMs();
        while (!warmup.step(1000)) {}
        (timings.get(label) ?? timings.set(label, []).get(label)!).push(cpuMs() - cpu0);
      }
      if (repeat === 0) warm = hashesOfEveryOrder();
    }

    await loadState();
    const cold = hashesOfEveryOrder();
    const report = {
      tracks: library.tracks.length,
      warmMs: Object.fromEntries([...timings].map(([key, values]) => [key, { median: Math.round(median(values) * 10) / 10, min: Math.round(Math.min(...values) * 10) / 10 }])),
      coldEqualsWarm: Object.keys(cold).every((name) => cold[name] === warm[name]),
      hashes: cold,
      warmHashes: warm,
    };
    const text = JSON.stringify(report, null, 1);
    if (OUT) writeFileSync(OUT, text);
    else console.log(text);
  }, 900_000);
});
