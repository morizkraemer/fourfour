import { describe, expect, it } from 'vitest';
import type { ArchivedMerge, DuplicateGroup, DuplicateMember, MergeReport } from '../services/tauri.svelte.ts';
import {
  archiveCopies,
  exactCount,
  exactRequests,
  groupHeading,
  groupsAfterMerge,
  keeperOf,
  mergeSummary,
  reasonOf,
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
    expect(reasonOf(g, {})).toBe(g.reason);
    expect(reasonOf(g, { 1: 9 })).toBe('your pick');
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

describe('accept all exact matches', () => {
  const groups = [group(1, 'exact_audio', [1, 2]), group(2, 'metadata_only', [3, 4]), group(3, 'exact_audio', [5, 6])];

  it('takes only exact groups the user did not skip, with the chosen keepers', () => {
    expect(exactCount(groups, new Set([3]))).toBe(1);
    expect(exactRequests(groups, { 1: 2 }, new Set([3]))).toEqual([{ keeper_id: 2, remove_ids: [1], require_identical: true }]);
  });

  it('is empty when every exact group is skipped', () => {
    expect(exactRequests(groups, {}, new Set([1, 3]))).toEqual([]);
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

  it('counts copies with the right plural', () => {
    const merge = (n: number) => ({ tracks: Array.from({ length: n }, (_, i) => ({ id: i })) }) as unknown as ArchivedMerge;
    expect(archiveCopies(merge(1))).toBe('1 copy');
    expect(archiveCopies(merge(3))).toBe('3 copies');
    expect(mergeSummary({ merged: [{ merge_id: 1, keeper_id: 1, removed_ids: [2, 3] }], failed: [] })).toBe('Merged 2 copies into 1 keeper. No file was deleted.');
  });
});
