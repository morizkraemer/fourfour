import { beforeEach, describe, expect, it } from 'vitest';
import {
  addFolder,
  addPlaylist,
  deletePlaylist,
  duplicatePlaylist,
  library,
  movePlaylistInTree,
  playlistsInFolder,
} from './library.svelte.ts';
import { folderState, toggleFolder } from './sidebar-folders.svelte.ts';

const rowLabels = () =>
  library.sidebarData.playlists.rows.map((r) => `${'-'.repeat(r.depth)}${r.label}${r.kind === 'folder' ? '/' : ''}`);

beforeEach(() => {
  library.playlists = [];
  folderState.collapsed = [];
});

describe('playlist folders in the store', () => {
  it('nests rows under their folder and numbers new folder names', async () => {
    const sets = await addFolder();
    const second = await addFolder();
    await addPlaylist('Friday', [1, 2], sets);
    await addPlaylist('Loose');
    expect(library.playlists.find((p) => p.id === second)?.name).toBe('New folder 2');
    expect(rowLabels()).toEqual(['New folder/', '-Friday', 'New folder 2/', 'Loose']);
  });

  it('hides the contents of a collapsed folder and expands it when something is added', async () => {
    const sets = await addFolder(0, 'Sets');
    await addPlaylist('Friday', [], sets);
    toggleFolder(sets);
    expect(rowLabels()).toEqual(['Sets/']);
    await addPlaylist('Saturday', [], sets);
    expect(rowLabels()).toEqual(['Sets/', '-Friday', '-Saturday']);
  });

  it('moves playlists into folders and refuses a folder into itself', async () => {
    const sets = await addFolder(0, 'Sets');
    const inner = await addFolder(sets, 'Inner');
    const loose = await addPlaylist('Loose');
    expect(await movePlaylistInTree(loose, inner)).toBe(true);
    expect(rowLabels()).toEqual(['Sets/', '-Inner/', '--Loose']);
    expect(await movePlaylistInTree(sets, inner)).toBe(false);
    expect(await movePlaylistInTree(sets, sets)).toBe(false);
    expect(rowLabels()).toEqual(['Sets/', '-Inner/', '--Loose']);
  });

  it('reorders siblings around an anchor', async () => {
    const a = await addPlaylist('A');
    await addPlaylist('B');
    const c = await addPlaylist('C');
    await movePlaylistInTree(c, 0, a);
    expect(rowLabels()).toEqual(['C', 'A', 'B']);
    await movePlaylistInTree(c, 0, a, true);
    expect(rowLabels()).toEqual(['A', 'C', 'B']);
  });

  it('deletes a folder together with everything inside it', async () => {
    const sets = await addFolder(0, 'Sets');
    const inner = await addFolder(sets, 'Inner');
    await addPlaylist('Deep', [1], inner);
    await addPlaylist('Shallow', [2], sets);
    await addPlaylist('Keep');
    expect(playlistsInFolder(sets).map((p) => p.name).sort()).toEqual(['Deep', 'Shallow']);
    await deletePlaylist(sets);
    expect(rowLabels()).toEqual(['Keep']);
    expect(library.playlists).toHaveLength(1);
  });

  it('duplicates a playlist next to the original with its tracks', async () => {
    const sets = await addFolder(0, 'Sets');
    const friday = await addPlaylist('Friday', [3, 4], sets);
    await addPlaylist('Other');
    const copy = await duplicatePlaylist(friday);
    expect(rowLabels()).toEqual(['Sets/', '-Friday', '-Friday copy', 'Other']);
    const copied = library.playlists.find((p) => p.id === copy)!;
    expect(copied.track_ids).toEqual([3, 4]);
    expect(copied.parent_id).toBe(sets);
    copied.track_ids.push(9);
    expect(library.playlists.find((p) => p.id === friday)?.track_ids).toEqual([3, 4]);
  });

  it('does not duplicate a folder', async () => {
    const sets = await addFolder(0, 'Sets');
    expect(await duplicatePlaylist(sets)).toBeNull();
  });
});
