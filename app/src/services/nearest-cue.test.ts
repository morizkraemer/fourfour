import { describe, expect, it } from 'vitest';
import { nearestCue } from '../../../prototyping/viewport/design-system/utils/nearest-cue.js';

const markers = [
  { key: 'a', position: 0.25 },
  { key: 'b', position: 0.26 },
  { key: 'end', position: 1 },
];

describe('nearestCue', () => {
  it('hits a cue within the slack', () => {
    expect(nearestCue(markers, 100 + 250 - 5, 100, 1000)?.key).toBe('a');
    expect(nearestCue(markers, 100 + 250 - 6, 100, 1000)?.key).toBe('a');
  });
  it('misses outside the slack', () => {
    expect(nearestCue(markers, 100 + 250 - 7, 100, 1000)).toBeNull();
    expect(nearestCue(markers, 100 + 600, 100, 1000)).toBeNull();
  });
  it('picks the nearest of several in range', () => {
    expect(nearestCue(markers, 100 + 257, 100, 1000)?.key).toBe('b');
    expect(nearestCue(markers, 100 + 252, 100, 1000)?.key).toBe('a');
  });
  it('treats clamped edge markers at their drawn position', () => {
    expect(nearestCue(markers, 100 + 998, 100, 1000)?.key).toBe('end');
    expect(nearestCue([{ key: 's', position: 0 }], 100 + 3, 100, 1000)?.key).toBe('s');
  });
  it('returns null for no markers or no width', () => {
    expect(nearestCue([], 10, 0, 100)).toBeNull();
    expect(nearestCue(markers, 10, 0, 0)).toBeNull();
  });
});
