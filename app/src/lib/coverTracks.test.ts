import { describe, expect, it } from 'vitest';
import { trackIdsWithCover } from './coverTracks.ts';

const tracks = [
  { id: 1, raw: { has_artwork: false } },
  { id: 2, raw: { has_artwork: true } },
  { id: 3 },
  { id: 4, raw: { has_artwork: true } },
  { id: 5, raw: { has_artwork: true } },
];

describe('trackIdsWithCover', () => {
  it('lists the tracks that have artwork, in order', () => {
    expect(trackIdsWithCover(tracks)).toEqual([2, 4, 5]);
  });
  it('stops at the limit', () => {
    expect(trackIdsWithCover(tracks, 2)).toEqual([2, 4]);
  });
});
