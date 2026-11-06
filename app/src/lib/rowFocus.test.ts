import { describe, expect, it } from 'vitest';
import { MAX_LEAD_ROWS, leadRows, rowFocusOrder } from './rowFocus.ts';

const base = { firstRow: 88, endRow: 152, visibleFirst: 100, visibleLast: 130, rowCount: 10000 };

describe('rowFocusOrder', () => {
  it('standing still starts in the middle of the screen', () => {
    const order = rowFocusOrder({ ...base, rowsPerFrame: 0 });
    expect(order[0]).toBe(115);
    expect(new Set(order.slice(0, 31))).toEqual(new Set(Array.from({ length: 31 }, (_, i) => 100 + i)));
  });

  it('scrolling down starts ahead of the bottom edge', () => {
    const order = rowFocusOrder({ ...base, rowsPerFrame: 4 });
    expect(order[0]).toBe(130 + leadRows(4) / 2);
    expect(Math.max(...order)).toBe(130 + leadRows(4));
    expect(order.indexOf(140)).toBeLessThan(order.indexOf(100));
  });

  it('scrolling up starts ahead of the top edge', () => {
    const order = rowFocusOrder({ ...base, rowsPerFrame: -4 });
    expect(order[0]).toBe(100 - leadRows(-4) / 2);
    expect(Math.min(...order)).toBe(100 - leadRows(-4));
  });

  it('lists every row once and stays inside the list', () => {
    const order = rowFocusOrder({ firstRow: 0, endRow: 40, visibleFirst: 0, visibleLast: 25, rowCount: 60, rowsPerFrame: 30 });
    expect(new Set(order).size).toBe(order.length);
    expect(Math.min(...order)).toBe(0);
    expect(Math.max(...order)).toBe(59);
  });

  it('caps the lead', () => {
    expect(leadRows(1000)).toBe(MAX_LEAD_ROWS);
    expect(leadRows(0)).toBe(0);
  });
});
