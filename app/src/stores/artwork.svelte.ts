import { getTrackArtwork } from '../services/tauri.svelte.ts';

// Reactive state containing the cached blob URLs
let cache = $state<Record<number, string>>({});
/** Bumped on clear() so in-flight fetches from a prior library session are ignored. */
let cacheGeneration = 0;
const inflight = new Map<number, Promise<string | null>>();

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
   * Asynchronously fetch and cache artwork
   */
  async fetch(trackId: number) {
    if (cache[trackId] !== undefined || inflight.has(trackId)) return;

    const gen = cacheGeneration;
    const promise = (async () => {
      try {
        const bytes = await getTrackArtwork(trackId);
        if (gen !== cacheGeneration) return null;
        if (bytes && bytes.length > 0) {
          const blob = new Blob([new Uint8Array(bytes)], { type: 'image/jpeg' });
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
  }
};
