import { beforeEach, describe, expect, it, vi } from 'vitest';

const invokeMock = vi.hoisted(() => vi.fn());
const storage = vi.hoisted(() => new Map<string, string>());

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

async function fresh() {
  vi.resetModules();
  const library = await import('./library.svelte.ts');
  const drop = await import('./organize-drop.svelte.ts');
  const sticks = await import('./sticks.svelte.ts');
  library.library.playlists = [
    { id: 1, name: 'Peak Time', track_ids: [10, 11] },
    { id: 2, name: 'Warmup', track_ids: [] },
  ];
  return { library: library.library, ...drop, ...sticks };
}

beforeEach(() => {
  storage.clear();
  vi.stubGlobal('localStorage', {
    getItem: (key: string) => storage.get(key) ?? null,
    setItem: (key: string, value: string) => void storage.set(key, value),
    removeItem: (key: string) => void storage.delete(key),
  });
  invokeMock.mockReset();
  invokeMock.mockResolvedValue(undefined);
});

describe('drop on a playlist card', () => {
  it('adds the dragged tracks to the playlist', async () => {
    const { library, dropTracksOnCard } = await fresh();
    const added = await dropTracksOnCard({ kind: 'playlist', playlistId: 2 }, [10, 12]);
    expect(added).toBe(2);
    expect(library.playlists[1].track_ids).toEqual([10, 12]);
  });

  it('does not add a track twice', async () => {
    const { library, dropTracksOnCard } = await fresh();
    const added = await dropTracksOnCard({ kind: 'playlist', playlistId: 1 }, [10, 11, 12, 12]);
    expect(added).toBe(1);
    expect(library.playlists[0].track_ids).toEqual([10, 11, 12]);
  });

  it('confirms the count that was added', async () => {
    const { dropFlash, dropTracksOnCard } = await fresh();
    await dropTracksOnCard({ kind: 'playlist', playlistId: 2 }, [1, 2, 3]);
    expect(dropFlash.key).toBe('playlist:2');
    expect(dropFlash.text).toBe('Added 3');
  });

  it('says so when everything was already there', async () => {
    const { dropFlash, dropTracksOnCard } = await fresh();
    await dropTracksOnCard({ kind: 'playlist', playlistId: 1 }, [10, 11]);
    expect(dropFlash.text).toBe('All already there');
  });
});

describe('drop on a stick card', () => {
  it('puts the tracks on the stick All tracks list', async () => {
    const { dropTracksOnCard, stickAllTrackIds } = await fresh();
    const added = await dropTracksOnCard({ kind: 'stick', volumePath: '/Volumes/A' }, [5, 6]);
    expect(added).toBe(2);
    expect(stickAllTrackIds('/Volumes/A')).toEqual([5, 6]);
    expect(stickAllTrackIds('/Volumes/B')).toEqual([]);
  });

  it('skips tracks the stick already holds', async () => {
    const { dropTracksOnCard, dropFlash, stickAllTrackIds } = await fresh();
    await dropTracksOnCard({ kind: 'stick', volumePath: '/Volumes/A' }, [5, 6]);
    await dropTracksOnCard({ kind: 'stick', volumePath: '/Volumes/A' }, [6, 7]);
    expect(stickAllTrackIds('/Volumes/A')).toEqual([5, 6, 7]);
    expect(dropFlash.text).toBe('Added 1 · 1 already there');
  });

  it('keeps the list across restarts', async () => {
    const first = await fresh();
    await first.dropTracksOnCard({ kind: 'stick', volumePath: '/Volumes/A' }, [5]);
    const second = await fresh();
    expect(second.stickAllTrackIds('/Volumes/A')).toEqual([5]);
  });
});
