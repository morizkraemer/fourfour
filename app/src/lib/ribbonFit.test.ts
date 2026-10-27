import { describe, expect, it } from 'vitest';
import { fitRibbonCovers, ribbonSlots } from './ribbonFit.ts';

const covers = Array.from({ length: 30 }, (_, i) => `c${i}`);

describe('ribbon cover fitting', () => {
  it('counts whole tiles that fit', () => {
    expect(ribbonSlots(400)).toBe(Math.floor((400 - 12 - 28) / 46));
    expect(ribbonSlots(20)).toBe(0);
  });
  it('fills the rail and ends with a +N tile when more exist', () => {
    const slots = ribbonSlots(500);
    const fit = fitRibbonCovers(covers, 500);
    expect(fit.covers.length).toBe(slots - 1);
    expect(fit.more).toBe(30 - (slots - 1));
  });
  it('shows everything when it fits', () => {
    expect(fitRibbonCovers(covers.slice(0, 3), 600)).toEqual({ covers: covers.slice(0, 3), more: 0 });
  });
  it('reserves a slot for the drop hint', () => {
    expect(fitRibbonCovers(covers.slice(0, 5), 5 * 46 + 40 + 12, 1).more).toBeGreaterThan(0);
  });
});
