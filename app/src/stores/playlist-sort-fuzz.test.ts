import { describe, expect, it, vi } from 'vitest';

const loaded = vi.hoisted(() => ({ tracks: [] as Record<string, unknown>[], playlists: [] as Record<string, unknown>[] }));

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
  invoke: vi.fn(async (command: string) => (command === 'load_state' ? { tracks: loaded.tracks, playlists: JSON.parse(JSON.stringify(loaded.playlists)) } : undefined)),
}));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));

import {
  addTracksToPlaylist,
  bumpLibraryTracks,
  filterTracks,
  library,
  libraryMembersOf,
  loadState,
  moveTracksToPlaylist,
  playlistByName,
  removeTracksFromPlaylist,
  reorderPlaylistTracks,
  tracksForSidebarSource,
} from './library.svelte.ts';
import { camelotRank, clearSort, durationSeconds, setSort, sortTracks, startLibrarySortWarmup } from './table-columns.svelte.ts';

function generator(seed: number) {
  let state = seed;
  return () => {
    state = (state * 1664525 + 1013904223) >>> 0;
    return state / 2 ** 32;
  };
}

const WORDS = ['alpha', 'beta', 'gamma', 'delta', 'dub', 'deep', 'house', 'Zed', 'amor', 'bells', 'Éclair', 'ünder', '日本', ''];
const KEYS = ['title', 'bpm', 'artist', 'album', 'genre', 'key', 'time', 'label', 'index', 'rating', 'fav'] as const;
type Key = (typeof KEYS)[number];

type Row = Record<string, any> & { id: number };

function randomInfo(random: () => number, id: number) {
  const pick = () => WORDS[Math.floor(random() * WORDS.length)];
  return {
    id,
    source_path: `/m/${id}.mp3`,
    title: `${pick()} ${pick()}`.trim(),
    artist: pick(),
    album: random() < 0.3 ? '' : pick(),
    genre: random() < 0.3 ? '' : pick(),
    tempo: random() < 0.15 ? 0 : (90 + Math.floor(random() * 6)) * 100,
    key: random() < 0.2 ? '' : `${1 + Math.floor(random() * 4)}${random() < 0.5 ? 'A' : 'B'}`,
    duration_secs: random() < 0.1 ? 0 : 60 + Math.floor(random() * 4) * 30,
    bitrate: 320,
    file_size: 1,
    has_artwork: false,
    has_cues: false,
    has_analysis: true,
    analysis_pending: false,
    rating: Math.floor(random() * 3),
  };
}

function shuffled(random: () => number, ids: number[]): number[] {
  const out = [...ids];
  for (let i = out.length - 1; i > 0; i--) {
    const j = Math.floor(random() * (i + 1));
    [out[i], out[j]] = [out[j], out[i]];
  }
  return out;
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
  if (key === 'fav') return typeof row.fav === 'number' && row.fav > 0 ? row.fav : null;
  const text = String(row[key] ?? '').trim();
  if (text === '' || text === '—' || text === 'undefined') return null;
  if (key === 'bpm') return parseFloat(text);
  if (key === 'key') return camelotRank(text);
  if (key === 'time') return durationSeconds(text);
  if (key === 'index') return parseInt(text, 10);
  if (key === 'rating') return parseInt(text, 10) > 0 ? parseInt(text, 10) : null;
  return text.toLowerCase();
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

async function loadRandomLibrary(random: () => number, count: number) {
  loaded.tracks = Array.from({ length: count }, (_, i) => randomInfo(random, i + 1));
  const ids = loaded.tracks.map((t) => t.id as number);
  loaded.playlists = [
    { id: 1, name: 'many', track_ids: shuffled(random, ids).slice(0, Math.floor(count * 0.6)), parent_id: 0, is_folder: false },
    { id: 2, name: 'dupes', track_ids: [...shuffled(random, ids).slice(0, 30), ...shuffled(random, ids).slice(0, 10), 99999], parent_id: 0, is_folder: false },
    { id: 3, name: 'all', track_ids: shuffled(random, ids), parent_id: 0, is_folder: false },
    { id: 4, name: 'few', track_ids: shuffled(random, ids).slice(0, 5), parent_id: 0, is_folder: false },
    { id: 5, name: 'empty', track_ids: [], parent_id: 0, is_folder: false },
    { id: 6, name: 'stale', track_ids: [...shuffled(random, ids).slice(0, 40), 5000, 5001], parent_id: 0, is_folder: false },
  ];
  await loadState();
}

describe('playlist rows, filter and sort against a plain reference', () => {
  it('stay equal through switching, typing, edits, bumps, sort changes and reloads', async () => {
    const random = generator(2024);
    const count = 150;
    await loadRandomLibrary(random, count);
    const current = () => library.tracks as Row[];
    const names = ['many', 'dupes', 'all', 'few', 'empty', 'stale'];
    let name = 'many';
    let query = '';
    let target = 'alpha';
    let typed = 0;
    let sort: { key: Key; dir: 1 | -1 } | null = null;
    const log: string[] = [];
    const warmups: ReturnType<typeof startLibrarySortWarmup>[] = [];
    const idsOf = (playlist: string) => [...playlistByName(playlist)!.track_ids];

    for (let step = 0; step < 1500; step++) {
      const r = random();
      if (r < 0.3) {
        const move = random();
        if (move < 0.6) typed = Math.min(target.length, typed + 1);
        else if (move < 0.9) typed = Math.max(0, typed - 1);
        else {
          target = WORDS[Math.floor(random() * (WORDS.length - 1))];
          typed = 1 + Math.floor(random() * 2);
        }
        query = target.slice(0, typed);
        log.push(`query "${query}"`);
      } else if (r < 0.45) {
        name = names[Math.floor(random() * names.length)];
        log.push(`show ${name}`);
      } else if (r < 0.6) {
        const ids: number[] = [];
        for (let n = 0; n < 1 + Math.floor(random() * 3); n++) {
          const row = current()[Math.floor(random() * count)];
          const fresh = randomInfo(random, row.id);
          row.title = fresh.title;
          row.bpm = fresh.tempo ? (fresh.tempo / 100).toFixed(1) : '—';
          row.artist = fresh.artist;
          row.key = fresh.key || '—';
          ids.push(row.id);
        }
        bumpLibraryTracks(ids);
        log.push(`bump ${ids.join(',')}`);
      } else if (r < 0.75) {
        const key = KEYS[Math.floor(random() * KEYS.length)];
        clearSort('playlist');
        setSort('playlist', key);
        const dir = random() < 0.5 ? 1 : -1;
        if (dir === -1) setSort('playlist', key);
        sort = { key, dir };
        log.push(`sort ${key} ${dir}`);
      } else if (r < 0.8) {
        clearSort('playlist');
        sort = null;
        log.push('unsort');
      } else if (r < 0.9) {
        const edit = random();
        const playlist = playlistByName(name)!;
        const inside = idsOf(name);
        const some = (from: number[], n: number) => shuffled(random, from).slice(0, n);
        const everyId = current().map((row) => row.id);
        if (edit < 0.3) {
          const ids = some(everyId, 1 + Math.floor(random() * 6));
          await addTracksToPlaylist(playlist.id, ids);
          log.push(`add ${ids.join(',')}`);
        } else if (edit < 0.55) {
          const ids = some(inside, 1 + Math.floor(random() * 6));
          await removeTracksFromPlaylist(playlist.id, ids);
          log.push(`remove ${ids.join(',')}`);
        } else if (edit < 0.8) {
          const ordered = shuffled(random, inside);
          await reorderPlaylistTracks(playlist.id, ordered);
          log.push('reorder');
        } else {
          const ids = some(everyId, 1 + Math.floor(random() * 4));
          await moveTracksToPlaylist({ trackIds: ids, toPlaylistId: playlist.id, insertIndex: Math.floor(random() * 10), removeFromSource: false });
          log.push(`insert ${ids.join(',')}`);
        }
      } else if (r < 0.95) {
        warmups.push(startLibrarySortWarmup(KEYS.filter((k) => k !== 'fav' && random() < 0.6), { read: 1 + Math.floor(random() * 40), sort: 1 + Math.floor(random() * 30) }));
        log.push('warmup');
      } else if (r < 0.97) {
        await loadState();
        log.push('reload');
      }
      for (const warmup of warmups) for (let n = 0; n < Math.floor(random() * 4); n++) warmup.step(0);

      const base = tracksForSidebarSource(name);
      const filtered = filterTracks(base, query);
      const shown = sort ? sortTracks(filtered, 'playlist') : filtered;
      const byId = new Map(current().map((row) => [row.id, row]));
      const rows = idsOf(name).flatMap((id) => (byId.has(id) ? [byId.get(id)!] : []));
      const matching = rows.filter((row) => referenceMatches(row, query));
      const expected = sort ? referenceSorted(matching, sort.key, sort.dir) : matching;
      const context = `step ${step} on ${name}: ${log.slice(-6).join(' | ')}`;
      expect(shown.map((row) => row.id), context).toEqual(expected.map((row) => row.id));
      expect(base.map((row) => row.id), context).toEqual(rows.map((row) => row.id));
      expect(filtered.map((row) => row.id), context).toEqual(matching.map((row) => row.id));
      const members = libraryMembersOf(filtered);
      if (members) expect(Array.from(members, (p) => current()[p].id), context).toEqual(matching.map((row) => row.id));
      if (name !== 'dupes') expect(members, context).not.toBeNull();
    }
    clearSort('playlist');
  });

  it('returns the same rows while neither the ids nor a member changed', async () => {
    const random = generator(7);
    await loadRandomLibrary(random, 60);
    const first = tracksForSidebarSource('many');
    expect(tracksForSidebarSource('many')).toBe(first);
    const inside = new Set(playlistByName('many')!.track_ids);
    const outside = current().find((row) => !inside.has(row.id))!;
    outside.title = 'changed outside';
    bumpLibraryTracks([outside.id]);
    expect(tracksForSidebarSource('many')).toBe(first);
    const member = current().find((row) => inside.has(row.id))!;
    member.title = 'changed inside';
    bumpLibraryTracks([member.id]);
    const second = tracksForSidebarSource('many');
    expect(second).not.toBe(first);
    expect(second.map((row) => row.id)).toEqual(first.map((row) => row.id));
    expect(filterTracks(second, 'changed inside').map((row) => row.id)).toEqual([member.id]);
    await removeTracksFromPlaylist(playlistByName('many')!.id, [member.id]);
    expect(tracksForSidebarSource('many').map((row) => row.id)).toEqual(first.map((row) => row.id).filter((id) => id !== member.id));
    function current() {
      return library.tracks as Row[];
    }
  });
});
