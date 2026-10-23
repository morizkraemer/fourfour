import { describe, expect, it } from 'vitest';
import {
  OVERSCAN_ROWS,
  ROW_HEIGHT,
  dropLineTop,
  indexForSlot,
  rowTop,
  rowWindow,
  scrollTopToReveal,
  spacerHeight,
} from './rowWindow.ts';

const view = { viewportHeight: 700, headerHeight: 28 };

describe('rowWindow', () => {
  it('starts at row 0 with overscan below at the top', () => {
    const w = rowWindow({ ...view, scrollTop: 0, rowCount: 30000 });
    expect(w.firstRow).toBe(0);
    expect(w.endRow).toBe(Math.ceil((700 - 28) / ROW_HEIGHT) + OVERSCAN_ROWS);
  });
  it('clamps the first row at 0 while the header is still scrolled over', () => {
    expect(rowWindow({ ...view, scrollTop: 10, rowCount: 100 }).firstRow).toBe(0);
  });
  it('slides with scrollTop', () => {
    const w = rowWindow({ ...view, scrollTop: 28 * 1000 + 28, rowCount: 30000 });
    expect(w.firstRow).toBe(1000 - OVERSCAN_ROWS);
    expect(w.endRow).toBe(1000 + Math.ceil(700 / ROW_HEIGHT) + OVERSCAN_ROWS);
  });
  it('clamps the end row to the row count at the bottom', () => {
    const rowCount = 30000;
    const scrollTop = 28 + rowCount * ROW_HEIGHT - 700;
    const w = rowWindow({ ...view, scrollTop, rowCount });
    expect(w.endRow).toBe(rowCount);
    expect(w.firstRow).toBeLessThan(rowCount);
    expect(w.endRow - w.firstRow).toBeLessThanOrEqual(w.poolSize);
  });
  it('handles a list shorter than the viewport', () => {
    const w = rowWindow({ ...view, scrollTop: 0, rowCount: 5 });
    expect(w).toMatchObject({ firstRow: 0, endRow: 5 });
  });
  it('handles an empty list', () => {
    const w = rowWindow({ ...view, scrollTop: 0, rowCount: 0 });
    expect(w.firstRow).toBe(0);
    expect(w.endRow).toBe(0);
  });
  it('resizing the viewport changes the pool size and end row only', () => {
    const small = rowWindow({ ...view, scrollTop: 2800, rowCount: 1000 });
    const big = rowWindow({ ...view, viewportHeight: 1400, scrollTop: 2800, rowCount: 1000 });
    expect(big.firstRow).toBe(small.firstRow);
    expect(big.endRow).toBeGreaterThan(small.endRow);
    expect(big.poolSize).toBeGreaterThan(small.poolSize);
  });
  it('has a zero viewport before the first measure', () => {
    const w = rowWindow({ scrollTop: 0, viewportHeight: 0, headerHeight: 0, rowCount: 100 });
    expect(w).toEqual({ firstRow: 0, endRow: OVERSCAN_ROWS, poolSize: 2 * OVERSCAN_ROWS + 2 });
  });
  it('jump to the end then back to the top gives the top window again', () => {
    const top = rowWindow({ ...view, scrollTop: 0, rowCount: 30000 });
    rowWindow({ ...view, scrollTop: 28 + 30000 * 28 - 700, rowCount: 30000 });
    expect(rowWindow({ ...view, scrollTop: 0, rowCount: 30000 })).toEqual(top);
  });
});

describe('indexForSlot', () => {
  it('maps every slot to a distinct row inside the window, for any first row', () => {
    const poolSize = 51;
    for (const firstRow of [0, 1, 50, 51, 52, 12345, 29999 - poolSize]) {
      const indexes = Array.from({ length: poolSize }, (_, slot) => indexForSlot(slot, firstRow, poolSize));
      expect(new Set(indexes).size).toBe(poolSize);
      expect(Math.min(...indexes)).toBe(firstRow);
      expect(Math.max(...indexes)).toBe(firstRow + poolSize - 1);
    }
  });
  it('keeps a slot on its row while the window slides by one', () => {
    const poolSize = 51;
    const before = Array.from({ length: poolSize }, (_, slot) => indexForSlot(slot, 100, poolSize));
    const after = Array.from({ length: poolSize }, (_, slot) => indexForSlot(slot, 101, poolSize));
    const moved = before.filter((index, slot) => index !== after[slot]);
    expect(moved).toEqual([100]);
  });
  it('slot equal to first row mod pool is at the first row', () => {
    expect(indexForSlot(100 % 51, 100, 51)).toBe(100);
  });
});

describe('row geometry', () => {
  it('positions rows and the spacer', () => {
    expect(rowTop(3)).toBe(84);
    expect(spacerHeight(30000)).toBe(840000);
    expect(spacerHeight(0)).toBeUndefined();
  });
  it('clamps the drop line to the end of the list', () => {
    expect(dropLineTop(2, 10)).toBe(56);
    expect(dropLineTop(99, 10)).toBe(280);
    expect(dropLineTop(0, 0)).toBe(0);
  });
});

describe('scrollTopToReveal', () => {
  const base = { ...view, rowCount: 30000 };
  it('leaves scrollTop alone when the row is visible', () => {
    expect(scrollTopToReveal(5, { ...base, scrollTop: 0 })).toBe(0);
  });
  it('scrolls down just enough to show a row below the viewport', () => {
    const top = scrollTopToReveal(100, { ...base, scrollTop: 0 });
    expect(top).toBe(28 + 101 * 28 - 700);
  });
  it('scrolls up so a row above the viewport sits right under the header', () => {
    expect(scrollTopToReveal(10, { ...base, scrollTop: 5000 })).toBe(10 * 28);
  });
  it('jumps to the last row without scrolling past the end', () => {
    const maxScroll = 28 + 30000 * 28 - 700;
    expect(scrollTopToReveal(29999, { ...base, scrollTop: 0 })).toBe(maxScroll);
    expect(scrollTopToReveal(99999, { ...base, scrollTop: 0 })).toBe(maxScroll);
  });
  it('clamps a negative index to the first row', () => {
    expect(scrollTopToReveal(-5, { ...base, scrollTop: 5000 })).toBe(0);
  });
  it('returns 0 for an empty list and for a list shorter than the viewport', () => {
    expect(scrollTopToReveal(0, { ...view, scrollTop: 300, rowCount: 0 })).toBe(0);
    expect(scrollTopToReveal(3, { ...view, scrollTop: 300, rowCount: 5 })).toBe(0);
  });
});
