import { beforeEach, describe, expect, it, vi } from 'vitest';

const hooks = vi.hoisted(() => ({ invoke: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke: hooks.invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
vi.mock('../services/playback.svelte.ts', () => ({
  playbackPause: vi.fn(),
  playbackPlay: vi.fn(async () => undefined),
  playbackSeek: vi.fn(),
  playbackSetPosition: vi.fn(),
  playbackStop: vi.fn(async () => undefined),
  subscribePlaybackError: vi.fn(async () => () => {}),
  subscribePlaybackTick: vi.fn(async () => () => {}),
}));

import { library, libraryRowsChangedSince } from './library.svelte.ts';
import { commitTrackRating } from './track-edits.svelte.ts';
import { clearSort, setSort, sortTracks } from './table-columns.svelte.ts';

Object.assign(globalThis, { window: { __TAURI_INTERNALS__: {} } });

function row(index: number) {
  return library.tracks[index] as Record<string, any>;
}

describe('commitTrackRating', () => {
  beforeEach(() => {
    hooks.invoke.mockReset();
    library.tracks = [
      { id: 1, title: 'A', rating: 0 },
      { id: 2, title: 'B', rating: 2 },
    ] as never;
  });

  it('stores the rating through the backend and shows it on the row', async () => {
    hooks.invoke.mockResolvedValue(undefined);
    expect(await commitTrackRating(1, 4)).toBe(true);
    expect(hooks.invoke).toHaveBeenCalledWith('set_track_rating', { trackId: 1, rating: 4 });
    expect(row(0).rating).toBe(4);
  });

  it('clears with 0 over an imported rating', async () => {
    hooks.invoke.mockResolvedValue(undefined);
    await commitTrackRating(2, 0);
    expect(row(1).rating).toBe(0);
  });

  it('keeps the old rating and reports when the backend refuses', async () => {
    hooks.invoke.mockRejectedValue('no space');
    expect(await commitTrackRating(2, 5)).toBe(false);
    expect(row(1).rating).toBe(2);
    expect(library.statusMessage).toBe("Couldn't set the rating. Try again.");
  });

  it('sorts by the effective rating with unrated tracks last', async () => {
    hooks.invoke.mockResolvedValue(undefined);
    await commitTrackRating(1, 5);
    clearSort('library');
    setSort('library', 'rating');
    setSort('library', 'rating');
    const ids = sortTracks([...library.tracks] as never, 'library').map((t: any) => t.id);
    expect(ids).toEqual([1, 2]);
  });

  it('re-reads the sort key after a rating change', async () => {
    hooks.invoke.mockResolvedValue(undefined);
    clearSort('library');
    setSort('library', 'rating');
    expect(sortTracks(library.tracks, 'library').map((t: any) => t.id)).toEqual([2, 1]);
    await commitTrackRating(1, 1);
    expect(sortTracks(library.tracks, 'library').map((t: any) => t.id)).toEqual([1, 2]);
  });

  it('re-reads the sort key of the changed row only', async () => {
    hooks.invoke.mockResolvedValue(undefined);
    clearSort('library');
    setSort('library', 'rating');
    expect(sortTracks(library.tracks, 'library').map((t: any) => t.id)).toEqual([2, 1]);
    row(1).rating = 9;
    await commitTrackRating(1, 3);
    expect(sortTracks(library.tracks, 'library').map((t: any) => t.id)).toEqual([2, 1]);
    library.tracks = [...library.tracks.map((t: any) => ({ ...t }))] as never;
    expect(sortTracks(library.tracks, 'library').map((t: any) => t.id)).toEqual([1, 2]);
  });
});

describe('libraryRowsChangedSince', () => {
  beforeEach(() => {
    hooks.invoke.mockReset();
    hooks.invoke.mockResolvedValue(undefined);
    library.tracks = [
      { id: 1, title: 'A', rating: 0 },
      { id: 2, title: 'B', rating: 2 },
      { id: 3, title: 'C', rating: 0 },
    ] as never;
  });

  it('lists the rows bumped since an earlier list', async () => {
    const loaded = library.tracks;
    await commitTrackRating(3, 1);
    const afterOne = library.tracks;
    await commitTrackRating(1, 2);
    expect(libraryRowsChangedSince(loaded)).toEqual([row(2), row(0)]);
    expect(libraryRowsChangedSince(afterOne)).toEqual([row(0)]);
    expect(libraryRowsChangedSince(library.tracks)).toEqual([]);
  });

  it('cannot tell across a replaced list', async () => {
    const loaded = library.tracks;
    await commitTrackRating(3, 1);
    library.tracks = [{ id: 1, title: 'A' }] as never;
    await commitTrackRating(1, 2);
    expect(libraryRowsChangedSince(loaded)).toBeNull();
  });
});
