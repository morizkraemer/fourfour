import { readFileSync, writeFileSync } from 'node:fs';
import { describe, it, vi } from 'vitest';

const env = (globalThis as unknown as { process: { env: Record<string, string | undefined> } }).process.env;
const STATE = env.FF_PERF_STATE;
const OUT = env.FF_PERF_OUT;

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

const KEYS = ['title', 'artist', 'album', 'label', 'genre', 'year', 'rating', 'color', 'bpm', 'key', 'time'];
const QUERIES = ['', 'house', 'a'];

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

function hashColumn(key: string, into: Record<string, string>) {
  for (const query of QUERIES) {
    clearSort('library');
    setSort('library', key);
    const asc = view(query);
    into[`${key} asc "${query}"`] = `${asc.length}/${ids(asc)}`;
    setSort('library', key);
    const desc = view(query);
    into[`${key} desc "${query}"`] = `${desc.length}/${ids(desc)}`;
  }
  clearSort('library');
}

function hashAll(): Record<string, string> {
  const out: Record<string, string> = {};
  for (const key of KEYS) hashColumn(key, out);
  const plain = view('');
  out['unsorted ""'] = `${plain.length}/${ids(plain)}`;
  return out;
}

function lcg(seed: number) {
  let state = seed >>> 0;
  return () => {
    state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
    return state / 0x100000000;
  };
}

const WORDS = ['house', 'dub', 'amor', 'bells', 'zeta', 'Ärger', 'lagos', 'ghosts', 'night'];

function scriptedBumps(count: number, random: () => number, perBump: number) {
  for (let n = 0; n < count; n++) {
    const changed: number[] = [];
    for (let k = 0; k < perBump; k++) {
      const row = library.tracks[Math.floor(random() * library.tracks.length)];
      row.bpm = (80 + Math.floor(random() * 900) / 10).toFixed(1);
      row.bpmExact = Number(row.bpm);
      row.key = `${1 + Math.floor(random() * 12)}${random() < 0.5 ? 'A' : 'B'}`;
      if (random() < 0.2) row.artist = `${WORDS[Math.floor(random() * WORDS.length)]} ${Math.floor(random() * 50)}`;
      if (random() < 0.1) row.title = `${WORDS[Math.floor(random() * WORDS.length)]} ${Math.floor(random() * 5000)}`;
      if (random() < 0.1) row.genre = WORDS[Math.floor(random() * WORDS.length)];
      if (random() < 0.05) row.rating = Math.floor(random() * 6);
      changed.push(row.id as number);
    }
    bumpLibraryTracks(changed);
  }
}

describe.skipIf(!STATE)('sorted orders while analysis bumps rows, real payload', () => {
  it('hashes every column, direction and query after scripted bump steps, fresh and stale', async () => {
    const report: Record<string, string> = {};
    const timings: Record<string, number> = {};

    await loadState();
    clearSort('library');
    const warmup = startLibrarySortWarmup(KEYS);
    while (!warmup.step(1000)) {}
    Object.assign(report, Object.fromEntries(Object.entries(hashAll()).map(([k, v]) => [`warm ${k}`, v])));

    const random = lcg(137);
    for (const [label, bumps, perBump] of [['1 bump', 1, 1], ['3 bumps', 3, 2], ['70 bumps', 70, 1], ['130 bumps', 130, 3], ['500 bumps', 500, 1], ['bulk 5000', 1, 5000]] as const) {
      scriptedBumps(bumps, random, perBump);
      const t0 = performance.now();
      const hashes = hashAll();
      timings[`fresh ${label}`] = Math.round(performance.now() - t0);
      for (const [name, hash] of Object.entries(hashes)) report[`fresh ${label} ${name}`] = hash;
    }

    await loadState();
    clearSort('library');
    const secondWarmup = startLibrarySortWarmup(KEYS);
    while (!secondWarmup.step(1000)) {}
    for (const round of [0, 1]) {
      for (const key of KEYS) {
        scriptedBumps(80, random, 1);
        const t0 = performance.now();
        const hashes: Record<string, string> = {};
        hashColumn(key, hashes);
        timings[`stale round ${round} ${key}`] = Math.round(performance.now() - t0);
        for (const [name, hash] of Object.entries(hashes)) report[`stale round ${round} ${name}`] = hash;
      }
    }

    const text = JSON.stringify({ tracks: library.tracks.length, hashes: report, timings }, null, 1);
    if (OUT) writeFileSync(OUT, text);
    else console.log(text);
  }, 1_800_000);
});
