import type { TrackInfo } from '../services/tauri.svelte.ts';
import { bumpLibraryTracks, filterTracks, importDroppedTracks, library, loadState, librarySearchTexts, tracksForSidebarSource } from './library.svelte.ts';
import { clearSort, setSort, sortTracks, startLibrarySortWarmup } from './table-columns.svelte.ts';

export function lcg(seed: number) {
  let state = seed >>> 0;
  return () => {
    state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
    return state / 0x100000000;
  };
}

const WORDS = ['house', 'dub', 'amor', 'bells', 'zeta', 'Ärger', 'lagos', 'ghosts', 'night', 'Éclair', '日本', 'x1', ''];

export function randomTrackInfo(random: () => number, id: number, folder: string): TrackInfo {
  const pick = () => WORDS[Math.floor(random() * WORDS.length)];
  return {
    id,
    source_path: `${folder}/t${id}.mp3`,
    title: `${pick()} ${pick()}`.trim(),
    artist: pick(),
    album: random() < 0.3 ? '' : pick(),
    genre: random() < 0.3 ? '' : pick(),
    tempo: random() < 0.2 ? 0 : (90 + Math.floor(random() * 60)) * 100,
    key: random() < 0.2 ? '' : `${1 + Math.floor(random() * 12)}${random() < 0.5 ? 'A' : 'B'}`,
    duration_secs: random() < 0.1 ? 0 : 60 + Math.floor(random() * 500),
    bitrate: 320,
    file_size: 1,
    has_artwork: random() < 0.5,
    has_cues: false,
    has_analysis: random() < 0.7,
    analysis_pending: false,
  };
}

export function hashList(items: readonly (string | number)[]): string {
  let a = 0x811c9dc5;
  let b = 0x9e3779b9;
  for (const item of items) {
    const text = String(item);
    for (let k = 0; k < text.length; k++) {
      a = Math.imul(a ^ text.charCodeAt(k), 0x01000193) >>> 0;
      b = Math.imul(b + text.charCodeAt(k) + 1, 0x85ebca6b) >>> 0;
      b ^= b >>> 13;
    }
    a = Math.imul(a ^ 0x1f, 0x01000193) >>> 0;
  }
  return a.toString(16).padStart(8, '0') + b.toString(16).padStart(8, '0');
}

export type AppendOp =
  | { kind: 'append'; batch: TrackInfo[] }
  | { kind: 'bump'; ids: number[]; seed: number }
  | { kind: 'check' };

export interface AppendHarness {
  load(tracks: TrackInfo[]): Promise<void>;
  land(batch: TrackInfo[]): Promise<void>;
  mutate(ids: number[], seed: number): void;
  hashes(): Record<string, string>;
  warm(): void;
}

/**
 * Lands `ops` on a loaded library batch by batch, with columns warmed first or not, and compares the hashes at every
 * `check` with a library loaded whole with the same rows and the same bumps. Returns the names of the hashes that differ.
 */
export async function appendScenario(harness: AppendHarness, initial: TrackInfo[], ops: AppendOp[], warmFirst: boolean): Promise<{ checks: number; mismatches: string[] }> {
  const whole: Record<string, string>[] = [];
  const rows = [...initial];
  const replayed: AppendOp[] = [];
  for (const op of ops) {
    if (op.kind === 'append') rows.push(...op.batch);
    replayed.push(op);
    if (op.kind !== 'check') continue;
    await harness.load(rows);
    for (const earlier of replayed) if (earlier.kind === 'bump') harness.mutate(earlier.ids, earlier.seed);
    whole.push(harness.hashes());
  }
  await harness.load(initial);
  if (warmFirst) harness.warm();
  harness.hashes();
  const mismatches: string[] = [];
  let checks = 0;
  for (const op of ops) {
    if (op.kind === 'append') await harness.land(op.batch);
    else if (op.kind === 'bump') harness.mutate(op.ids, op.seed);
    else {
      const incremental = harness.hashes();
      const reference = whole[checks++];
      for (const name of Object.keys(reference)) if (incremental[name] !== reference[name]) mismatches.push(`check ${checks}: ${name}`);
    }
  }
  return { checks, mismatches };
}

export const KEYS = ['title', 'artist', 'album', 'label', 'genre', 'year', 'rating', 'color', 'bpm', 'key', 'time', 'index'];
const QUERIES = ['', 'house', 'a', 'night x'];

function view(query: string) {
  return sortTracks(filterTracks(tracksForSidebarSource('All Tracks'), query), 'library');
}

function hashes(): Record<string, string> {
  const out: Record<string, string> = {};
  for (const key of KEYS) {
    for (const query of QUERIES) {
      clearSort('library');
      setSort('library', key);
      const asc = view(query);
      out[`${key} asc "${query}"`] = `${asc.length}/${hashList(asc.map((row) => row.id))}`;
      setSort('library', key);
      const desc = view(query);
      out[`${key} desc "${query}"`] = `${desc.length}/${hashList(desc.map((row) => row.id))}`;
    }
  }
  clearSort('library');
  const plain = view('');
  out['unsorted'] = `${plain.length}/${hashList(plain.map((row) => row.id))}`;
  const texts = librarySearchTexts();
  out['search text'] = `${texts.length}/${hashList(texts)}`;
  return out;
}

const MUTATION_WORDS = ['house', 'dub', 'night', 'zeta', 'Ärger', 'a'];

function mutate(ids: number[], seed: number) {
  const wanted = new Set(ids);
  for (const row of library.tracks) {
    if (!wanted.has(row.id)) continue;
    const random = lcg(seed * 7919 + row.id);
    row.bpm = (80 + Math.floor(random() * 900) / 10).toFixed(1);
    row.bpmExact = Number(row.bpm);
    row.key = `${1 + Math.floor(random() * 12)}${random() < 0.5 ? 'A' : 'B'}`;
    if (random() < 0.3) row.artist = `${MUTATION_WORDS[Math.floor(random() * MUTATION_WORDS.length)]} ${Math.floor(random() * 20)}`;
    if (random() < 0.2) row.title = `${MUTATION_WORDS[Math.floor(random() * MUTATION_WORDS.length)]} ${Math.floor(random() * 500)}`;
    if (random() < 0.1) row.rating = Math.floor(random() * 6);
  }
  bumpLibraryTracks(ids);
}

/** `hooks` is what the test's mocked backend answers `load_state` and `commit_import` with. */
export function createHarness(hooks: { state: { tracks: unknown[]; playlists: unknown[] }; added: unknown[] }): AppendHarness {
  return {
    async load(tracks) {
      hooks.state = { tracks: tracks as unknown[], playlists: [] };
      await loadState();
      clearSort('library');
    },
    async land(batch) {
      hooks.added = batch;
      await importDroppedTracks(['/Downloads/Crate']);
    },
    mutate,
    hashes,
    warm() {
      const warmup = startLibrarySortWarmup(KEYS);
      while (!warmup.step(1000)) {}
    },
  };
}
