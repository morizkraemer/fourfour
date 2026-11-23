import { describe, expect, it } from 'vitest';
import type { ArchivedMerge, DuplicateGroup, DuplicateMember, MergeReport } from '../services/tauri.svelte.ts';
import {
  archiveNote,
  copyFacts,
  copyPlaces,
  defaultPicks,
  groupHeading,
  groupNote,
  groupsAfterMerge,
  keeperOf,
  mergeLabel,
  mergeSummary,
  pickedCount,
  pickedRequests,
  requestFor,
} from './duplicateCleanup.ts';

function member(id: number, over: Partial<DuplicateMember> = {}): DuplicateMember {
  return {
    id,
    artist: 'Hale',
    title: 'Copper Lines',
    path: `/music/${id}.flac`,
    format: 'FLAC',
    lossless: true,
    bitrate: 900,
    sample_rate: 44100,
    duration_secs: 380,
    file_size: 1000,
    playlists: 2,
    cues: 0,
    date_added: '2026-01-01',
    file_present: true,
    ...over,
  };
}

function group(key: number, confidence: DuplicateGroup['confidence'], ids: number[]): DuplicateGroup {
  return { key, confidence, keeper_id: ids[0], reason: 'Lossless (FLAC) over lossy (MP3)', members: ids.map((id) => member(id)) };
}

describe('keeper choice', () => {
  const g = group(1, 'metadata_only', [5, 7, 9]);

  it('uses the proposed keeper until the user picks another member', () => {
    expect(keeperOf(g, {})).toBe(5);
    expect(keeperOf(g, { 1: 9 })).toBe(9);
  });

  it('ignores a pick that is not a member of the group', () => {
    expect(keeperOf(g, { 1: 42 })).toBe(5);
  });

  it('merges every other member into the keeper', () => {
    expect(requestFor(g, { 1: 7 })).toEqual({ keeper_id: 7, remove_ids: [5, 9], require_identical: false });
  });

  it('asks the backend to compare the files only for exact groups', () => {
    expect(requestFor(group(2, 'exact_audio', [1, 2]), {}).require_identical).toBe(true);
  });
});

describe('songs picked for merging', () => {
  const groups = [group(1, 'exact_audio', [1, 2]), group(2, 'metadata_only', [3, 4]), group(3, 'exact_audio', [5, 6])];

  it('starts with the exact groups only', () => {
    expect([...defaultPicks(groups)]).toEqual([1, 3]);
  });

  it('merges the picked groups with the chosen keepers', () => {
    expect(pickedCount(groups, new Set([1, 2]))).toBe(2);
    expect(pickedRequests(groups, { 1: 2 }, new Set([1, 2]))).toEqual([
      { keeper_id: 2, remove_ids: [1], require_identical: true },
      { keeper_id: 3, remove_ids: [4], require_identical: false },
    ]);
  });

  it('ignores picks for groups that are gone and is empty with no picks', () => {
    expect(pickedCount(groups, new Set([9]))).toBe(0);
    expect(pickedRequests(groups, {}, new Set())).toEqual([]);
  });

  it('labels the action with the number of songs', () => {
    expect(mergeLabel(0)).toBe('Merge');
    expect(mergeLabel(1)).toBe('Merge 1 song');
    expect(mergeLabel(4)).toBe('Merge 4 songs');
  });
});

describe('a copy in one line', () => {
  const g = group(1, 'exact_audio', [1, 2]);

  it('says the format, and the bitrate only for lossy files', () => {
    expect(copyFacts(member(1, { playlists: 3, cues: 1 }), g)).toBe('FLAC · in 3 playlists · 1 cue');
    expect(copyFacts(member(2, { format: 'mp3', lossless: false, bitrate: 320, playlists: 0, cues: 0 }), g)).toBe('MP3 320 kbps · no playlists or cues');
    expect(copyFacts(member(2, { playlists: 0, cues: 2 }), g)).toBe('FLAC · 2 cues');
  });

  it('shows the length only when the copies differ', () => {
    const uneven = { ...g, members: [member(1), member(2, { duration_secs: 381 })] };
    expect(copyFacts(uneven.members[1], uneven)).toBe('FLAC · 6:21 · in 2 playlists');
  });

  it('shows only the part of the path that tells the copies apart', () => {
    expect(copyPlaces(['/Users/dj/Music/Library/Hale/a.flac', '/Users/dj/Downloads/a.mp3'])).toEqual(['Music/Library/Hale', 'Downloads']);
    expect(copyPlaces(['/Users/dj/Music/a.flac', '/Volumes/Gigs SSD/Sets/a.flac', '/home/dj/a.flac'])).toEqual(['Music', 'Gigs SSD/Sets', '']);
    expect(copyPlaces(['/Music/a.wav', '/Music/a (1).wav'])).toEqual(['a.wav', 'a (1).wav']);
    expect(copyPlaces(['/Music/a.wav', '/Music/Old/a.wav'])).toEqual(['Music', 'Music/Old']);
    expect(copyPlaces(['C:\\Music\\a.wav', 'D:\\Sets\\a.wav'])).toEqual(['C:/Music', 'D:/Sets']);
  });

  it('gives the match and why the keeper was suggested', () => {
    expect(groupNote(g, {})).toBe('Identical audio · Suggested: Lossless (FLAC) over lossy (MP3)');
    expect(groupNote(g, { 1: 2 })).toBe('Identical audio · Your pick');
  });
});

describe('after a merge', () => {
  const groups = [group(1, 'exact_audio', [1, 2]), group(2, 'metadata_only', [3, 4])];
  const report: MergeReport = { merged: [{ merge_id: 1, keeper_id: 1, removed_ids: [2] }], failed: [] };

  it('drops merged groups and keeps the rest', () => {
    expect(groupsAfterMerge(groups, report).map((g) => g.key)).toEqual([2]);
  });

  it('summarises merges and says no file was deleted', () => {
    expect(mergeSummary(report)).toBe('Merged 1 copy into 1 keeper. No file was deleted.');
    expect(mergeSummary({ merged: [], failed: [{ keeper_id: 1, message: 'not identical' }] })).toBe('1 merge not done: not identical');
  });
});

describe('text', () => {
  it('names a group after its keeper with a plain hyphen', () => {
    expect(groupHeading(group(1, 'exact_audio', [1, 2]))).toBe('Hale - Copper Lines');
  });

  it('says what was merged and when', () => {
    const merge = (n: number) => ({ merged_at: '2026-11-20T18:30:00Z', tracks: Array.from({ length: n }, (_, i) => ({ id: i })) }) as unknown as ArchivedMerge;
    const one = { merged_at: '2026-11-20T18:30:00Z', tracks: [{ path: '/Users/dj/Downloads/a.mp3', format: 'MP3', lossless: false, bitrate: 320 }] } as unknown as ArchivedMerge;
    expect(archiveNote(merge(3))).toBe('3 copies · merged 20 Nov 2026');
    expect(archiveNote(one)).toBe('MP3 320 kbps from Downloads · merged 20 Nov 2026');
    expect(mergeSummary({ merged: [{ merge_id: 1, keeper_id: 1, removed_ids: [2, 3] }], failed: [] })).toBe('Merged 2 copies into 1 keeper. No file was deleted.');
  });
});
