import { describe, expect, it } from 'vitest';
import { compareSortValues, MAX_REPOSITIONED, sortedOrder, sortedSubset, withChangedValues, type SortValue } from './sortOrder.ts';

function generator(seed: number) {
  let state = seed;
  return () => {
    state = (state * 1664525 + 1013904223) >>> 0;
    return state / 2 ** 32;
  };
}

function reference(values: readonly SortValue[], direction: 1 | -1): number[] {
  return values
    .map((value, position) => ({ value, position }))
    .sort((a, b) => {
      if (a.value === null && b.value === null) return 0;
      if (a.value === null) return 1;
      if (b.value === null) return -1;
      if (a.value < b.value) return -1 * direction;
      if (a.value > b.value) return 1 * direction;
      return 0;
    })
    .map((entry) => entry.position);
}

const WORDS = ['', 'a', 'B', 'alpha', 'Alpha', 'alpha 2', 'beta', 'ä', 'z', 'zz', '10', '9', ' x'].map((w) => w.toLowerCase());

function randomValues(random: () => number, n: number, kind: 'string' | 'number'): SortValue[] {
  return Array.from({ length: n }, () => {
    const r = random();
    if (r < 0.15) return null;
    if (kind === 'string') return WORDS[Math.floor(random() * WORDS.length)];
    return Math.floor(random() * 12) / 2 - 1;
  });
}

describe('sortedOrder', () => {
  it('gives the order of a stable comparator sort, nulls last, both directions', () => {
    const random = generator(7);
    for (const kind of ['string', 'number'] as const) {
      for (let round = 0; round < 40; round++) {
        const values = randomValues(random, Math.floor(random() * 60), kind);
        for (const direction of [1, -1] as const) {
          const sorted = sortedOrder(values, direction);
          expect([...sorted.order]).toEqual(reference(values, direction));
          sorted.order.forEach((position, i) => expect(sorted.rank[position]).toBe(i));
        }
      }
    }
  });

  it('handles an empty list and a list without values', () => {
    expect([...sortedOrder([], 1).order]).toEqual([]);
    expect([...sortedOrder([null, null, null], -1).order]).toEqual([0, 1, 2]);
  });

  it('compares like the sort comparator', () => {
    expect(compareSortValues(null, null, 1)).toBe(0);
    expect(compareSortValues(null, 'a', -1)).toBeGreaterThan(0);
    expect(compareSortValues('a', null, -1)).toBeLessThan(0);
    expect(compareSortValues(1, 2, 1)).toBeLessThan(0);
    expect(compareSortValues(1, 2, -1)).toBeGreaterThan(0);
    expect(compareSortValues('b', 'b', -1)).toBe(0);
  });
});

describe('sortedSubset', () => {
  it('orders any subset of positions like sorting just those rows', () => {
    const random = generator(11);
    for (let round = 0; round < 40; round++) {
      const values = randomValues(random, 50, 'string');
      const sorted = sortedOrder(values, round % 2 ? 1 : -1);
      const picked = Array.from({ length: 50 }, (_, i) => i).filter(() => random() < 0.4);
      const expected = reference(
        picked.map((p) => values[p]),
        round % 2 ? 1 : -1,
      ).map((i) => picked[i]);
      expect([...sortedSubset(picked, sorted)]).toEqual(expected);
    }
  });
});

describe('withChangedValues', () => {
  it('moves the changed positions to where a new sort puts them', () => {
    const random = generator(23);
    for (const kind of ['string', 'number'] as const) {
      for (let round = 0; round < 60; round++) {
        const values = randomValues(random, 5 + Math.floor(random() * 50), kind);
        const direction = round % 2 ? 1 : -1;
        const sorted = sortedOrder(values, direction);
        const changed = Array.from({ length: 1 + Math.floor(random() * 6) }, () => Math.floor(random() * values.length));
        const fresh = randomValues(random, values.length, kind);
        for (const p of changed) values[p] = fresh[p];
        const moved = withChangedValues(sorted, values, changed, direction)!;
        expect([...moved.order]).toEqual(reference(values, direction));
        moved.order.forEach((position, i) => expect(moved.rank[position]).toBe(i));
      }
    }
  });

  it('gives up past the limit and returns the same order for no change', () => {
    const sorted = sortedOrder([3, 1, 2], 1);
    expect(withChangedValues(sorted, [3, 1, 2], [], 1)).toBe(sorted);
    const many = Array.from({ length: MAX_REPOSITIONED + 1 }, (_, i) => i);
    expect(withChangedValues(sorted, [3, 1, 2], many, 1)).toBeNull();
  });
});
