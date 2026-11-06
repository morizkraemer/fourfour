import { readFileSync, writeFileSync } from 'node:fs';
import { PerformanceObserver } from 'node:perf_hooks';
import { describe, it, vi } from 'vitest';

const env = (globalThis as unknown as { process: { env: Record<string, string | undefined> } }).process.env;
const STATE = env.FF_PERF_STATE;
const OUT = env.FF_PERF_OUT;
const BUMPS = Number(env.FF_PERF_BUMPS ?? 300);
const PER_BUMP = Number(env.FF_PERF_PER_BUMP ?? 2);

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

import * as store from './library.svelte.ts';
import { clearSort, setSort, sortTracks, startLibrarySortWarmup } from './table-columns.svelte.ts';
import { sameMembers } from '../lib/stableSort.ts';

const { bumpLibraryTracks, filterTracks, library, loadState, tracksForSidebarSource } = store;

function lcg(seed: number) {
  let state = seed >>> 0;
  return () => {
    state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
    return state / 0x100000000;
  };
}

let bumpMs = 0;

function bumpOnce(random: () => number) {
  const changed: number[] = [];
  for (let k = 0; k < PER_BUMP; k++) {
    const row = library.tracks[Math.floor(random() * library.tracks.length)];
    row.bpm = (80 + Math.floor(random() * 900) / 10).toFixed(1);
    row.bpmExact = Number(row.bpm);
    row.key = `${1 + Math.floor(random() * 12)}${random() < 0.5 ? 'A' : 'B'}`;
    changed.push(row.id as number);
  }
  const t0 = performance.now();
  bumpLibraryTracks(changed);
  bumpMs += performance.now() - t0;
}

interface Previous {
  rows: unknown[];
  members: Set<unknown> | null;
}

describe.skipIf(!STATE)('per-bump cost while a sort is active', () => {
  it('measures bump plus the sorted view per bump', async () => {
    vi.spyOn(console, 'warn').mockImplementation(() => {});
    await loadState();
    clearSort('library');
    const keys = ['title', 'artist', 'album', 'label', 'genre', 'year', 'rating', 'color', 'bpm', 'key', 'time'];
    const warmup = startLibrarySortWarmup(keys);
    while (!warmup.step(1000)) {}
    const sameLibraryMembers = (store as unknown as Record<string, ((a: unknown, b: unknown) => boolean) | undefined>).sameLibraryMembers;
    const results: Record<string, number> = {};
    let gcMs = 0;
    let gcCount = 0;
    const observer = new PerformanceObserver((list) => {
      for (const entry of list.getEntries()) {
        gcMs += entry.duration;
        gcCount++;
      }
    });
    observer.observe({ entryTypes: ['gc'] });
    for (const [label, key, filter] of [['sorted artist', 'artist', ''], ['sorted bpm', 'bpm', ''], ['sorted artist, filter "house"', 'artist', 'house']] as const) {
      clearSort('library');
      setSort('library', key);
      let previous: Previous | null = null;
      const phase = { bump: 0, filter: 0, same: 0, sort: 0 };
      const view = () => {
        const t0 = performance.now();
        const rows = filterTracks(tracksForSidebarSource('All Tracks'), filter);
        const t1 = performance.now();
        const same =
          previous !== null &&
          ((sameLibraryMembers !== undefined && sameLibraryMembers(previous.rows, rows)) ||
            sameMembers((previous.members ??= new Set(previous.rows)), rows));
        const t2 = performance.now();
        const result = sortTracks(rows, 'library');
        phase.filter += t1 - t0;
        phase.same += t2 - t1;
        phase.sort += performance.now() - t2;
        previous = { rows, members: same ? previous!.members : null };
        return result;
      };
      view();
      const random = lcg(139);
      for (let i = 0; i < 40; i++) {
        bumpOnce(random);
        view();
      }
      phase.filter = phase.same = phase.sort = 0;
      bumpMs = 0;
      await new Promise((r) => setTimeout(r, 50));
      gcMs = 0;
      gcCount = 0;
      const samples: number[] = [];
      for (let i = 0; i < BUMPS; i++) {
        const t0 = performance.now();
        bumpOnce(random);
        phase.bump += performance.now() - t0;
        view();
        samples.push(performance.now() - t0);
      }
      await new Promise((r) => setTimeout(r, 50));
      results[`${label} bumpLibraryTracks ms per bump`] = Math.round((bumpMs / BUMPS) * 1000) / 1000;
      for (const [name, total] of Object.entries(phase)) results[`${label} ${name} ms per bump`] = Math.round((total / BUMPS) * 1000) / 1000;
      samples.sort((a, b) => a - b);
      results[`${label} median ms`] = Math.round(samples[samples.length >> 1] * 1000) / 1000;
      results[`${label} mean ms`] = Math.round((samples.reduce((a, b) => a + b, 0) / samples.length) * 1000) / 1000;
      results[`${label} gc ms per bump`] = Math.round((gcMs / BUMPS) * 1000) / 1000;
      results[`${label} gc count`] = gcCount;
    }
    const text = JSON.stringify({ tracks: library.tracks.length, perBump: PER_BUMP, bumps: BUMPS, results }, null, 1);
    if (OUT) writeFileSync(OUT, text);
    else console.log(text);
  }, 600_000);
});
