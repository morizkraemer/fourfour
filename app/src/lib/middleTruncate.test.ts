import { describe, expect, it } from 'vitest';
import { middleTruncate } from '../../../prototyping/viewport/design-system/utils/middle-truncate.js';

describe('middleTruncate', () => {
  it('leaves short paths unchanged', () => {
    expect(middleTruncate('/a/b.mp3', 40)).toBe('/a/b.mp3');
    expect(middleTruncate('/a/b.mp3', 8)).toBe('/a/b.mp3');
  });
  it('keeps the start and the full filename', () => {
    const path = '/Users/merlin/Music/neue treibung/15 - Script - Dubby.mp3';
    const out = middleTruncate(path, 44);
    expect(out.length).toBeLessThanOrEqual(44);
    expect(out.startsWith('/Users/')).toBe(true);
    expect(out.endsWith('/15 - Script - Dubby.mp3')).toBe(true);
    expect(out).toContain('…');
  });
  it('never exceeds the budget, even when the filename alone is too long', () => {
    const out = middleTruncate('/x/a very long file name that cannot fit.flac', 20);
    expect(Array.from(out).length).toBeLessThanOrEqual(20);
    expect(out.endsWith('.flac')).toBe(true);
  });
  it('counts unicode code points, not UTF-16 units', () => {
    const out = middleTruncate('/Müsik/🎵🎵🎵/Björk – Jóga 🎶.mp3', 20);
    expect(Array.from(out).length).toBeLessThanOrEqual(20);
    expect(out.endsWith('🎶.mp3')).toBe(true);
    expect(out).not.toMatch(/[\ud800-\udbff](?![\udc00-\udfff])/);
  });
  it('handles windows separators and tiny budgets', () => {
    expect(middleTruncate('C:\\Music\\Set\\track.wav', 14).endsWith('track.wav')).toBe(true);
    expect(middleTruncate('/abc/def.mp3', 1)).toBe('…');
  });
});
