import { describe, expect, it } from 'vitest';
import { canDockOrganizeAt, favouriteTrackIds, ORGANIZE_DOCK_MIN_WINDOW_PX } from './newPlaylist.ts';

const slotOf = (name: string) => {
  const m = name.match(/^Favorites (\d)$/);
  return m ? Number(m[1]) : null;
};

describe('canDockOrganizeAt', () => {
  it('docks from the three-column breakpoint upwards', () => {
    expect(ORGANIZE_DOCK_MIN_WINDOW_PX).toBe(1100);
    expect(canDockOrganizeAt(1440)).toBe(true);
    expect(canDockOrganizeAt(1100)).toBe(true);
    expect(canDockOrganizeAt(1099)).toBe(false);
  });
});

describe('favouriteTrackIds', () => {
  const playlists = [
    { name: 'Warmup', track_ids: [99] },
    { name: 'Favorites 3', track_ids: [5, 1] },
    { name: 'Favorites 1', track_ids: [1, 2] },
    { name: 'Favorites 2', track_ids: [] },
  ];
  it('orders by slot and drops duplicates', () => {
    expect(favouriteTrackIds(playlists, slotOf)).toEqual([1, 2, 5]);
  });
  it('can be limited to one slot', () => {
    expect(favouriteTrackIds(playlists, slotOf, 3)).toEqual([5, 1]);
  });
  it('is empty without favourites', () => {
    expect(favouriteTrackIds([{ name: 'Warmup', track_ids: [1] }], slotOf)).toEqual([]);
  });
});
