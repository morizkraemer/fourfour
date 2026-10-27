import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import type { UsbSyncPlan } from '../services/tauri.svelte.ts';

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

const STICK = '/Volumes/SanDisk 64GB';

function plan(overrides: Partial<UsbSyncPlan> = {}): UsbSyncPlan {
  return {
    volume: 'SanDisk 64GB',
    stick_id: 'abc',
    playlist_count: 1,
    add: [{ track_id: 1, title: 'Hyperion', artist: 'Solomun', size_bytes: 1000 }],
    remove: [],
    update_count: 0,
    unchanged_count: 0,
    skipped_unanalyzed: [],
    foreign_kept_count: 0,
    bytes_to_add: 1000,
    bytes_to_free: 0,
    bytes_needed: 2000,
    free_bytes: 10_000,
    fits: true,
    ...overrides,
  };
}

const doneReport = {
  volume: 'SanDisk 64GB',
  path: STICK,
  status: 'done',
  report: { tracks_added: 1, tracks_updated: 0, tracks_replaced: 0, tracks_removed: 0, tracks_unchanged: 0 },
  error: null,
  skipped_unanalyzed: 0,
  elapsed_ms: 5,
};

let nextPlan: UsbSyncPlan;
let calls: { command: string; args: any }[];

beforeEach(() => {
  nextPlan = plan();
  calls = [];
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (command: string, args: any) => {
    calls.push({ command, args });
    switch (command) {
      case 'get_stick':
        return { stick_id: 'abc', track_ids: [7, 8], playlist_ids: [2] };
      case 'plan_usb_sync':
        return nextPlan;
      case 'sync_usb_many':
        return [doneReport];
      case 'get_volume_capacity':
        return { total_bytes: 64e9, free_bytes: 22.8e9 };
      default:
        return undefined;
    }
  });
});

const COLD_IMPORT_TIMEOUT_MS = 60_000;

beforeAll(async () => {
  await import('./sticks.svelte.ts');
  await import('./stick-sync.svelte.ts');
}, COLD_IMPORT_TIMEOUT_MS);

async function fresh() {
  vi.resetModules();
  const sticks = await import('./sticks.svelte.ts');
  const sync = await import('./stick-sync.svelte.ts');
  await sticks.loadStick(STICK);
  return { ...sticks, ...sync };
}

describe('sync target', () => {
  it('is exactly the stick All tracks and the playlists put on it', async () => {
    const { stickSyncTarget } = await fresh();
    expect(stickSyncTarget(STICK)).toEqual({
      volume: STICK,
      selection: { kind: 'playlists', playlist_ids: [2], track_ids: [7, 8] },
    });
  });
});

describe('when to ask', () => {
  it('asks only for removals, won’t fit, unanalysed or foreign tracks', async () => {
    const { planNeedsConfirm } = await fresh();
    const track = { track_id: 3, title: 'x', artist: 'y', size_bytes: 1 };
    expect(planNeedsConfirm(plan())).toBe(false);
    expect(planNeedsConfirm(plan({ remove: [track] }))).toBe(true);
    expect(planNeedsConfirm(plan({ fits: false }))).toBe(true);
    expect(planNeedsConfirm(plan({ skipped_unanalyzed: [track] }))).toBe(true);
    expect(planNeedsConfirm(plan({ foreign_kept_count: 2 }))).toBe(true);
    expect(planNeedsConfirm(plan({ fits: null }))).toBe(false);
  });
});

describe('sync flow', () => {
  it('writes straight away when nothing needs an answer', async () => {
    const { syncStick, stickSync, syncStatusLine } = await fresh();
    await syncStick(STICK);
    const syncCall = calls.find((c) => c.command === 'sync_usb_many');
    expect(syncCall?.args.targets[0].selection).toEqual({ kind: 'playlists', playlist_ids: [2], track_ids: [7, 8] });
    expect(stickSync.stage).toBe('done');
    expect(syncStatusLine()).toBe('Synced · 1 added · Safe to eject');
  });

  it('stops at the confirm when files would be removed, then writes on confirm', async () => {
    nextPlan = plan({ remove: [{ track_id: 3, title: 'Drift', artist: 'Bicep', size_bytes: 1 }] });
    const { syncStick, confirmStickSync, stickSync } = await fresh();
    await syncStick(STICK);
    expect(stickSync.stage).toBe('confirm');
    expect(calls.some((c) => c.command === 'sync_usb_many')).toBe(false);
    await confirmStickSync();
    expect(stickSync.stage).toBe('done');
    expect(calls.some((c) => c.command === 'sync_usb_many')).toBe(true);
  });

  it('never writes when it will not fit', async () => {
    nextPlan = plan({ fits: false, free_bytes: 10 });
    const { syncStick, confirmStickSync, stickSync } = await fresh();
    await syncStick(STICK);
    expect(stickSync.stage).toBe('confirm');
    await confirmStickSync();
    expect(stickSync.stage).toBe('confirm');
    expect(calls.some((c) => c.command === 'sync_usb_many')).toBe(false);
  });

  it('cancel leaves the stick untouched', async () => {
    nextPlan = plan({ skipped_unanalyzed: [{ track_id: 9, title: 'Raw', artist: 'A', size_bytes: 1 }] });
    const { syncStick, cancelStickSync, stickSync } = await fresh();
    await syncStick(STICK);
    cancelStickSync();
    expect(stickSync.stage).toBe('idle');
    expect(calls.some((c) => c.command === 'sync_usb_many')).toBe(false);
  });

  it('shows the engine error when the write fails', async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_stick') return { stick_id: 'abc', track_ids: [7], playlist_ids: [] };
      if (command === 'plan_usb_sync') return plan();
      if (command === 'sync_usb_many')
        return [{ ...doneReport, status: 'failed', report: null, error: 'Not enough space on SanDisk 64GB' }];
      return undefined;
    });
    const { syncStick, stickSync, syncStatusLine } = await fresh();
    await syncStick(STICK);
    expect(stickSync.stage).toBe('failed');
    expect(syncStatusLine()).toBe('Not enough space on SanDisk 64GB');
  });
});

describe('capacity label', () => {
  it('reads used of total', async () => {
    const { capacityLabel } = await fresh();
    expect(capacityLabel({ total_bytes: 64e9, free_bytes: 22.8e9 })).toBe('41.2 of 64 GB');
    expect(capacityLabel({ total_bytes: 8e9, free_bytes: 1e8 })).toBe('7.9 of 8.0 GB');
    expect(capacityLabel(null)).toBeNull();
  });
});
