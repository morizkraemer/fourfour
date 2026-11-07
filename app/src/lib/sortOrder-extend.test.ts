import { describe, expect, it } from 'vitest';
import { extendValueClasses, valueClasses, type SortValue } from './sortOrder.ts';

function generator(seed: number) {
  let state = seed;
  return () => {
    state = (state * 1664525 + 1013904223) >>> 0;
    return state / 2 ** 32;
  };
}

const WORDS = ['', 'a', 'b', 'alpha', 'alpha 2', 'beta', 'ä', 'z', 'zz', '10', '9', ' x', 'ß', 'é', 'i̇stanbul', 'σίσυφος', '日本', '日本語', '😀', '\u{10000}', '�', 'á', 'á'];

function values(random: () => number, n: number, kind: 'string' | 'number'): SortValue[] {
  return Array.from({ length: n }, () => {
    const r = random();
    if (r < 0.12) return null;
    if (kind === 'string') return r < 0.5 ? WORDS[Math.floor(random() * WORDS.length)] : `w${Math.floor(random() * 900)}`;
    return r < 0.5 ? Math.floor(random() * 12) / 2 - 1 : Math.floor(random() * 900) / 4 - 50;
  });
}

describe('extendValueClasses', () => {
  it('gives the classes of the whole list, strings and numbers, with repeats, new values and no value', () => {
    let folds = 0;
    for (const kind of ['string', 'number'] as const) {
      for (let seed = 1; seed <= 60; seed++) {
        const random = generator(seed * 7919);
        const all = values(random, 40 + Math.floor(random() * 400), kind);
        const split = 1 + Math.floor(random() * (all.length - 1));
        const held = valueClasses(all.slice(0, split));
        if (!held || held.count === 0) continue;
        const folded = extendValueClasses(held, all, split);
        expect(folded).toBeDefined();
        folds++;
        const whole = valueClasses(all)!;
        expect(folded!.count).toBe(whole.count);
        expect(Array.from(folded!.classes)).toEqual(Array.from(whole.classes));
        const more = [...all, ...values(random, 30, kind)];
        const again = extendValueClasses(folded!, more, all.length)!;
        expect(Array.from(again.classes)).toEqual(Array.from(valueClasses(more)!.classes));
      }
    }
    expect(folds).toBeGreaterThan(80);
  });

  it('gives up on values of the other kind and on NaN', () => {
    expect(extendValueClasses(valueClasses(['b', 'a'])!, ['b', 'a', 3], 2)).toBeUndefined();
    expect(extendValueClasses(valueClasses([1, 2])!, [1, 2, NaN], 2)).toBeUndefined();
  });

  it('keeps zero and negative zero together', () => {
    const all = [0, 5, -3, -0, 7, 0];
    const folded = extendValueClasses(valueClasses(all.slice(0, 3))!, all, 3)!;
    expect(Array.from(folded.classes)).toEqual(Array.from(valueClasses(all)!.classes));
  });
});
