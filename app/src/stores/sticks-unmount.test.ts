import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

const STICK = '/Volumes/SanDisk 64GB';

let releaseGetStick: () => void;

beforeAll(async () => {
  await import('./sticks.svelte.ts');
}, 60_000);

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (command: string) => {
    if (command === 'get_stick') {
      await new Promise<void>((resolve) => (releaseGetStick = resolve));
      return { stick_id: 'abc', track_ids: [7], playlist_ids: [] };
    }
    if (command === 'get_volume_capacity') {
      await new Promise<void>((resolve) => (releaseGetStick = resolve));
      return { total_bytes: 64e9, free_bytes: 22.8e9 };
    }
    return undefined;
  });
});

describe('a stick unmounted while it loads', () => {
  it('does not come back after the load finishes', async () => {
    vi.resetModules();
    const { loadStick, loadSticks, sticks } = await import('./sticks.svelte.ts');
    const loading = loadStick(STICK);
    await loadSticks([]);
    releaseGetStick();
    await loading;
    expect(sticks.byPath[STICK]).toBeUndefined();
  });

  it('does not keep its capacity either', async () => {
    vi.resetModules();
    const { loadCapacity, loadSticks, sticks } = await import('./sticks.svelte.ts');
    const loading = loadCapacity(STICK);
    await loadSticks([]);
    releaseGetStick();
    await loading;
    expect(sticks.capacity[STICK]).toBeUndefined();
  });
});
