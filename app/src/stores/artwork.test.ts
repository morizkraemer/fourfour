import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const hooks = vi.hoisted(() => ({ getTrackArtwork: vi.fn() }));
vi.mock('../services/tauri.svelte.ts', () => ({
  getTrackArtwork: hooks.getTrackArtwork,
  ARTWORK_ROW_PX: 64,
}));

const { artwork } = await import('./artwork.svelte.ts');

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

async function scrollThrough(count: number) {
  for (let id = 1; id <= count; id++) {
    artwork.getRow(id, true);
    await settle();
  }
  await settle();
}

describe('row artwork cache', () => {
  let live: Set<string>;
  let created = 0;

  beforeEach(() => {
    live = new Set();
    created = 0;
    hooks.getTrackArtwork.mockImplementation(async () => new ArrayBuffer(3000));
    vi.spyOn(URL, 'createObjectURL').mockImplementation(() => {
      const url = `blob:test/${created++}`;
      live.add(url);
      return url;
    });
    vi.spyOn(URL, 'revokeObjectURL').mockImplementation((url) => {
      live.delete(url);
    });
  });

  afterEach(() => {
    artwork.clear();
    vi.restoreAllMocks();
  });

  it('keeps a bounded number of thumbnails while a long list is scrolled through', async () => {
    await scrollThrough(5000);
    expect(created).toBe(5000);
    expect(live.size).toBeLessThanOrEqual(2000);
    expect(live.size).toBeGreaterThan(1000);
  });

  it('serves a thumbnail that was just fetched from the cache without fetching again', async () => {
    await scrollThrough(10);
    const fetches = hooks.getTrackArtwork.mock.calls.length;
    expect(artwork.getRow(10, true)).toMatch(/^blob:/);
    expect(hooks.getTrackArtwork.mock.calls.length).toBe(fetches);
  });

  it('fetches an evicted thumbnail again when its row comes back', async () => {
    await scrollThrough(5000);
    expect(artwork.getRow(1, true)).toBe('');
    await settle();
    expect(artwork.getRow(1, true)).toMatch(/^blob:/);
  });

  it('revokes every thumbnail when the library is cleared', async () => {
    await scrollThrough(300);
    artwork.clear();
    expect(live.size).toBe(0);
  });
});
