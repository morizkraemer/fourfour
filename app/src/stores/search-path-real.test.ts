import { readFileSync, writeFileSync } from 'node:fs';
import { describe, it, vi } from 'vitest';

const env = (globalThis as unknown as { process: { env: Record<string, string | undefined> } }).process.env;
const STATE = env.FF_PERF_STATE;
const OUT = env.FF_PERF_OUT;
const REPEATS = Number(env.FF_PERF_REPEATS ?? 7);

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
import { clearSort, setSort, sortTracks } from './table-columns.svelte.ts';

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

type Script = { name: string; run: () => { id: number }[] }[];

function typed(query: string, sorted: boolean, label: string): Script {
  const steps: Script = [];
  for (let i = 1; i <= query.length; i++) steps.push({ name: `${label} '${query.slice(0, i)}'`, run: () => view(query.slice(0, i), sorted) });
  return steps;
}

function view(query: string, sorted: boolean) {
  const filtered = filterTracks(tracksForSidebarSource('All Tracks'), query);
  return sorted ? sortTracks(filtered, 'library') : filtered;
}

function script(): Script {
  const steps: Script = [];
  steps.push(...typed('ghosts of lagos', false, 'type'));
  const query = 'ghosts of lagos';
  for (let i = 1; i <= 6; i++) steps.push({ name: `backspace ${i}`, run: () => view(query.slice(0, query.length - i), false) });
  steps.push({ name: 'clear', run: () => view('', false) });
  steps.push(...typed('house', false, 'type'));
  steps.push({ name: 'clear', run: () => view('', false) });
  for (const [key, label] of [['artist', 'artist asc'], ['artist', 'artist desc'], ['title', 'title asc'], ['bpm', 'bpm asc'], ['key', 'key asc']] as const) {
    steps.push({
      name: `sort ${label}`,
      run: () => {
        if (label.endsWith('asc')) {
          clearSort('library');
          setSort('library', key);
        } else setSort('library', key);
        return view('', true);
      },
    });
  }
  clearSort('library');
  setSort('library', 'artist');
  steps.push({ name: 'sort artist asc again', run: () => view('', true) });
  steps.push(...typed('amor bells', true, 'sorted type'));
  const sortedQuery = 'amor bells';
  for (let i = 1; i <= 4; i++) steps.push({ name: `sorted backspace ${i}`, run: () => view(sortedQuery.slice(0, sortedQuery.length - i), true) });
  steps.push({ name: 'sorted clear', run: () => view('', true) });
  steps.push({ name: 'sorted type a', run: () => view('a', true) });
  steps.push({ name: 'sorted type a, backspace', run: () => view('', true) });
  steps.push({ name: 'sorted house', run: () => view('house', true) });
  steps.push({ name: 'sorted house + dub', run: () => view('house dub', true) });
  steps.push({ name: 'sorted dub', run: () => view('dub', true) });
  steps.push({ name: 'sorted clear 2', run: () => view('', true) });
  clearSort('library');
  for (const key of ['album', 'genre', 'time', 'label', 'index', 'year', 'rating', 'color', 'title', 'artist', 'bpm', 'key']) {
    steps.push({
      name: `column ${key} asc`,
      run: () => {
        clearSort('library');
        setSort('library', key);
        return view('', true);
      },
    });
    steps.push({
      name: `column ${key} desc`,
      run: () => {
        setSort('library', key);
        return view('', true);
      },
    });
    steps.push({ name: `column ${key} desc, filter house`, run: () => view('house', true) });
  }
  clearSort('library');
  return steps;
}

describe.skipIf(!STATE)('search and sort data path on the real payload', () => {
  it('prints step timings and the hash of every result list', async () => {
    await loadState();
    const tracks = library.tracks.length;
    const timings = new Map<string, number[]>();
    const hashes = new Map<string, string>();
    const order: string[] = [];

    for (let repeat = 0; repeat < REPEATS; repeat++) {
      if (repeat > 0) await loadState();
      clearSort('library');
      const steps = script();
      clearSort('library');
      filterTracks(tracksForSidebarSource('All Tracks'), 'zzzz');
      for (const step of steps) {
        const t0 = performance.now();
        const result = step.run();
        const ms = performance.now() - t0;
        if (!order.includes(step.name)) order.push(step.name);
        (timings.get(step.name) ?? timings.set(step.name, []).get(step.name)!).push(ms);
        if (repeat === 0) hashes.set(step.name, `${result.length}/${ids(result)}`);
      }
    }

    const bumpSteps: { name: string; ms: number; hash: string }[] = [];
    for (const [label, query, sortKey] of [['bump, no filter, no sort', '', null], ['bump, filter house', 'house', null], ['bump, sorted bpm', '', 'bpm'], ['bump, filter house, sorted bpm', 'house', 'bpm']] as const) {
      clearSort('library');
      if (sortKey) setSort('library', sortKey);
      const samples: number[] = [];
      let last: { id: number }[] = [];
      view(query, !!sortKey);
      for (let n = 0; n < 30; n++) {
        const row = library.tracks[(n * 997) % library.tracks.length];
        row.bpm = (100 + n).toFixed(1);
        row.key = `${(n % 12) + 1}A`;
        const t0 = performance.now();
        bumpLibraryTracks([row.id]);
        last = view(query, !!sortKey);
        samples.push(performance.now() - t0);
      }
      bumpSteps.push({ name: label, ms: median(samples), hash: `${last.length}/${ids(last)}` });
    }
    clearSort('library');

    const report = {
      tracks,
      steps: order.map((name) => ({ name, ms: median(timings.get(name)!), min: Math.min(...timings.get(name)!), hash: hashes.get(name) })),
      bumps: bumpSteps,
    };
    const text = JSON.stringify(report, null, 1);
    if (OUT) writeFileSync(OUT, text);
    else console.log(text);
  }, 600_000);
});
