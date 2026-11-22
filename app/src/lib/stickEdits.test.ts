import { describe, expect, it } from 'vitest';
import { moveTracks, playlistTracksOnStick, sameOrder, stickSelection, withoutTracks } from './stickEdits.ts';

describe('edits to a playlist on a stick', () => {
  it('removes tracks and keeps the order of the rest', () => {
    expect(withoutTracks([1, 2, 3, 4], [2, 4])).toEqual([1, 3]);
    expect(withoutTracks([1, 2], [9])).toEqual([1, 2]);
  });

  it('moves one track before another row', () => {
    expect(moveTracks([1, 2, 3, 4], [4], 1)).toEqual([1, 4, 2, 3]);
    expect(moveTracks([1, 2, 3, 4], [1], 3)).toEqual([2, 3, 1, 4]);
    expect(moveTracks([1, 2, 3, 4], [2], 4)).toEqual([1, 3, 4, 2]);
  });

  it('moves several tracks together in their list order', () => {
    expect(moveTracks([1, 2, 3, 4, 5], [4, 2], 0)).toEqual([2, 4, 1, 3, 5]);
    expect(moveTracks([1, 2, 3, 4, 5], [1, 2], 5)).toEqual([3, 4, 5, 1, 2]);
  });

  it('leaves the list alone when a track is dropped where it already is', () => {
    expect(sameOrder(moveTracks([1, 2, 3], [2], 1), [1, 2, 3])).toBe(true);
    expect(sameOrder(moveTracks([1, 2, 3], [2], 2), [1, 2, 3])).toBe(true);
    expect(sameOrder(moveTracks([1, 2, 3], [9], 0), [1, 2, 3])).toBe(true);
  });
});

describe('what the next sync writes', () => {
  const kept = [{ playlist_id: 7, as_of: '2026-11-22T10:00:00Z', track_ids: [3, 1] }];

  it('a playlist that follows the library uses the library list', () => {
    expect(playlistTracksOnStick(5, [1, 2, 3], kept)).toEqual([1, 2, 3]);
    expect(playlistTracksOnStick(5, [1, 2, 3], undefined)).toEqual([1, 2, 3]);
  });

  it("a kept playlist uses the stick's own list", () => {
    expect(playlistTracksOnStick(7, [1, 2, 3], kept)).toEqual([3, 1]);
  });

  it('the sync target carries the kept lists of playlists still on the stick', () => {
    expect(stickSelection({ track_ids: [9], playlist_ids: [5, 7], kept_as_is: kept })).toEqual({
      kind: 'playlists',
      playlist_ids: [5, 7],
      track_ids: [9],
      kept: [{ playlist_id: 7, track_ids: [3, 1] }],
    });
    expect(stickSelection({ track_ids: [], playlist_ids: [5], kept_as_is: kept }).kept).toEqual([]);
  });

  it('an unknown stick or an engine without kept lists syncs what the library holds', () => {
    expect(stickSelection(undefined)).toEqual({ kind: 'playlists', playlist_ids: [], track_ids: [], kept: [] });
    expect(stickSelection({ track_ids: [1], playlist_ids: [2] }).kept).toEqual([]);
    expect(stickSelection({ track_ids: [], playlist_ids: [7], kept_as_is: [{ playlist_id: 7 }] }).kept).toEqual([]);
  });
});
