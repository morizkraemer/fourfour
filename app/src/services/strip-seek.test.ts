import { describe, expect, it } from 'vitest';
import { stripFraction } from '../../../prototyping/viewport/design-system/utils/strip-seek.js';

describe('stripFraction', () => {
  it('maps a pointer position to a track fraction', () => {
    expect(stripFraction(150, 100, 200)).toBe(0.25);
    expect(stripFraction(100, 100, 200)).toBe(0);
    expect(stripFraction(300, 100, 200)).toBe(1);
  });
  it('clamps a pointer outside the strip', () => {
    expect(stripFraction(40, 100, 200)).toBe(0);
    expect(stripFraction(900, 100, 200)).toBe(1);
  });
  it('is 0 for a strip without width', () => {
    expect(stripFraction(120, 100, 0)).toBe(0);
  });
});
