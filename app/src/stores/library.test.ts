import { describe, expect, it } from 'vitest';
import {
  FAVORITE_SLOTS,
  favoritePlaylistName,
  favoriteSlotFromName,
  filterTracks,
  isFavoritePlaylistName,
  sidebarSourceLabel,
} from './library.svelte.ts';

describe('favorite playlist names', () => {
  it('round-trips every slot', () => {
    for (let slot = 1; slot <= FAVORITE_SLOTS; slot++) {
      expect(favoriteSlotFromName(favoritePlaylistName(slot))).toBe(slot);
    }
  });
  it('rejects slot 0, slots above the max and malformed names', () => {
    expect(favoriteSlotFromName('Favorites 0')).toBeNull();
    expect(favoriteSlotFromName(`Favorites ${FAVORITE_SLOTS + 1}`)).toBeNull();
    expect(favoriteSlotFromName('favorites 1')).toBeNull();
    expect(favoriteSlotFromName('Favorites 1 ')).toBeNull();
    expect(favoriteSlotFromName('My Favorites 1')).toBeNull();
    expect(favoriteSlotFromName('')).toBeNull();
  });
  it('isFavoritePlaylistName mirrors slot parsing', () => {
    expect(isFavoritePlaylistName('Favorites 3')).toBe(true);
    expect(isFavoritePlaylistName('Warmup')).toBe(false);
  });
});

describe('filterTracks', () => {
  const tracks = [
    { id: 1, title: 'Strobe', artist: 'deadmau5', bpm: '128', key: 'Am', index: 1 },
    { id: 2, title: 'Opus', artist: 'Eric Prydz', bpm: '126', raw: { genre: 'Progressive House' } },
    { id: 3, title: 'Untitled', filePath: '/music/Techno/untitled.wav' },
  ];
  it('returns the same array for blank queries', () => {
    expect(filterTracks(tracks, '')).toBe(tracks);
    expect(filterTracks(tracks, '   ')).toBe(tracks);
  });
  it('matches case-insensitively across fields', () => {
    expect(filterTracks(tracks, 'STROBE').map((t) => t.id)).toEqual([1]);
    expect(filterTracks(tracks, 'prydz').map((t) => t.id)).toEqual([2]);
  });
  it('requires all whitespace-separated terms', () => {
    expect(filterTracks(tracks, 'eric opus').map((t) => t.id)).toEqual([2]);
    expect(filterTracks(tracks, 'eric strobe')).toEqual([]);
  });
  it('searches raw fields and file paths', () => {
    expect(filterTracks(tracks, 'progressive').map((t) => t.id)).toEqual([2]);
    expect(filterTracks(tracks, 'techno').map((t) => t.id)).toEqual([3]);
  });
  it('tolerates missing fields without matching "undefined"', () => {
    expect(filterTracks(tracks, 'undefined')).toEqual([]);
  });
});

describe('sidebarSourceLabel', () => {
  it('defaults empty source to All Tracks', () => {
    expect(sidebarSourceLabel('')).toBe('All Tracks');
    expect(sidebarSourceLabel('All Tracks')).toBe('All Tracks');
  });
  it('passes playlist names through', () => {
    expect(sidebarSourceLabel('Warmup')).toBe('Warmup');
  });
});
