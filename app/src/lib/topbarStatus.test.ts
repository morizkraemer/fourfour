import { describe, expect, it } from 'vitest';
import { topbarIdleStatus, type TopbarStatusInput } from './topbarStatus.ts';

function input(overrides: Partial<TopbarStatusInput> = {}): TopbarStatusInput {
  return {
    volumes: [],
    activeVolume: null,
    stickTrackCount: () => 0,
    trackCount: 1200,
    unanalyzedCount: 0,
    sync: { stage: 'idle', volumePath: null, plan: null },
    ...overrides,
  };
}

describe('top bar idle status', () => {
  it('summarises the library when no stick is connected', () => {
    expect(topbarIdleStatus(input())).toEqual({ kind: 'library', text: '1200 tracks' });
  });

  it('adds the unanalyzed count', () => {
    expect(topbarIdleStatus(input({ unanalyzedCount: 3, trackCount: 1 }))).toEqual({
      kind: 'library',
      text: '1 track · 3 unanalyzed',
    });
  });

  it('shows the connected stick with its track count', () => {
    const status = topbarIdleStatus(input({ volumes: ['/Volumes/DJ1'], stickTrackCount: () => 40 }));
    expect(status).toEqual({ kind: 'stick', volume: '/Volumes/DJ1', name: 'DJ1', detail: '40 tracks' });
  });

  it('prefers the open stick and notes the others', () => {
    const status = topbarIdleStatus(input({ volumes: ['/v/A', '/v/B'], activeVolume: '/v/B' }));
    expect(status).toMatchObject({ kind: 'stick', volume: '/v/B', name: 'B +1' });
  });

  it('counts the changes of a plan waiting for confirmation', () => {
    const sync = {
      stage: 'confirm' as const,
      volumePath: '/v/A',
      plan: { add: [1, 2], remove: [3], update_count: 1 },
    };
    expect(topbarIdleStatus(input({ volumes: ['/v/A'], sync }))).toMatchObject({ detail: '4 changes' });
  });

  it('says in sync after a finished write or an empty plan', () => {
    const done = { stage: 'done' as const, volumePath: '/v/A', plan: null };
    expect(topbarIdleStatus(input({ volumes: ['/v/A'], sync: done }))).toMatchObject({ detail: 'in sync' });
    const empty = { stage: 'confirm' as const, volumePath: '/v/A', plan: { add: [], remove: [], update_count: 0 } };
    expect(topbarIdleStatus(input({ volumes: ['/v/A'], sync: empty }))).toMatchObject({ detail: 'in sync' });
  });

  it('ignores sync state of another stick', () => {
    const sync = { stage: 'done' as const, volumePath: '/v/B', plan: null };
    expect(topbarIdleStatus(input({ volumes: ['/v/A'], stickTrackCount: () => 2, sync }))).toMatchObject({
      detail: '2 tracks',
    });
  });

  it('reports failures', () => {
    const sync = { stage: 'failed' as const, volumePath: '/v/A', plan: null };
    expect(topbarIdleStatus(input({ volumes: ['/v/A'], sync }))).toMatchObject({ detail: 'sync failed' });
  });
});
