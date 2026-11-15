import { describe, expect, it } from 'vitest';
import { compareSortValues, MAX_MOVED_IN_PLACE, MAX_REPOSITIONED, moveChangedValues, orderFromClasses, sortedOrder, sortedSubset, valueClasses, valueClassesSteps, withChangedValues, type SortValue } from './sortOrder.ts';

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

  it('moves hundreds of positions at once, with repeats, ties and blanks', () => {
    const random = generator(41);
    for (const kind of ['string', 'number'] as const) {
      for (let round = 0; round < 40; round++) {
        const size = 200 + Math.floor(random() * 600);
        const values = randomValues(random, size, kind);
        const direction = round % 2 ? 1 : -1;
        const sorted = sortedOrder(values, direction);
        const changed = Array.from({ length: 70 + Math.floor(random() * 500) }, () => Math.floor(random() * size));
        const fresh = randomValues(random, size, kind);
        for (const p of changed) values[p] = random() < 0.1 ? null : fresh[random() < 0.5 ? p : Math.floor(random() * size)];
        const moved = withChangedValues(sorted, values, changed, direction)!;
        expect([...moved.order]).toEqual(reference(values, direction));
        moved.order.forEach((position, i) => expect(moved.rank[position]).toBe(i));
      }
    }
  });

  it('puts appended rows, with changed ones, where a new sort puts them', () => {
    const random = generator(77);
    for (const kind of ['string', 'number'] as const) {
      for (let round = 0; round < 80; round++) {
        const size = Math.floor(random() * 120);
        const values = randomValues(random, size, kind);
        const direction = round % 2 ? 1 : -1;
        const sorted = sortedOrder(values, direction);
        const appended = randomValues(random, 1 + Math.floor(random() * (random() < 0.2 ? 300 : 20)), kind);
        const changed = size === 0 ? [] : Array.from({ length: Math.floor(random() * 8) }, () => Math.floor(random() * size));
        const fresh = randomValues(random, size, kind);
        for (const p of changed) values[p] = random() < 0.1 ? null : fresh[p];
        values.push(...appended);
        const moved = withChangedValues(sorted, values, [...changed, size + Math.floor(random() * appended.length)], direction)!;
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

describe('moveChangedValues', () => {
  it('moves the order in place to where a new sort puts it, repeated over many bumps, and drops the rank', () => {
    const random = generator(139);
    for (const kind of ['string', 'number'] as const) {
      for (let round = 0; round < 40; round++) {
        const values = randomValues(random, 5 + Math.floor(random() * 300), kind);
        const direction = round % 2 ? 1 : -1;
        const sorted = sortedOrder(values, direction);
        const order = sorted.order;
        for (let bump = 0; bump < 12; bump++) {
          expect(sorted.rank.length).toBe(values.length);
          const changed = Array.from({ length: Math.floor(random() * (MAX_MOVED_IN_PLACE + 1)) }, () => Math.floor(random() * values.length));
          const fresh = randomValues(random, values.length, kind);
          for (const p of changed) values[p] = random() < 0.15 ? null : fresh[random() < 0.5 ? p : Math.floor(random() * values.length)];
          expect(moveChangedValues(sorted, values, changed, direction)).toBe(true);
          expect(sorted.order).toBe(order);
          expect([...sorted.order]).toEqual(reference(values, direction));
          expect(sorted.rankIsBuilt()).toBe(changed.length === 0);
          sorted.order.forEach((position, i) => expect(sorted.rank[position]).toBe(i));
        }
      }
    }
  });

  it('touches nothing past the limit', () => {
    const sorted = sortedOrder([3, 1, 2], 1);
    const many = Array.from({ length: MAX_MOVED_IN_PLACE + 1 }, (_, i) => i);
    expect(moveChangedValues(sorted, [9, 8, 7], many, 1)).toBe(false);
    expect([...sorted.order]).toEqual([1, 2, 0]);
  });
});

describe('sortedSubset without a rank', () => {
  it('gives the same rows with and without the rank of the order', () => {
    const random = generator(77);
    for (let round = 0; round < 60; round++) {
      const values = randomValues(random, 80, 'number');
      const direction = round % 2 ? 1 : -1;
      const picked = Array.from({ length: 80 }, (_, i) => i).filter(() => random() < (round % 3 === 0 ? 0.05 : 0.6));
      const without = sortedSubset(picked, sortedOrder(values, direction));
      const ranked = sortedOrder(values, direction);
      void ranked.rank;
      expect([...without]).toEqual([...sortedSubset(picked, ranked)]);
    }
  });
});
