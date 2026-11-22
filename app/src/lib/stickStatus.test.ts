import { describe, expect, it } from 'vitest';
import { planChanges, stickStatus, topbarStick, type StickSyncSnapshot } from './stickStatus.ts';

const STICK = '/Volumes/INTENSO';
const idle: StickSyncSnapshot = { stage: 'idle', volumePath: null, progress: null };

function syncing(stage: StickSyncSnapshot['stage'], progress: StickSyncSnapshot['progress'] = null, volumePath = STICK) {
  return { stage, volumePath, progress };
}

describe('stick status', () => {
  it('is idle with no dot when nothing waits', () => {
    expect(stickStatus(STICK, idle, null)).toEqual({ kind: 'idle', tone: 'none', label: 'In sync' });
    expect(stickStatus(STICK, idle, { changes: 0, fits: true }).tone).toBe('none');
  });

  it('never shows selection: a finished sync is idle again', () => {
    expect(stickStatus(STICK, syncing('done'), { changes: 0, fits: true }).kind).toBe('idle');
  });

  it('waits in steady amber when changes are pending', () => {
    expect(stickStatus(STICK, idle, { changes: 3, fits: true })).toEqual({ kind: 'pending', tone: 'waiting', label: '3 changes waiting' });
    expect(stickStatus(STICK, idle, { changes: 1, fits: null }).label).toBe('1 change waiting');
    expect(stickStatus(STICK, syncing('confirm'), { changes: 2, fits: true }).kind).toBe('pending');
  });

  it('waits after an edit even when the dry run counts no track changes', () => {
    expect(stickStatus(STICK, idle, { changes: 0, fits: true, edited: true })).toEqual({ kind: 'pending', tone: 'waiting', label: 'Changes waiting' });
    expect(stickStatus(STICK, idle, { changes: 2, fits: true, edited: true }).label).toBe('2 changes waiting');
  });

  it('is busy while checking and writing, with the progress when known', () => {
    expect(stickStatus(STICK, syncing('planning'), null)).toEqual({ kind: 'syncing', tone: 'busy', label: 'Checking' });
    expect(stickStatus(STICK, syncing('writing'), null).label).toBe('Syncing');
    expect(stickStatus(STICK, syncing('writing', { done: 12, total: 40 }), { changes: 40, fits: true }).label).toBe('Syncing 12/40');
  });

  it('is a problem when the sync failed or the changes will not fit', () => {
    expect(stickStatus(STICK, syncing('failed'), { changes: 3, fits: true })).toEqual({ kind: 'error', tone: 'problem', label: 'Sync failed' });
    expect(stickStatus(STICK, idle, { changes: 9, fits: false })).toEqual({ kind: 'wont-fit', tone: 'problem', label: "Won't fit" });
  });

  it('ignores a sync that runs on another stick', () => {
    expect(stickStatus(STICK, syncing('writing', null, '/Volumes/OTHER'), { changes: 2, fits: true }).kind).toBe('pending');
    expect(stickStatus(STICK, syncing('failed', null, '/Volumes/OTHER'), null).kind).toBe('idle');
  });

  it('counts adds, removals and updates as changes', () => {
    expect(planChanges({ add: [1, 2], remove: [3], update_count: 4 })).toBe(7);
  });
});

describe('top bar stick', () => {
  const at = (volume: string, kind: Parameters<typeof stickStatus>[2], sync = idle) => ({ volume, status: stickStatus(volume, sync, kind) });

  it('shows nothing when no stick is connected or every stick is fine', () => {
    expect(topbarStick([])).toBeNull();
    expect(topbarStick([at('/v/A', null), at('/v/B', { changes: 0, fits: true })])).toBeNull();
  });

  it('shows the stick that needs attention', () => {
    const shown = topbarStick([at('/v/A', null), at('/v/B', { changes: 2, fits: true })]);
    expect(shown).toMatchObject({ volume: '/v/B', name: 'B', others: 0 });
    expect(shown?.status.label).toBe('2 changes waiting');
  });

  it('puts the most urgent stick first and counts the rest', () => {
    const shown = topbarStick([
      at('/v/A', { changes: 2, fits: true }),
      at('/v/B', null, { stage: 'writing', volumePath: '/v/B', progress: null }),
      at('/v/C', { changes: 5, fits: false }),
    ]);
    expect(shown).toMatchObject({ volume: '/v/C', others: 2 });
    expect(shown?.status.kind).toBe('wont-fit');
  });
});
