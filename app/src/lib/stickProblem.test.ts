import { describe, expect, it } from 'vitest';
import { isStickProblem, roundBytes, stickProblem, type StickProblemInput } from './stickProblem.ts';

function input(overrides: Partial<StickProblemInput>): StickProblemInput {
  return {
    kind: 'wont-fit',
    stickName: 'SANDISK 8GB',
    bytesNeeded: 136_000_000,
    freeBytes: 41_000_000,
    playlists: [
      { id: 3, name: 'Closing', bytes: 52_000_000 },
      { id: 2, name: 'Peak Time', bytes: 118_000_000 },
    ],
    unanalysed: [],
    error: '',
    ...overrides,
  };
}

describe('stick problem, in plain words', () => {
  it('says how much is too much and lists the playlists, biggest first', () => {
    const problem = stickProblem(input({}));
    expect(problem.title).toBe('Too much for SANDISK 8GB');
    expect(problem.what).toBe('95 MB too much: the new tracks need 136 MB, the stick has 41 MB free.');
    expect(problem.todo).toBe('Take a playlist off, or free up space.');
    expect(problem.playlists.map((p) => p.name)).toEqual(['Peak Time', 'Closing']);
    expect(problem.action).toBeNull();
  });

  it('points at tracks when no playlist is on the stick', () => {
    expect(stickProblem(input({ playlists: [], freeBytes: null })).todo).toBe('Remove tracks from it, or free up space.');
  });

  it('shows the sync error with a retry', () => {
    const problem = stickProblem(input({ kind: 'error', error: "fourfour can't reach the stick." }));
    expect(problem).toMatchObject({ title: 'Sync to SANDISK 8GB failed', what: "fourfour can't reach the stick.", action: 'retry' });
    expect(stickProblem(input({ kind: 'error' })).what).toBe('The sync stopped. Check the stick, then sync again.');
  });

  it('names the tracks that are not analyzed and offers to analyze them', () => {
    const tracks = ['Glue', 'Opal', 'Home', 'Breathe', 'Baby', 'Gosh'].map((title) => ({ title, artist: 'Bicep' }));
    const problem = stickProblem(input({ kind: 'unanalysed', unanalysed: tracks }));
    expect(problem.title).toBe('6 tracks are not analyzed');
    expect(problem.names).toEqual(['Bicep - Glue', 'Bicep - Opal', 'Bicep - Home', 'Bicep - Breathe']);
    expect(problem.moreNames).toBe(2);
    expect(problem.action).toBe('analyze');
    const one = stickProblem(input({ kind: 'unanalysed', unanalysed: [{ title: 'Xtal', artist: '' }] }));
    expect(one.title).toBe('1 track is not analyzed');
    expect(one.what).toBe('It is left off SANDISK 8GB until analyzed. Everything else syncs as usual.');
    expect(one.names).toEqual(['Xtal']);
  });

  it('knows which states are problems', () => {
    expect(['error', 'wont-fit', 'unanalysed', 'pending', 'syncing', 'idle'].map((k) => isStickProblem(k as never))).toEqual([true, true, true, false, false, false]);
  });

  it('rounds sizes for reading', () => {
    expect([roundBytes(94_600_000), roundBytes(1_250_000_000), roundBytes(400)]).toEqual(['95 MB', '1.3 GB', '1 KB']);
  });
});
