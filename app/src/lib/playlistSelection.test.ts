import { describe, expect, it } from 'vitest';
import {
  EMPTY_SELECTION,
  nextPlaylistSelection,
  outermostIds,
  playlistIdsInSelection,
  pruneSelection,
} from './playlistSelection.ts';

const plain = { shift: false, toggle: false };
const order = [1, 2, 3, 4, 5];

describe('nextPlaylistSelection', () => {
  it('selects a single row on plain click', () => {
    expect(nextPlaylistSelection(order, EMPTY_SELECTION, 3, plain)).toEqual({ ids: [3], anchor: 3 });
    expect(nextPlaylistSelection(order, { ids: [1, 2], anchor: 1 }, 4, plain)).toEqual({ ids: [4], anchor: 4 });
  });

  it('toggles a row with the toggle modifier and moves the anchor', () => {
    const one = nextPlaylistSelection(order, EMPTY_SELECTION, 2, plain);
    const two = nextPlaylistSelection(order, one, 4, { shift: false, toggle: true });
    expect(two).toEqual({ ids: [2, 4], anchor: 4 });
    expect(nextPlaylistSelection(order, two, 2, { shift: false, toggle: true })).toEqual({ ids: [4], anchor: 2 });
  });

  it('selects the range from the anchor with shift, in either direction', () => {
    const start = { ids: [2], anchor: 2 };
    expect(nextPlaylistSelection(order, start, 4, { shift: true, toggle: false })).toEqual({ ids: [2, 3, 4], anchor: 2 });
    expect(nextPlaylistSelection(order, { ids: [4], anchor: 4 }, 1, { shift: true, toggle: false })).toEqual({
      ids: [1, 2, 3, 4],
      anchor: 4,
    });
  });

  it('falls back to a single selection when shift has no anchor', () => {
    expect(nextPlaylistSelection(order, EMPTY_SELECTION, 3, { shift: true, toggle: false })).toEqual({
      ids: [3],
      anchor: 3,
    });
  });
});

describe('pruneSelection', () => {
  it('drops ids that no longer exist', () => {
    expect(pruneSelection({ ids: [1, 2, 3], anchor: 2 }, new Set([1, 3]))).toEqual({ ids: [1, 3], anchor: null });
  });
  it('keeps the same object when nothing changed', () => {
    const selection = { ids: [1], anchor: 1 };
    expect(pruneSelection(selection, new Set([1, 2]))).toBe(selection);
  });
});

const tree = [
  { id: 1, name: 'Folder', track_ids: [], parent_id: 0, is_folder: true },
  { id: 2, name: 'In folder', track_ids: [1], parent_id: 1, is_folder: false },
  { id: 3, name: 'Sub folder', track_ids: [], parent_id: 1, is_folder: true },
  { id: 4, name: 'Deep', track_ids: [2], parent_id: 3, is_folder: false },
  { id: 5, name: 'Top', track_ids: [3], parent_id: 0, is_folder: false },
];

describe('outermostIds', () => {
  it('drops items inside another selected folder', () => {
    expect(outermostIds(tree, [1, 2, 4, 5])).toEqual([1, 5]);
  });
  it('keeps siblings of an unselected folder', () => {
    expect(outermostIds(tree, [2, 4])).toEqual([2, 4]);
  });
});

describe('playlistIdsInSelection', () => {
  it('expands selected folders into their playlists without duplicates', () => {
    expect(playlistIdsInSelection(tree, [1, 2, 5]).sort()).toEqual([2, 4, 5]);
  });
});
