import { describe, expect, it } from 'vitest';
import { searchModeForColumn } from './searchLayout.ts';

describe('searchModeForColumn', () => {
  it('shows the field from 720 px up', () => {
    expect(searchModeForColumn(1200)).toBe('field');
    expect(searchModeForColumn(720)).toBe('field');
  });

  it('collapses to an icon between 440 and 720 px', () => {
    expect(searchModeForColumn(719)).toBe('icon');
    expect(searchModeForColumn(440)).toBe('icon');
  });

  it('moves the icon to the left sidebar below 440 px', () => {
    expect(searchModeForColumn(439)).toBe('sidebar');
    expect(searchModeForColumn(0)).toBe('sidebar');
  });
});
