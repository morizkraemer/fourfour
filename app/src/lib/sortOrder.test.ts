import { describe, expect, it } from 'vitest';
import { compareSortValues, MAX_REPOSITIONED, orderFromClasses, sortedOrder, sortedSubset, valueClasses, valueClassesSteps, withChangedValues, type SortValue } from './sortOrder.ts';

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

const WORDS = ['', 'a', 'B', 'alpha', 'Alpha', 'alpha 2', 'beta', 'ä', 'z', 'zz', '10', '9', ' x', 'ß', 'É', 'é', 'İstanbul', 'Σίσυφος', '日本', '日本語', '😀', '\u{10000}', '\uFFFD', 'a\u0301', 'á'].map((w) => w.toLowerCase());

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

  it('gives the same order in any chunking of the steps', () => {
    const random = generator(31);
    for (const kind of ['string', 'number'] as const) {
      for (let round = 0; round < 30; round++) {
        const values = randomValues(random, Math.floor(random() * 90), kind);
        const whole = valueClasses(values)!;
        for (const chunk of [1, 2, 3, 7, 16, 1000]) {
          const steps = valueClassesSteps(values, chunk);
          let step = steps.next();
          while (!step.done) step = steps.next();
          expect(step.value!.count).toBe(whole.count);
          expect([...step.value!.classes]).toEqual([...whole.classes]);
          for (const direction of [1, -1] as const) {
            expect([...orderFromClasses(step.value!, direction).order]).toEqual(reference(values, direction));
          }
        }
      }
    }
  });

  it('orders many distinct strings and ties across chunk borders like the comparator', () => {
    const random = generator(5);
    const values: SortValue[] = Array.from({ length: 3000 }, () => (random() < 0.1 ? null : WORDS[Math.floor(random() * WORDS.length)] + String(Math.floor(random() * 400)).padStart(3, '0')));
    for (const direction of [1, -1] as const) {
      expect([...sortedOrder(values, direction).order]).toEqual(reference(values, direction));
      const steps = valueClassesSteps(values, 64);
      let step = steps.next();
      while (!step.done) step = steps.next();
      expect([...orderFromClasses(step.value!, direction).order]).toEqual(reference(values, direction));
    }
  });

  it('keeps the old comparator order for lists that are not all strings or all numbers', () => {
    const mixed: SortValue[] = ['b', 2, null, 'a', 1, 'c'];
    expect(valueClasses(mixed)).toBeNull();
    expect(valueClasses([1, NaN, 2])).toBeNull();
    const order = [...sortedOrder(mixed, 1).order];
    expect(order.slice().sort((a, b) => a - b)).toEqual([0, 1, 2, 3, 4, 5]);
    expect(order[order.length - 1]).toBe(2);
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
