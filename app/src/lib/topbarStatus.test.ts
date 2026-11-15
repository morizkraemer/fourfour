import { describe, expect, it } from 'vitest';
import { topbarStickStatus, type TopbarStatusInput } from './topbarStatus.ts';

function input(overrides: Partial<TopbarStatusInput> = {}): TopbarStatusInput {
  return {
    volumes: [],
    activeVolume: null,
    stickTrackCount: () => 0,
    sync: { stage: 'idle', volumePath: null, plan: null },
    ...overrides,
  };
}

describe('top bar idle status', () => {
  it('shows nothing when no stick is connected', () => {
    expect(topbarStickStatus(input())).toBeNull();
  });

  it('shows the connected stick with its track count', () => {
    const status = topbarStickStatus(input({ volumes: ['/Volumes/DJ1'], stickTrackCount: () => 40 }));
    expect(status).toEqual({ volume: '/Volumes/DJ1', name: 'DJ1', detail: '40 tracks' });
  });

  it('prefers the open stick and notes the others', () => {
    const status = topbarStickStatus(input({ volumes: ['/v/A', '/v/B'], activeVolume: '/v/B' }));
    expect(status).toMatchObject({ volume: '/v/B', name: 'B +1' });
  });

  it('counts the changes of a plan waiting for confirmation', () => {
    const sync = {
      stage: 'confirm' as const,
      volumePath: '/v/A',
      plan: { add: [1, 2], remove: [3], update_count: 1 },
    };
    expect(topbarStickStatus(input({ volumes: ['/v/A'], sync }))).toMatchObject({ detail: '4 changes' });
  });

  it('says in sync after a finished write or an empty plan', () => {
    const done = { stage: 'done' as const, volumePath: '/v/A', plan: null };
    expect(topbarStickStatus(input({ volumes: ['/v/A'], sync: done }))).toMatchObject({ detail: 'in sync' });
    const empty = { stage: 'confirm' as const, volumePath: '/v/A', plan: { add: [], remove: [], update_count: 0 } };
    expect(topbarStickStatus(input({ volumes: ['/v/A'], sync: empty }))).toMatchObject({ detail: 'in sync' });
  });

  it('ignores sync state of another stick', () => {
    const sync = { stage: 'done' as const, volumePath: '/v/B', plan: null };
    expect(topbarStickStatus(input({ volumes: ['/v/A'], stickTrackCount: () => 2, sync }))).toMatchObject({
      detail: '2 tracks',
    });
  });

  it('reports failures', () => {
    const sync = { stage: 'failed' as const, volumePath: '/v/A', plan: null };
    expect(topbarStickStatus(input({ volumes: ['/v/A'], sync }))).toMatchObject({ detail: 'sync failed' });
  });
});
