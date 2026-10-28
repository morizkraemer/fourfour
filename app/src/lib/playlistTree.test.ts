import { describe, expect, it } from 'vitest';
import {
  canMoveInto,
  descendantIds,
  movePlaylist,
  removeWithDescendants,
  sidebarTreeRows,
  uniqueName,
  type TreePlaylist,
} from './playlistTree.ts';

function node(id: number, name: string, parent_id = 0, is_folder = false, track_ids: number[] = []): TreePlaylist {
  return { id, name, parent_id, is_folder, track_ids };
}

const tree = [
  node(1, 'Sets', 0, true),
  node(2, 'Friday', 1, false, [7, 8]),
  node(3, 'Archive', 1, true),
  node(4, 'Old', 3, false, [9]),
  node(5, 'Loose'),
  node(6, 'Favorites 1'),
];
const notHidden = () => false;
const favoritesHidden = (p: TreePlaylist) => p.name.startsWith('Favorites ');

describe('sidebarTreeRows', () => {
  it('lists the tree depth-first in array order with depths', () => {
    const rows = sidebarTreeRows(tree, new Set(), favoritesHidden);
    expect(rows.map((r) => [r.name, r.depth])).toEqual([
      ['Sets', 0],
      ['Friday', 1],
      ['Archive', 1],
      ['Old', 2],
      ['Loose', 0],
    ]);
  });

  it('skips the children of collapsed folders and reports expansion', () => {
    const rows = sidebarTreeRows(tree, new Set([1]), favoritesHidden);
    expect(rows.map((r) => r.name)).toEqual(['Sets', 'Loose']);
    expect(rows[0]).toMatchObject({ isFolder: true, expanded: false });
  });

  it('shows a nested collapsed folder without its contents', () => {
    const rows = sidebarTreeRows(tree, new Set([3]), favoritesHidden);
    expect(rows.map((r) => r.name)).toEqual(['Sets', 'Friday', 'Archive', 'Loose']);
  });

  it('carries the track count of playlists', () => {
    const rows = sidebarTreeRows(tree, new Set(), favoritesHidden);
    expect(rows.find((r) => r.name === 'Friday')?.trackCount).toBe(2);
  });

  it('follows array order among siblings', () => {
    const reordered = [tree[4], ...tree.slice(0, 4)];
    const rows = sidebarTreeRows(reordered, new Set(), notHidden);
    expect(rows[0].name).toBe('Loose');
  });
});

describe('moving', () => {
  it('finds all descendants', () => {
    expect([...descendantIds(tree, 1)].sort()).toEqual([2, 3, 4]);
    expect(descendantIds(tree, 5).size).toBe(0);
  });

  it('refuses cycles, self moves, playlist parents and unknown ids', () => {
    expect(canMoveInto(tree, 1, 1)).toBe(false);
    expect(canMoveInto(tree, 1, 3)).toBe(false);
    expect(canMoveInto(tree, 1, 4)).toBe(false);
    expect(canMoveInto(tree, 5, 2)).toBe(false);
    expect(canMoveInto(tree, 99, 0)).toBe(false);
    expect(canMoveInto(tree, 3, 1)).toBe(true);
    expect(canMoveInto(tree, 3, 0)).toBe(true);
  });

  it('moves a playlist into a folder as its last child', () => {
    const moved = movePlaylist(tree, 5, 3)!;
    expect(moved.find((p) => p.id === 5)?.parent_id).toBe(3);
    const rows = sidebarTreeRows(moved, new Set(), favoritesHidden);
    expect(rows.map((r) => [r.name, r.depth])).toEqual([
      ['Sets', 0],
      ['Friday', 1],
      ['Archive', 1],
      ['Old', 2],
      ['Loose', 2],
    ]);
  });

  it('moves out of a folder to the root', () => {
    const moved = movePlaylist(tree, 2, 0)!;
    expect(moved.find((p) => p.id === 2)?.parent_id).toBe(0);
    expect(sidebarTreeRows(moved, new Set(), favoritesHidden).map((r) => r.name)).toEqual([
      'Sets',
      'Archive',
      'Old',
      'Loose',
      'Friday',
    ]);
  });

  it('places before or after an anchor sibling', () => {
    const before = movePlaylist(tree, 5, 1, 2)!;
    expect(sidebarTreeRows(before, new Set(), favoritesHidden).slice(0, 4).map((r) => r.name)).toEqual([
      'Sets',
      'Loose',
      'Friday',
      'Archive',
    ]);
    const after = movePlaylist(tree, 5, 1, 2, true)!;
    expect(sidebarTreeRows(after, new Set(), favoritesHidden).slice(0, 4).map((r) => r.name)).toEqual([
      'Sets',
      'Friday',
      'Loose',
      'Archive',
    ]);
  });

  it('keeps a moved folder with its contents', () => {
    const moved = movePlaylist(tree, 3, 0)!;
    const rows = sidebarTreeRows(moved, new Set(), favoritesHidden);
    expect(rows.map((r) => [r.name, r.depth])).toEqual([
      ['Sets', 0],
      ['Friday', 1],
      ['Loose', 0],
      ['Archive', 0],
      ['Old', 1],
    ]);
  });

  it('returns null for a forbidden move and leaves the input alone', () => {
    const before = JSON.stringify(tree);
    expect(movePlaylist(tree, 1, 4)).toBeNull();
    expect(JSON.stringify(tree)).toBe(before);
  });
});

describe('removeWithDescendants', () => {
  it('drops a folder with everything inside it', () => {
    expect(removeWithDescendants(tree, 1).map((p) => p.id)).toEqual([5, 6]);
  });

  it('drops a single playlist', () => {
    expect(removeWithDescendants(tree, 5).map((p) => p.id)).toEqual([1, 2, 3, 4, 6]);
  });
});

describe('uniqueName', () => {
  it('keeps a free name and numbers a taken one', () => {
    expect(uniqueName(['A'], 'B')).toBe('B');
    expect(uniqueName(['New folder'], 'New folder')).toBe('New folder 2');
    expect(uniqueName(['New folder', 'New folder 2'], 'New folder')).toBe('New folder 3');
  });
});
