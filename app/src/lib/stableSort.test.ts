import { describe, expect, it } from 'vitest';
import { anchorShift, sameMembers } from './stableSort.ts';

const rows = (n: number) => Array.from({ length: n }, (_, i) => ({ id: i }));

describe('sameMembers', () => {
  it('is true for the same rows in another order', () => {
    const list = rows(5);
    expect(sameMembers(new Set(list), [...list].reverse())).toBe(true);
  });

  it('is false when a row is missing, added or swapped', () => {
    const list = rows(5);
    const members = new Set(list);
    expect(sameMembers(members, list.slice(1))).toBe(false);
    expect(sameMembers(members, [...list, { id: 9 }])).toBe(false);
    expect(sameMembers(members, [...list.slice(1), { id: 9 }])).toBe(false);
  });
});

describe('anchorShift', () => {
  it('follows the rows in view when rows are inserted above them', () => {
    const before = rows(100);
    const moved = [before[80], before[90], before[95]];
    const after = [...moved, ...before.filter((r) => !moved.includes(r))];
    expect(anchorShift(before, after, { first: 40, end: 60 })).toBe(3);
  });

  it('follows the rows in view when rows above them leave', () => {
    const before = rows(100);
    const moved = [before[2], before[5]];
    const after = [...before.filter((r) => !moved.includes(r)), ...moved];
    expect(anchorShift(before, after, { first: 40, end: 60 })).toBe(-2);
  });

  it('ignores a row in view that moved on its own', () => {
    const before = rows(100);
    const after = [before[50], ...before.filter((r) => r !== before[50])];
    expect(anchorShift(before, after, { first: 40, end: 60 })).toBe(1);
  });

  it('is zero without a range, for an unchanged order, or when nothing in view is found', () => {
    const before = rows(10);
    expect(anchorShift(before, [...before], null)).toBe(0);
    expect(anchorShift(before, [...before], { first: 2, end: 6 })).toBe(0);
    expect(anchorShift(before, rows(10), { first: 2, end: 6 })).toBe(0);
  });
});
