import { getTrackArtwork, ARTWORK_ROW_PX } from '../services/tauri.svelte.ts';

// Reactive state containing the cached blob URLs
let cache = $state<Record<number, string>>({});
/** Bumped on clear() so in-flight fetches from a prior library session are ignored. */
let cacheGeneration = 0;
const inflight = new Map<number, Promise<string | null>>();

let rowCache = $state<Record<number, string>>({});
const ROW_FETCH_CONCURRENCY = 4;
const ROW_QUEUE_LIMIT = 64;
const rowQueue: number[] = [];
const rowQueued = new Set<number>();
let rowActive = 0;

function pumpRowQueue() {
  while (rowActive < ROW_FETCH_CONCURRENCY && rowQueue.length > 0) {
    const trackId = rowQueue.pop()!;
    rowQueued.delete(trackId);
    const gen = cacheGeneration;
    rowActive++;
    getTrackArtwork(trackId, ARTWORK_ROW_PX)
      .then((bytes) => {
        if (gen !== cacheGeneration) return;
        rowCache[trackId] = bytes ? URL.createObjectURL(new Blob([bytes], { type: 'image/jpeg' })) : '';
      })
      .finally(() => {
        rowActive--;
        pumpRowQueue();
      });
  }
}

export const artwork = {
  /**
   * Get a blob URL reactively. Triggers fetch in background if missing and track has artwork.
   */
  get(trackId: number, hasArtwork: boolean | undefined): string {
    if (!trackId || !hasArtwork) return '';
    
    const cachedUrl = cache[trackId];
    if (cachedUrl !== undefined) return cachedUrl;

    void this.fetch(trackId);
    return ''; // Return empty string while loading
  },

  /**
   * Small thumbnail for list rows. Requests wait in a short newest-first queue, so rows that
   * scrolled past before their turn are dropped instead of fetched.
   */
  getRow(trackId: number, hasArtwork: boolean | undefined): string {
    if (!trackId || !hasArtwork) return '';
    const cachedUrl = rowCache[trackId];
    if (cachedUrl !== undefined) return cachedUrl;
    if (!rowQueued.has(trackId)) {
      rowQueued.add(trackId);
      rowQueue.push(trackId);
      if (rowQueue.length > ROW_QUEUE_LIMIT) rowQueued.delete(rowQueue.shift()!);
      queueMicrotask(pumpRowQueue);
    }
    return '';
  },

  /**
   * Asynchronously fetch and cache artwork
   */
  async fetch(trackId: number) {
    if (cache[trackId] !== undefined || inflight.has(trackId)) return;

    const gen = cacheGeneration;
    const promise = (async () => {
      try {
        const bytes = await getTrackArtwork(trackId);
        if (gen !== cacheGeneration) return null;
        if (bytes) {
          const blob = new Blob([bytes], { type: 'image/jpeg' });
          const url = URL.createObjectURL(blob);
          cache[trackId] = url;
          return url;
        }
        cache[trackId] = ''; // No bytes — negative cache
      } catch (err) {
        if (gen !== cacheGeneration) return null;
        console.warn(`Failed to fetch artwork for track ${trackId}:`, err);
        cache[trackId] = '';
      } finally {
        inflight.delete(trackId);
      }
      return null;
    })();

    inflight.set(trackId, promise);
    await promise;
  },

  /**
   * Revoke all object URLs and clear the cache to prevent memory leaks.
   */
  clear() {
    cacheGeneration++;
    for (const key of Object.keys(cache)) {
      const url = cache[Number(key)];
      if (url && url.startsWith('blob:')) {
        try {
          URL.revokeObjectURL(url);
        } catch (err) {
          console.warn(`Failed to revoke object URL for track ${key}:`, err);
        }
      }
    }
    cache = {}; // Reset the Svelte state reactively
    inflight.clear();
    for (const url of Object.values(rowCache)) {
      if (url) URL.revokeObjectURL(url);
    }
    rowCache = {};
    rowQueue.length = 0;
    rowQueued.clear();
  }
};
