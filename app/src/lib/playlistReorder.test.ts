import { describe, expect, it } from 'vitest';
import { moveSelectedByOne } from './playlistReorder.ts';

const order = [1, 2, 3, 4, 5];

describe('moveSelectedByOne', () => {
  it('moves one track up and down', () => {
    expect(moveSelectedByOne(order, new Set([3]), 'up')).toEqual([1, 3, 2, 4, 5]);
    expect(moveSelectedByOne(order, new Set([3]), 'down')).toEqual([1, 2, 4, 3, 5]);
  });

  it('moves a contiguous block as one', () => {
    expect(moveSelectedByOne(order, new Set([3, 4]), 'up')).toEqual([1, 3, 4, 2, 5]);
    expect(moveSelectedByOne(order, new Set([2, 3]), 'down')).toEqual([1, 4, 2, 3, 5]);
  });

  it('moves a split selection, each track one row', () => {
    expect(moveSelectedByOne(order, new Set([2, 4]), 'up')).toEqual([2, 1, 4, 3, 5]);
    expect(moveSelectedByOne(order, new Set([2, 4]), 'down')).toEqual([1, 3, 2, 5, 4]);
  });

  it('keeps a block pressed against the end where it is and moves the rest', () => {
    expect(moveSelectedByOne(order, new Set([1, 3]), 'up')).toEqual([1, 3, 2, 4, 5]);
    expect(moveSelectedByOne(order, new Set([3, 5]), 'down')).toEqual([1, 2, 4, 3, 5]);
  });

  it('returns null when nothing can move', () => {
    expect(moveSelectedByOne(order, new Set([1]), 'up')).toBeNull();
    expect(moveSelectedByOne(order, new Set([4, 5]), 'down')).toBeNull();
    expect(moveSelectedByOne(order, new Set(), 'up')).toBeNull();
    expect(moveSelectedByOne(order, new Set([1, 2, 3, 4, 5]), 'down')).toBeNull();
  });
});
