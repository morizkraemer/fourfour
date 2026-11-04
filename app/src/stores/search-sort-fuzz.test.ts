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

import { bumpLibraryTracks, filterTracks, library, libraryPositionsOf, loadState } from './library.svelte.ts';
import { clearSort, setSort, sortTracks } from './table-columns.svelte.ts';

function generator(seed: number) {
  let state = seed;
  return () => {
    state = (state * 1664525 + 1013904223) >>> 0;
    return state / 2 ** 32;
  };
}

const WORDS = ['alpha', 'beta', 'gamma', 'delta', 'dub', 'deep', 'house', 'Zed', 'amor', 'bells', 'ghosts', 'x1', ''];
const KEYS = ['title', 'bpm', 'artist'] as const;
type Key = (typeof KEYS)[number];

type Row = { id: number; title: string; artist: string; bpm: string; album: string; genre: string; key: string; label: string; time: string; filePath: string; index: string; raw: Record<string, string> };

function randomInfo(random: () => number, id: number) {
  const pick = () => WORDS[Math.floor(random() * WORDS.length)];
  return {
    id,
    source_path: `/m/${id}.mp3`,
    title: `${pick()} ${pick()}`.trim(),
    artist: pick(),
    album: '',
    genre: '',
    tempo: random() < 0.15 ? 0 : (90 + Math.floor(random() * 60)) * 100,
    key: '',
    duration_secs: 200,
    bitrate: 320,
    file_size: 1,
    has_artwork: false,
    has_cues: false,
    has_analysis: true,
    analysis_pending: false,
  };
}

async function loadRandomLibrary(random: () => number, count: number) {
  loaded.tracks = Array.from({ length: count }, (_, i) => randomInfo(random, i + 1));
  await loadState();
}

function referenceMatches(row: Row, query: string): boolean {
  const fields = [row.title, row.artist, row.album, row.label, row.genre, row.key, row.bpm, row.time, row.filePath, row.index, row.raw.title, row.raw.artist, row.raw.album, row.raw.genre, row.raw.key, row.raw.source_path];
  return query
    .trim()
    .toLowerCase()
    .split(/\s+/)
    .filter(Boolean)
    .every((term) => fields.some((field) => String(field ?? '').toLowerCase().includes(term)));
}

function referenceValue(row: Row, key: Key): string | number | null {
  const text = String(row[key] ?? '').trim();
  if (text === '' || text === '—' || text === 'undefined') return null;
  return key === 'bpm' ? parseFloat(text) : text.toLowerCase();
}

function referenceSorted(rows: Row[], key: Key, dir: 1 | -1): Row[] {
  return [...rows].sort((a, b) => {
    const x = referenceValue(a, key);
    const y = referenceValue(b, key);
    if (x === null && y === null) return 0;
    if (x === null) return 1;
    if (y === null) return -1;
    return x < y ? -dir : x > y ? dir : 0;
  });
}

describe('filter and sort of the library against a plain reference', () => {
  it('stay equal through typing, backspacing, edits, bumps and sort changes', async () => {
    const random = generator(99);
    const count = 120;
    await loadRandomLibrary(random, count);
    const current = () => library.tracks as Row[];
    let query = '';
    let target = 'alpha';
    let typed = 0;
    let second = '';
    let sort: { key: Key; dir: 1 | -1 } | null = null;
    const log: string[] = [];

    for (let step = 0; step < 600; step++) {
      const r = random();
      if (r < 0.45) {
        const move = random();
        if (move < 0.55) typed = Math.min(target.length, typed + 1);
        else if (move < 0.85) typed = Math.max(0, typed - 1);
        else if (move < 0.93) {
          target = WORDS[Math.floor(random() * (WORDS.length - 1))];
          typed = 1 + Math.floor(random() * 2);
        } else second = second ? '' : WORDS[Math.floor(random() * (WORDS.length - 1))].slice(0, 2);
        query = `${target.slice(0, typed)}${second ? ' ' + second : ''}`;
        log.push(`query "${query}"`);
      } else if (r < 0.75) {
        const ids: number[] = [];
        for (let n = 0; n < 1 + Math.floor(random() * 3); n++) {
          const row = current()[Math.floor(random() * count)];
          const fresh = randomInfo(random, row.id);
          row.title = fresh.title;
          row.bpm = fresh.tempo ? (fresh.tempo / 100).toFixed(1) : '—';
          row.artist = fresh.artist;
          ids.push(row.id);
        }
        bumpLibraryTracks(ids);
        log.push(`bump ${ids.join(',')}`);
      } else if (r < 0.9) {
        const key = KEYS[Math.floor(random() * KEYS.length)];
        clearSort('library');
        setSort('library', key);
        const dir = random() < 0.5 ? 1 : -1;
        if (dir === -1) setSort('library', key);
        sort = { key, dir };
        log.push(`sort ${key} ${dir}`);
      } else if (r < 0.95) {
        clearSort('library');
        sort = null;
        log.push('unsort');
      }

      const filtered = filterTracks(library.tracks, query);
      const shown = sort ? sortTracks(filtered, 'library') : filtered;
      const matching = current().filter((row) => referenceMatches(row, query));
      const expected = sort ? referenceSorted(matching, sort.key, sort.dir) : matching;
      expect(shown.map((row) => row.id), `step ${step}: ${log.slice(-6).join(' | ')}`).toEqual(expected.map((row) => row.id));
      if (filtered !== library.tracks) {
        const positions = libraryPositionsOf(filtered) as number[];
        expect(positions.map((p) => current()[p].id)).toEqual(matching.map((row) => row.id));
      }
    }
    clearSort('library');
  });

  it('knows result positions only for results of the current library list', async () => {
    loaded.tracks = Array.from({ length: 5 }, (_, i) => ({ ...randomInfo(() => 0.5, i + 1), title: i % 2 ? 'odd' : 'even' }));
    await loadState();
    const first = filterTracks(library.tracks, 'odd');
    expect(libraryPositionsOf(first)).toEqual([1, 3]);
    expect(libraryPositionsOf(library.tracks)).toBe('all');
    expect(libraryPositionsOf([...first])).toBeNull();
    bumpLibraryTracks([1]);
    expect(libraryPositionsOf(first)).toBeNull();
  });

  it('keeps the results of other source lists apart', async () => {
    loaded.tracks = Array.from({ length: 6 }, (_, i) => ({ ...randomInfo(() => 0.5, i + 1), title: `t${i}`, artist: '' }));
    await loadState();
    const playlist = [library.tracks[4], library.tracks[1], library.tracks[3]];
    expect(filterTracks(playlist, 't4').map((r) => r.id)).toEqual([5]);
    expect(filterTracks(library.tracks, 't1').map((r) => r.id)).toEqual([2]);
    expect(filterTracks(playlist, 't1').map((r) => r.id)).toEqual([2]);
    expect(libraryPositionsOf(filterTracks(playlist, 't1'))).toBeNull();
    expect(filterTracks(playlist, 't')).toBe(playlist);
  });
});
