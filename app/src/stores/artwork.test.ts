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

describe('row artwork focus', () => {
  let started: number[];
  let releases: (() => void)[];

  beforeEach(() => {
    started = [];
    releases = [];
    hooks.getTrackArtwork.mockReset();
    hooks.getTrackArtwork.mockImplementation(
      (id: number) =>
        new Promise<ArrayBuffer>((resolve) => {
          started.push(id);
          releases.push(() => resolve(new ArrayBuffer(3000)));
        }),
    );
    vi.spyOn(URL, 'createObjectURL').mockImplementation(() => 'blob:focus');
    vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => undefined);
  });

  afterEach(() => {
    artwork.clear();
    vi.restoreAllMocks();
  });

  const rows = (ids: number[]) => ids.map((id) => ({ id, hasArtwork: true }));

  it('fetches the most urgent rows first and drops queued rows that left the list focus', async () => {
    artwork.focusRows('list', rows(Array.from({ length: 40 }, (_, i) => 100 + i)));
    await settle();
    const firstWave = [...started];
    expect(firstWave).toEqual([100, 101, 102, 103, 104, 105]);
    artwork.focusRows('list', rows([500, 501]));
    releases.splice(0).forEach((release) => release());
    await settle();
    expect(started.slice(firstWave.length)).toEqual([500, 501]);
  });

  it('asks once for a row whose fetch is still running', async () => {
    artwork.focusRows('list', rows([7]));
    await settle();
    artwork.focusRows('list', rows([7]));
    artwork.getRow(7, true);
    await settle();
    expect(started).toEqual([7]);
  });

  it('keeps what another list still wants', async () => {
    artwork.focusRows('a', rows([1, 2, 3, 4, 5, 6]));
    artwork.focusRows('b', rows([900]));
    await settle();
    artwork.focusRows('a', rows([10]));
    releases.splice(0).forEach((release) => release());
    await settle();
    expect(started.slice(6).sort((x, y) => x - y)).toEqual([10, 900].filter((id) => !started.slice(0, 6).includes(id)));
  });
});

describe('cover artwork cache', () => {
  let live: Set<string>;
  let created = 0;
  const COVER_LIMIT = 128;

  beforeEach(() => {
    live = new Set();
    created = 0;
    hooks.getTrackArtwork.mockReset();
    hooks.getTrackArtwork.mockImplementation(async () => new ArrayBuffer(3000));
    vi.spyOn(URL, 'createObjectURL').mockImplementation(() => {
      const url = `blob:cover/${created++}`;
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

  async function select(id: number) {
    artwork.get(id, true);
    await settle();
  }

  it('keeps a bounded number of covers while a thousand tracks are selected one after another', async () => {
    for (let id = 1; id <= 1000; id++) await select(id);
    expect(created).toBe(1000);
    expect(live.size).toBeLessThanOrEqual(COVER_LIMIT);
    expect(live.size).toBeGreaterThan(16);
  });

  it('never revokes the cover of a track that is in use while others are evicted', async () => {
    const stop = artwork.keep('player', () => [1]);
    await select(1);
    const url = artwork.get(1, true);
    expect(url).toMatch(/^blob:/);
    for (let id = 2; id <= 600; id++) await select(id);
    expect(live.has(url)).toBe(true);
    expect(artwork.get(1, true)).toBe(url);
    expect(live.size).toBeLessThanOrEqual(COVER_LIMIT + 1);
    stop();
  });

  it('keeps every cover that any consumer still shows, even above the limit', async () => {
    const shown = Array.from({ length: 150 }, (_, i) => i + 1);
    const stopA = artwork.keep('a', () => shown.slice(0, 75));
    const stopB = artwork.keep('b', () => shown.slice(75));
    for (const id of shown) await select(id);
    for (let id = 151; id <= 400; id++) await select(id);
    const urls = shown.map((id) => artwork.get(id, true));
    expect(urls.every((url) => /^blob:/.test(url) && live.has(url))).toBe(true);
    stopA();
    stopB();
  });

  it('lets a cover go once its consumer stops using it', async () => {
    const stop = artwork.keep('inspector', () => [1]);
    await select(1);
    const url = artwork.get(1, true);
    stop();
    for (let id = 2; id <= 600; id++) await select(id);
    expect(live.has(url)).toBe(false);
  });

  it('loads an evicted cover again when its track is selected again', async () => {
    for (let id = 1; id <= 600; id++) await select(id);
    expect(artwork.get(1, true)).toBe('');
    await settle();
    expect(artwork.get(1, true)).toMatch(/^blob:/);
    expect(hooks.getTrackArtwork.mock.calls.filter(([id]) => id === 1).length).toBe(2);
  });

  it('serves a recently selected cover again without fetching it twice', async () => {
    await select(5);
    const fetches = hooks.getTrackArtwork.mock.calls.length;
    expect(artwork.get(5, true)).toMatch(/^blob:/);
    expect(hooks.getTrackArtwork.mock.calls.length).toBe(fetches);
  });

  it('revokes every cover when the library is cleared', async () => {
    const stop = artwork.keep('player', () => [1]);
    for (let id = 1; id <= 50; id++) await select(id);
    artwork.clear();
    expect(live.size).toBe(0);
    stop();
  });
});
