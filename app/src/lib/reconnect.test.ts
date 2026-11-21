import { describe, expect, it } from 'vitest';
import { candidateLabel, canApplyRemap, choiceReasonText, commonFolder, reconnectSummary, relocationText, remapPreviewText } from './reconnect.ts';

describe('commonFolder', () => {
  it('finds the deepest folder that holds every path', () => {
    expect(commonFolder(['/Volumes/Old/Music/a/1.mp3', '/Volumes/Old/Music/b/2.mp3'])).toBe('/Volumes/Old/Music');
    expect(commonFolder(['/Volumes/Old/Music/a/1.mp3'])).toBe('/Volumes/Old/Music/a');
  });

  it('does not cut a folder name in the middle', () => {
    expect(commonFolder(['/m/House/1.mp3', '/m/Housing/2.mp3'])).toBe('/m');
  });

  it('is the root for paths that share only the root, and empty for nothing or unrelated drives', () => {
    expect(commonFolder(['/a/1.mp3', '/b/2.mp3'])).toBe('/');
    expect(commonFolder([])).toBe('');
    expect(commonFolder(['C:\\Music\\a.mp3', 'D:\\Music\\b.mp3'])).toBe('');
  });

  it('keeps the separator of Windows paths', () => {
    expect(commonFolder(['C:\\Music\\a\\1.mp3', 'C:\\Music\\b\\2.mp3'])).toBe('C:\\Music');
  });
});

describe('reconnect copy', () => {
  it('summarises reconnected, needs choice and still missing, leaving out zero parts', () => {
    const plan = { reconnect: [{ track_id: 1, source_path: '/a' }], needs_choice: [], still_missing: [] };
    expect(reconnectSummary(plan)).toBe('1 track reconnected.');
    expect(reconnectSummary({ reconnect: [], needs_choice: [{ track_id: 2, reason: 'copy', candidates: [] }], still_missing: [3, 4] })).toBe(
      '0 tracks reconnected, 1 needs your choice, 2 still missing.',
    );
    expect(reconnectSummary({ reconnect: plan.reconnect, needs_choice: [1, 2].map((track_id) => ({ track_id, reason: 'unconfirmed' as const, candidates: [] })), still_missing: [] })).toBe(
      '1 track reconnected, 2 need your choice.',
    );
  });

  it('words every reason a track needs a choice', () => {
    for (const reason of ['multiple_candidates', 'unconfirmed', 'copy', 'shared_file'] as const) {
      expect(choiceReasonText(reason).length).toBeGreaterThan(0);
    }
  });
});

describe('relocate folder copy', () => {
  it('previews found and still missing counts', () => {
    expect(remapPreviewText({ total: 10, found: 8, still_missing: 2 })).toBe('8 found · 2 still missing');
    expect(remapPreviewText({ total: 3, found: 3, still_missing: 0 })).toBe('3 found · none missing');
    expect(remapPreviewText({ total: 0, found: 0, still_missing: 0 })).toBe('No track lives in that folder.');
  });

  it('allows applying only when something was found', () => {
    expect(canApplyRemap(null)).toBe(false);
    expect(canApplyRemap({ total: 2, found: 0, still_missing: 2 })).toBe(false);
    expect(canApplyRemap({ total: 2, found: 1, still_missing: 1 })).toBe(true);
  });

  it('names the undoable step', () => {
    expect(relocationText('/old', '/new', 1)).toBe('1 track moved from /old to /new.');
    expect(relocationText('/old', '/new', 4)).toBe('4 tracks moved from /old to /new.');
  });
});

describe('candidateLabel', () => {
  it('shows the file and its folder', () => {
    expect(candidateLabel('/Volumes/New/House/song.mp3')).toBe('House/song.mp3');
    expect(candidateLabel('C:\\Music\\song.mp3')).toBe('Music/song.mp3');
    expect(candidateLabel('song.mp3')).toBe('song.mp3');
  });
});
