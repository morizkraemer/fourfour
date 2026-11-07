import { beforeEach, describe, expect, it, vi } from 'vitest';

function base64(bytes: Uint8Array) {
  let text = '';
  for (const b of bytes) text += String.fromCharCode(b);
  return btoa(text);
}

const BANDS = base64(new Uint8Array(384).map((_, i) => 20 + (i % 90)));

function info(id: number, title: string, rating = 0) {
  return {
    id,
    source_path: `/m/${id}.mp3`,
    title,
    artist: `Artist ${id}`,
    album: 'B',
    genre: 'House',
    tempo: 12400 + id * 100,
    key: '8A',
    duration_secs: 200,
    bitrate: 320,
    file_size: 1,
    has_artwork: false,
    has_cues: false,
    has_analysis: true,
    analysis_pending: false,
    rating,
  };
}

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
  invoke: vi.fn(async (command: string, args: Record<string, any> = {}) => {
    if (command === 'load_state') {
      return { tracks: [info(1, 'Alpha', 3), info(2, 'Bravo'), info(3, 'Charlie')], playlists: [] };
    }
    if (command === 'get_row_previews') return args.trackIds.includes(1) ? [{ id: 1, waveform_bands: BANDS }] : [];
    if (command === 'plan_import') {
      return { token: 1, new_count: 1, already_in_library: 0, duplicates: [], skipped: { unsupported: 0, unsupported_exts: [], unreadable: 0 } };
    }
    if (command === 'commit_import') return { added: [info(4, 'Delta')], skipped_ids: [] };
    return undefined;
  }),
}));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));

import { analyzeTrackIds, bumpLibraryTracks, filterTracks, importFiles, library, libraryPlainRows, loadState, markCoversStored, requestRowPreviews, tracksForSidebarSource } from './library.svelte.ts';
import { fileStatus } from './file-status.svelte.ts';
import { sortTracks, setSort, clearSort } from './table-columns.svelte.ts';
import { bootRowOf } from './boot-snapshot.ts';
import { heldInState, watchValues } from './state-probe.svelte.ts';

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));
const row = (id: number) => library.tracks.find((t) => t.id === id)!;

beforeEach(async () => {
  fileStatus.byId = {};
  await loadState();
});

describe('library track rows', () => {
  it('stores plain data fields only, so mapping stays on the fast object path', () => {
    for (const track of library.tracks) {
      for (const key of Object.keys(track)) {
        expect('value' in Object.getOwnPropertyDescriptor(track, key)!, key).toBe(true);
      }
      expect(Object.keys(track)).not.toEqual(expect.arrayContaining(['bands']));
    }
  });

  it('follows a field write on that row only', async () => {
    const first = watchValues(() => row(1).title);
    const second = watchValues(() => row(2).title);
    row(1).title = 'Alpha (edit)';
    await settle();
    expect(first.values).toEqual(['Alpha', 'Alpha (edit)']);
    expect(second.values).toEqual(['Bravo']);
    first.stop();
    second.stop();
  });

  it('shows a bulk analyze start on mounted rows and on rows nobody has read yet', async () => {
    const mounted = watchValues(() => `${row(1).bpm}|${row(1).key}|${row(1).analysisPending}`);
    await analyzeTrackIds([1, 2, 3]);
    await settle();
    expect(mounted.values).toHaveLength(2);
    expect(mounted.values[1]).toBe('—|—|true');
    const late = watchValues(() => `${row(3).bpm}|${row(3).key}|${row(3).analysisPending}|${row(3).analysisLoaded}`);
    expect(late.values).toEqual(['—|—|true|false']);
    row(3).bpm = '99.0';
    await settle();
    expect(late.values).toEqual(['—|—|true|false', '99.0|—|true|false']);
    expect(filterTracks(library.tracks, 'Bravo').map((t) => t.id)).toEqual([2]);
    expect(libraryPlainRows().map((t) => [t.bpm, t.key])).toEqual([['—', '—'], ['—', '—'], ['—', '—']]);
    mounted.stop();
    late.stop();
  });

  it('shows the derived row fields and follows what they derive from', async () => {
    const track = row(1);
    expect('bands' in track && 'cover' in track && 'rowCover' in track && 'status' in track).toBe(true);
    expect(track.status).toBe('');
    const status = watchValues(() => row(1).status);
    const bands = watchValues(() => row(1).bands);
    fileStatus.byId = { 1: 'missing' };
    requestRowPreviews([track]);
    await settle();
    await settle();
    expect(status.values.length).toBe(2);
    expect(status.values[1]).not.toBe('');
    expect(bands.values[0]).toBeNull();
    expect(bands.values.at(-1)).not.toBeNull();
    expect(row(2).bands).toBeNull();
    status.stop();
    bands.stop();
  });

  it('keeps search, sort and boot rows in step after a bump', () => {
    row(3).title = 'Aardvark';
    row(3).rating = 5;
    expect(filterTracks(library.tracks, 'aardvark')).toEqual([]);
    bumpLibraryTracks([3]);
    expect(filterTracks(library.tracks, 'aardvark').map((t) => t.id)).toEqual([3]);
    expect(filterTracks(library.tracks, 'ARTIST 2 /m/2.MP3 house 8a').map((t) => t.id)).toEqual([2]);
    expect(filterTracks(library.tracks, 'undefined')).toEqual([]);
    clearSort('library');
    setSort('library', 'title');
    expect(sortTracks(library.tracks, 'library').map((t) => t.id)).toEqual([3, 1, 2]);
    clearSort('library');
    const boot = bootRowOf(row(3));
    expect(boot.cells.title).toBe('Aardvark');
    expect(boot.cells.rating).toBe(5);
    expect(Object.keys(boot.cells)).not.toContain('status');
  });

  it('marks covers stored after an import and bumps only rows that changed', () => {
    const before = library.tracks;
    markCoversStored([2, 99]);
    expect(row(2).raw.has_artwork).toBe(true);
    expect(row(1).raw.has_artwork).toBe(false);
    expect(library.tracks).not.toBe(before);
    const after = library.tracks;
    markCoversStored([2]);
    expect(library.tracks).toBe(after);
  });

  it('keeps a cover that is stored before the imported row lands', async () => {
    markCoversStored([4]);
    await importFiles(['/m/4.mp3']);
    expect(row(4).raw.has_artwork).toBe(true);
  });

  it('stays the same reactive row when held in other state', async () => {
    const held = heldInState(row(1));
    expect(held).toBe(row(1));
    const title = watchValues(() => held.title);
    row(1).title = 'Held';
    await settle();
    expect(title.values).toEqual(['Alpha', 'Held']);
    title.stop();
  });

  it('lists the analysed rows for Recently Added and sorts them from the plain objects', () => {
    expect(tracksForSidebarSource('Recently Added').map((t) => t.id)).toEqual([1, 2, 3]);
    setSort('library', 'artist');
    expect(sortTracks(library.tracks, 'library').map((t) => t.id)).toEqual([1, 2, 3]);
    row(2).artist = 'Zed';
    bumpLibraryTracks([2]);
    expect(sortTracks(library.tracks, 'library').map((t) => t.id)).toEqual([1, 3, 2]);
    clearSort('library');
  });
});
