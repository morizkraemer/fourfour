import { describe, expect, it } from 'vitest';
import {
  checkSummary,
  fileStateGlyph,
  fileStateTooltip,
  isUnplayable,
  lastCheckText,
  problemLabel,
  relocateSummary,
} from './fileHealth.ts';

describe('file health copy', () => {
  it('labels the top-bar button only when something is wrong', () => {
    expect(problemLabel({ missing: 0, error: 0 })).toBe('');
    expect(problemLabel({ missing: 3, error: 0 })).toBe('3 missing');
    expect(problemLabel({ missing: 2, error: 1 })).toBe('2 missing · 1 unreadable');
    expect(problemLabel({ missing: 0, error: 4 })).toBe('4 unreadable');
  });

  it('marks rows with one calm glyph per state', () => {
    expect(fileStateGlyph(undefined)).toBe('');
    expect(fileStateGlyph('missing')).toBe('!');
    expect(fileStateGlyph('error')).toBe('×');
    expect(fileStateGlyph('relocated')).toBe('✓');
    expect(fileStateTooltip(undefined, '/a.mp3')).toBeUndefined();
    expect(fileStateTooltip('missing', '/a.mp3')).toContain('File missing: /a.mp3');
  });

  it('keeps missing and unreadable files out of the player', () => {
    expect(isUnplayable('missing')).toBe(true);
    expect(isUnplayable('error')).toBe(true);
    expect(isUnplayable('relocated')).toBe(false);
    expect(isUnplayable(undefined)).toBe(false);
  });

  it('summarises a check and a relocate', () => {
    expect(checkSummary({ total: 188, missing: 3, error: 1 }, 4200)).toBe('188 tracks checked in 4 s. 4 need a look.');
    expect(checkSummary({ total: 1, missing: 0, error: 0 }, 40)).toBe('1 track checked in under a second. Every file is where it should be.');
    expect(relocateSummary({ matched: [1, 2], ambiguous: [3], not_found: [4, 5] })).toBe(
      '2 tracks relinked, 1 with more than one match, 2 not found.',
    );
    expect(relocateSummary({ matched: [1], ambiguous: [], not_found: [] })).toBe('1 track relinked.');
  });

  it('describes the last check', () => {
    const now = new Date(2026, 9, 29, 12, 0);
    expect(lastCheckText(null, { missing: 0, error: 0 }, now)).toBe('Not checked yet.');
    const today = lastCheckText(new Date(2026, 9, 29, 9, 12).getTime(), { missing: 3, error: 1 }, now);
    expect(today).toMatch(/^Last check today .+: 3 missing · 1 unreadable\.$/);
    expect(lastCheckText(new Date(2026, 9, 29, 9, 12).getTime(), { missing: 0, error: 0 }, now)).toMatch(/all files found\.$/);
  });
});
