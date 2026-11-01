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

import {
  addTracksToPlaylist,
  buildFavoriteSidebarRows,
  clearAllFavorites,
  clearPlaylistTracks,
  ensureFavoriteSlots,
  library,
  moveTracksToPlaylist,
  nextFavoriteSlot,
  removeFavoriteSlot,
  removeTrackFromPlaylist,
  removeTracksFromPlaylist,
  renamePlaylist,
  reorderPlaylistTracks,
} from './library.svelte.ts';

Object.assign(globalThis, { window: { __TAURI_INTERNALS__: {} } });

type Row = { id: number; fav?: number };

function playlist(id: number, name: string, track_ids: number[], over: Record<string, unknown> = {}) {
  return { id, name, track_ids, parent_id: 0, is_folder: false, ...over };
}

function ids(playlistId: number) {
  return library.playlists.find((p) => p.id === playlistId)?.track_ids;
}

function savedLists() {
  return hooks.invoke.mock.calls.filter(([command]) => command === 'save_state');
}

beforeEach(() => {
  hooks.invoke.mockReset();
  hooks.invoke.mockResolvedValue(undefined);
  library.tracks = [{ id: 1 }, { id: 2 }, { id: 3 }, { id: 4 }] as never;
  library.playlists = [
    playlist(1, 'Warmup', [1, 2, 3]),
    playlist(2, 'Peak', [4]),
    playlist(3, 'Favorites 1', []),
    playlist(4, 'Favorites 2', []),
  ] as never;
});

describe('adding and removing playlist tracks', () => {
  it('adds only tracks the playlist does not hold yet, once each, and saves', async () => {
    expect(await addTracksToPlaylist(1, [3, 4, 4, 2])).toBe(1);
    expect(ids(1)).toEqual([1, 2, 3, 4]);
    expect(savedLists()).toHaveLength(1);
  });

  it('does not save when nothing is new or the playlist is unknown', async () => {
    expect(await addTracksToPlaylist(1, [1, 2])).toBe(0);
    expect(await addTracksToPlaylist(99, [1])).toBe(0);
    expect(savedLists()).toHaveLength(0);
  });

  it('removes tracks from a playlist but keeps them in the library', async () => {
    await removeTracksFromPlaylist(1, [1, 3]);
    await removeTrackFromPlaylist(1, 2);
    expect(ids(1)).toEqual([]);
    expect(library.tracks).toHaveLength(4);
    expect(savedLists()).toHaveLength(2);
  });

  it('clears a playlist only when it has tracks', async () => {
    await clearPlaylistTracks(2);
    await clearPlaylistTracks(2);
    expect(ids(2)).toEqual([]);
    expect(savedLists()).toHaveLength(1);
  });

  it('keeps the order it is given when reordering', async () => {
    await reorderPlaylistTracks(1, [3, 1, 2]);
    expect(ids(1)).toEqual([3, 1, 2]);
  });

  it('trims the name when renaming and ignores a blank one', async () => {
    await renamePlaylist(1, '  Late set ');
    await renamePlaylist(2, '   ');
    expect(library.playlists.map((p) => p.name).slice(0, 2)).toEqual(['Late set', 'Peak']);
    expect(savedLists()).toHaveLength(1);
  });
});

describe('moving tracks between playlists', () => {
  it('moves tracks to a position and takes them out of the source', async () => {
    await moveTracksToPlaylist({ trackIds: [2, 3], toPlaylistId: 2, sourcePlaylistId: 1, insertIndex: 0 });
    expect(ids(1)).toEqual([1]);
    expect(ids(2)).toEqual([2, 3, 4]);
  });

  it('copies when asked to keep the source', async () => {
    await moveTracksToPlaylist({ trackIds: [1], toPlaylistId: 2, sourcePlaylistId: 1, removeFromSource: false });
    expect(ids(1)).toEqual([1, 2, 3]);
    expect(ids(2)).toEqual([4, 1]);
  });

  it('reorders inside one playlist without losing or doubling tracks', async () => {
    await moveTracksToPlaylist({ trackIds: [3], toPlaylistId: 1, sourcePlaylistId: 1, insertIndex: 0 });
    expect(ids(1)).toEqual([3, 1, 2]);
  });

  it('clamps an insert position past the end and lists a repeated track once', async () => {
    await moveTracksToPlaylist({ trackIds: [1, 1], toPlaylistId: 2, insertIndex: 50 });
    expect(ids(2)).toEqual([4, 1]);
  });

  it('changes nothing for an unknown target or no tracks', async () => {
    await moveTracksToPlaylist({ trackIds: [1], toPlaylistId: 99, sourcePlaylistId: 1 });
    await moveTracksToPlaylist({ trackIds: [], toPlaylistId: 2 });
    expect(ids(1)).toEqual([1, 2, 3]);
    expect(savedLists()).toHaveLength(0);
  });
});

describe('favorite slots', () => {
  const fav = () => (library.tracks as Row[]).map((t) => t.fav);

  it('shows the lowest favorite slot a track is in', async () => {
    await addTracksToPlaylist(4, [1, 2]);
    expect(fav()).toEqual([2, 2, undefined, undefined]);
    await addTracksToPlaylist(3, [2]);
    expect(fav()).toEqual([2, 1, undefined, undefined]);
  });

  it('updates the FAV column when tracks leave a favorite, by remove, clear and move', async () => {
    await addTracksToPlaylist(3, [2, 3]);
    await removeTracksFromPlaylist(3, [2]);
    expect(fav()).toEqual([undefined, undefined, 1, undefined]);
    await moveTracksToPlaylist({ trackIds: [3], toPlaylistId: 1, sourcePlaylistId: 3 });
    expect(fav()).toEqual([undefined, undefined, undefined, undefined]);
    await addTracksToPlaylist(4, [1]);
    await clearPlaylistTracks(4);
    expect(fav()).toEqual([undefined, undefined, undefined, undefined]);
  });

  it('empties every favorite but keeps the slots', async () => {
    await addTracksToPlaylist(4, [2]);
    await clearAllFavorites();
    expect(ids(3)).toEqual([]);
    expect(ids(4)).toEqual([]);
    expect(fav()).toEqual([undefined, undefined, undefined, undefined]);
  });

  it('offers the first free slot and shows at least two rows', () => {
    expect(nextFavoriteSlot()).toBe(3);
    expect(buildFavoriteSidebarRows().map((r) => [r.digit, r.count])).toEqual([
      [1, 0],
      [2, 0],
    ]);
  });

  it('brings back the two permanent slots and drops empty extra ones', async () => {
    library.playlists = [playlist(5, 'Favorites 3', []), playlist(6, 'Favorites 4', [2]), playlist(7, 'Mine', [])] as never;
    await ensureFavoriteSlots();
    expect(library.playlists.map((p) => p.name)).toEqual(['Favorites 4', 'Mine', 'Favorites 1', 'Favorites 2']);
  });

  it('removes only an extra slot and keeps the permanent ones', async () => {
    library.playlists.push(playlist(5, 'Favorites 3', [2]) as never);
    expect(await removeFavoriteSlot(3)).toBeNull();
    expect(await removeFavoriteSlot(1)).toBeNull();
    expect(await removeFavoriteSlot(5)).toBe('Favorites 3');
    expect(library.playlists.map((p) => p.name)).not.toContain('Favorites 3');
  });
});
