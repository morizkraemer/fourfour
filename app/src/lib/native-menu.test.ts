import { describe, expect, it } from 'vitest';
import { isTextEntry } from './native-menu.ts';

const inside = (field: { tagName: string; type?: string } | null) => ({ closest: () => field });

describe('isTextEntry', () => {
  it('keeps the native menu on text inputs and textareas', () => {
    expect(isTextEntry(inside({ tagName: 'INPUT', type: 'text' }))).toBe(true);
    expect(isTextEntry(inside({ tagName: 'INPUT' }))).toBe(true);
    expect(isTextEntry(inside({ tagName: 'TEXTAREA' }))).toBe(true);
  });

  it('drops it on everything else, including non-text inputs', () => {
    expect(isTextEntry(inside(null))).toBe(false);
    expect(isTextEntry(inside({ tagName: 'INPUT', type: 'checkbox' }))).toBe(false);
    expect(isTextEntry(null)).toBe(false);
    expect(isTextEntry({})).toBe(false);
  });
});
