import { describe, expect, it } from 'vitest';
import { firstDroppedTrack } from './playerDrop.ts';

const tracks = [{ id: 1 }, { id: 2 }, { id: 3 }];

describe('firstDroppedTrack', () => {
  it('loads the first dragged track', () => {
    expect(firstDroppedTrack([3, 1], tracks)).toBe(tracks[2]);
  });
  it('loads nothing for an empty drag or an unknown track', () => {
    expect(firstDroppedTrack([], tracks)).toBeNull();
    expect(firstDroppedTrack([9], tracks)).toBeNull();
  });
});
