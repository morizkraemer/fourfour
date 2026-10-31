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
    return undefined;
  }),
}));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));

import { bumpLibraryTracks, filterTracks, library, loadState, requestRowPreviews } from './library.svelte.ts';
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
    clearSort('library');
    setSort('library', 'title');
    expect(sortTracks(library.tracks, 'library').map((t) => t.id)).toEqual([3, 1, 2]);
    clearSort('library');
    const boot = bootRowOf(row(3));
    expect(boot.cells.title).toBe('Aardvark');
    expect(boot.cells.rating).toBe(5);
    expect(Object.keys(boot.cells)).not.toContain('status');
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
});
