import { readFileSync, writeFileSync } from 'node:fs';
import { Session } from 'node:inspector';
import { describe, it, vi } from 'vitest';

const env = (globalThis as unknown as { process: { env: Record<string, string | undefined> } }).process.env;
const STATE = env.FF_PERF_STATE;
const OUT = env.FF_PERF_OUT;
const REPEATS = Number(env.FF_PERF_REPEATS ?? 5);
const PROFILE = env.FF_PERF_PROFILE;
const WARM = env.FF_PERF_WARM !== '0';
const SIZES = (env.FF_PERF_PLAYLISTS ?? 'R124 1k,R124 5k,R124 20k').split(',');

const saveCost = { calls: 0, ms: 0 };

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
  invoke: vi.fn(async (command: string, args?: { playlists?: unknown }) => {
    if (command === 'load_state') return JSON.parse(readFileSync(STATE!, 'utf8'));
    if (command === 'save_state') {
      const t0 = performance.now();
      JSON.stringify(args);
      saveCost.calls++;
      saveCost.ms += performance.now() - t0;
    }
    return undefined;
  }),
}));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));

import {
  addTracksToPlaylist,
  bumpLibraryTracks,
  filterTracks,
  library,
  loadState,
  moveTracksToPlaylist,
  playlistByName,
  removeTracksFromPlaylist,
  reorderPlaylistTracks,
} from './library.svelte.ts';
import { openTableView } from './playlist-view-probe.svelte.ts';
import { clearSort, setSort, startLibrarySortWarmup } from './table-columns.svelte.ts';

const WARMED_COLUMNS = ['title', 'artist', 'label', 'bpm', 'key', 'time'];

async function loadWarm() {
  await loadState();
  if (!WARM) return;
  filterTracks(library.tracks, 'zzzz');
  const warmup = startLibrarySortWarmup(WARMED_COLUMNS);
  while (!warmup.step(50));
}

type Row = { id: number };

function idsHash(ids: readonly number[]): string {
  let a = 0x811c9dc5;
  let b = 0x9e3779b9;
  for (const id of ids) {
    a = Math.imul(a ^ id, 0x01000193) >>> 0;
    b = Math.imul(b + id + 1, 0x85ebca6b) >>> 0;
    b ^= b >>> 13;
  }
  return a.toString(16).padStart(8, '0') + b.toString(16).padStart(8, '0');
}

function rowsHash(rows: readonly Row[]): string {
  const ids: number[] = new Array(rows.length);
  for (let i = 0; i < rows.length; i++) ids[i] = rows[i].id;
  return `${rows.length}/${idsHash(ids)}`;
}

const median = (values: number[]) => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];

interface Outcome {
  rows: readonly Row[];
  extra?: () => string;
}

interface Step {
  name: string;
  prepare?: () => void;
  run: () => Promise<Outcome | readonly Row[]> | Outcome | readonly Row[];
}

const COLUMNS = ['title', 'artist', 'album', 'genre', 'time', 'label', 'bpm', 'key', 'index', 'year', 'rating', 'color'];

function script(name: string): Step[] {
  const view = openTableView('All Tracks');
  const steps: Step[] = [];
  const rows = () => view.rows as Row[];
  const playlistIds = () => idsHash([...playlistByName(name)!.track_ids]);
  steps.push({ name: `${name}: show All Tracks`, run: () => (view.show('All Tracks'), rows()) });
  steps.push({ name: `${name}: switch`, run: () => (view.show(name), rows()) });
  steps.push({ name: `${name}: switch away`, run: () => (view.show('All Tracks'), rows()) });
  steps.push({ name: `${name}: switch again`, run: () => (view.show(name), rows()) });
  const typed = (query: string, label: string) => {
    for (let i = 1; i <= query.length; i++) steps.push({ name: `${name}: ${label} '${query.slice(0, i)}'`, run: () => (view.search(query.slice(0, i)), rows()) });
  };
  typed('house dub', 'type');
  for (let i = 1; i <= 4; i++) steps.push({ name: `${name}: backspace ${i}`, run: () => (view.search('house dub'.slice(0, 9 - i)), rows()) });
  steps.push({ name: `${name}: clear`, run: () => (view.search(''), rows()) });
  typed('a', 'type');
  steps.push({ name: `${name}: clear a`, run: () => (view.search(''), rows()) });
  for (const key of COLUMNS) {
    steps.push({
      name: `${name}: sort ${key} asc`,
      run: () => {
        clearSort('playlist');
        setSort('playlist', key);
        return rows();
      },
    });
    steps.push({
      name: `${name}: sort ${key} desc`,
      run: () => {
        setSort('playlist', key);
        return rows();
      },
    });
  }
  steps.push({ name: `${name}: sort artist asc again`, run: () => (clearSort('playlist'), setSort('playlist', 'artist'), rows()) });
  const sortedTyped = (query: string, label: string) => {
    for (let i = 1; i <= query.length; i++) steps.push({ name: `${name}: ${label} '${query.slice(0, i)}'`, run: () => (view.search(query.slice(0, i)), rows()) });
  };
  sortedTyped('house', 'sorted type');
  steps.push({ name: `${name}: sorted backspace 1`, run: () => (view.search('hous'), rows()) });
  steps.push({ name: `${name}: sorted clear`, run: () => (view.search(''), rows()) });
  steps.push({ name: `${name}: sorted switch away`, run: () => (view.show('All Tracks'), rows()) });
  steps.push({ name: `${name}: sorted switch back`, run: () => (view.show(name), rows()) });
  steps.push({ name: `${name}: sorted bump bpm`, run: () => bumpBpm(name, rows) });
  steps.push({ name: `${name}: unsort`, run: () => (clearSort('playlist'), rows()) });

  const members = () => [...playlistByName(name)!.track_ids];
  let planned: number[] = [];
  const plan = (pick: () => number[]) => () => {
    planned = pick();
  };
  const outside = (count: number) => () => {
    const inside = new Set(members());
    return library.tracks.filter((t) => !inside.has(t.id)).slice(0, count).map((t) => t.id);
  };
  steps.push({
    name: `${name}: add 10`,
    prepare: plan(outside(10)),
    run: async () => {
      await addTracksToPlaylist(playlistByName(name)!.id, planned);
      return { rows: rows(), extra: playlistIds };
    },
  });
  steps.push({
    name: `${name}: add 500`,
    prepare: plan(outside(500)),
    run: async () => {
      await addTracksToPlaylist(playlistByName(name)!.id, planned);
      return { rows: rows(), extra: playlistIds };
    },
  });
  steps.push({
    name: `${name}: insert 10 at 100`,
    prepare: plan(outside(10)),
    run: async () => {
      await moveTracksToPlaylist({ trackIds: planned, toPlaylistId: playlistByName(name)!.id, insertIndex: 100, removeFromSource: false });
      return { rows: rows(), extra: playlistIds };
    },
  });
  steps.push({
    name: `${name}: remove 10`,
    prepare: plan(() => members().filter((_, i) => i % 7 === 3).slice(0, 10)),
    run: async () => {
      await removeTracksFromPlaylist(playlistByName(name)!.id, planned);
      return { rows: rows(), extra: playlistIds };
    },
  });
  steps.push({
    name: `${name}: remove 500`,
    prepare: plan(() => members().filter((_, i) => i % 5 === 1).slice(0, 500)),
    run: async () => {
      await removeTracksFromPlaylist(playlistByName(name)!.id, planned);
      return { rows: rows(), extra: playlistIds };
    },
  });
  steps.push({
    name: `${name}: reorder 10 to 50`,
    prepare: plan(() => {
      const current = members();
      const moving = current.filter((_, i) => i % 11 === 5).slice(0, 10);
      const without = current.filter((id) => !moving.includes(id));
      return [...without.slice(0, 50), ...moving, ...without.slice(50)];
    }),
    run: async () => {
      await reorderPlaylistTracks(playlistByName(name)!.id, planned);
      return { rows: rows(), extra: playlistIds };
    },
  });
  steps.push({
    name: `${name}: sorted add 10`,
    prepare: () => {
      setSort('playlist', 'title');
      rows();
      planned = outside(10)();
    },
    run: async () => {
      await addTracksToPlaylist(playlistByName(name)!.id, planned);
      return { rows: rows(), extra: playlistIds };
    },
  });
  steps.push({
    name: `${name}: sorted remove 10`,
    prepare: plan(() => members().filter((_, i) => i % 13 === 4).slice(0, 10)),
    run: async () => {
      await removeTracksFromPlaylist(playlistByName(name)!.id, planned);
      return { rows: rows(), extra: playlistIds };
    },
  });
  steps.push({ name: `${name}: end`, run: () => (clearSort('playlist'), view.show('All Tracks'), rows()) });
  return steps;
}

function bumpBpm(name: string, rows: () => Row[]): Row[] {
  const members = playlistByName(name)!.track_ids;
  const byId = new Map(library.tracks.map((t) => [t.id, t]));
  for (let n = 0; n < 5; n++) {
    const row = byId.get(members[(n * 331 + 17) % members.length])!;
    row.bpm = (90 + n * 7).toFixed(1);
    bumpLibraryTracks([row.id]);
  }
  return rows();
}

describe.skipIf(!STATE)('playlist views on the real payload', () => {
  it('prints step timings and the hash of every result list', async () => {
    const session = PROFILE ? new Session() : null;
    session?.connect();
    const post = (method: string, params?: object) => new Promise<any>((resolve, reject) => session!.post(method, params, (err, result) => (err ? reject(err) : resolve(result))));
    const timings = new Map<string, number[]>();
    const hashes = new Map<string, string>();
    const order: string[] = [];
    for (let repeat = 0; repeat < REPEATS; repeat++) {
      for (const name of SIZES) {
        await loadWarm();
        clearSort('playlist');
        const steps = script(name);
        clearSort('playlist');
        if (session) {
          await post('Profiler.enable');
          await post('Profiler.setSamplingInterval', { interval: 100 });
          await post('Profiler.start');
        }
        for (const step of steps) {
          step.prepare?.();
          const t0 = performance.now();
          const outcome = await step.run();
          const ms = performance.now() - t0;
          const result = 'rows' in outcome ? `${rowsHash(outcome.rows)} ${outcome.extra?.() ?? ''}` : rowsHash(outcome);
          if (!order.includes(step.name)) order.push(step.name);
          (timings.get(step.name) ?? timings.set(step.name, []).get(step.name)!).push(ms);
          if (repeat === 0) hashes.set(step.name, result);
        }
        if (session && repeat === REPEATS - 1) writeFileSync(`${PROFILE}-${name.replace(/\W+/g, '')}.cpuprofile`, JSON.stringify((await post('Profiler.stop')).profile));
        else if (session) await post('Profiler.stop');
      }
    }
    clearSort('playlist');
    const report = {
      saveCalls: saveCost.calls,
      saveStringifyMsPerCall: saveCost.ms / Math.max(1, saveCost.calls),
      steps: order.map((name) => ({ name, ms: median(timings.get(name)!), min: Math.min(...timings.get(name)!), hash: hashes.get(name) })),
    };
    const text = JSON.stringify(report, null, 1);
    if (OUT) writeFileSync(OUT, text);
    else console.log(text);
  }, 900_000);
});
