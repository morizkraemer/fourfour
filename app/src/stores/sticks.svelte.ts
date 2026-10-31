import {
  getStick,
  getVolumeCapacity,
  stickAddPlaylists,
  stickAddTracks,
  stickRemovePlaylists,
  stickRemoveTracks,
  stickSetKeepUpdated,
  type StickContents,
  type VolumeCapacity,
} from '../services/tauri.svelte.ts';

const LEGACY_ALL_TRACKS_KEY = 'fourfour.sticks.allTracks';

/** Per mounted stick (keyed by volume path): what the library remembers for it, by stick id. */
export const sticks = $state({
  byPath: {} as Record<string, StickContents>,
  capacity: {} as Record<string, VolumeCapacity | null>,
  error: '' as string,
});

const requested = new Set<string>();
const unmounted = new Set<string>();

function startLoading(volumePath: string) {
  requested.add(volumePath);
  unmounted.delete(volumePath);
}

function legacyAllTracks(): Record<string, number[]> {
  try {
    const parsed = JSON.parse(localStorage.getItem(LEGACY_ALL_TRACKS_KEY) ?? '{}');
    return parsed && typeof parsed === 'object' && !Array.isArray(parsed) ? parsed : {};
  } catch {
    return {};
  }
}

function forgetLegacy(volumePath: string) {
  try {
    const rest = legacyAllTracks();
    delete rest[volumePath];
    if (Object.keys(rest).length === 0) localStorage.removeItem(LEGACY_ALL_TRACKS_KEY);
    else localStorage.setItem(LEGACY_ALL_TRACKS_KEY, JSON.stringify(rest));
  } catch {
    return;
  }
}

export function stickAllTrackIds(volumePath: string): number[] {
  return sticks.byPath[volumePath]?.track_ids ?? [];
}

export function stickPlaylistIds(volumePath: string): number[] {
  return sticks.byPath[volumePath]?.playlist_ids ?? [];
}

export function stickName(volumePath: string): string {
  return volumePath.replace(/\\/g, '/').split('/').pop() || volumePath;
}

/** False when the engine does not report per-playlist "Keep updated" yet; the switch then stays on. */
export function keepUpdatedSupported(volumePath: string): boolean {
  return Array.isArray(sticks.byPath[volumePath]?.kept_as_is);
}

/** The date a playlist was kept as it is on the stick, or null when it follows the library. */
export function keptAsIsSince(volumePath: string, playlistId: number): string | null {
  return sticks.byPath[volumePath]?.kept_as_is?.find((k) => k.playlist_id === playlistId)?.as_of ?? null;
}

export function setKeepUpdated(volumePath: string, playlistId: number, keepUpdated: boolean): Promise<number> {
  return edit(volumePath, () => stickSetKeepUpdated(volumePath, playlistId, keepUpdated));
}

/** Reads the stick's lists from the library; moves a list left in localStorage by an older build into it. */
export async function loadStick(volumePath: string): Promise<void> {
  startLoading(volumePath);
  try {
    const contents = await getStick(volumePath);
    if (unmounted.has(volumePath)) return;
    sticks.byPath[volumePath] = contents;
    const legacy = legacyAllTracks()[volumePath];
    if (Array.isArray(legacy) && legacy.length > 0) {
      const [, migrated] = await stickAddTracks(volumePath, legacy);
      forgetLegacy(volumePath);
      if (unmounted.has(volumePath)) return;
      sticks.byPath[volumePath] = migrated;
    }
  } catch (err) {
    sticks.error = String(err);
  }
}

export async function loadCapacity(volumePath: string): Promise<void> {
  startLoading(volumePath);
  const capacity = await getVolumeCapacity(volumePath);
  if (unmounted.has(volumePath)) return;
  sticks.capacity[volumePath] = capacity;
}

export async function loadSticks(volumePaths: string[]): Promise<void> {
  for (const path of requested) {
    if (volumePaths.includes(path)) continue;
    unmounted.add(path);
    delete sticks.byPath[path];
    delete sticks.capacity[path];
  }
  await Promise.all(volumePaths.flatMap((path) => [loadStick(path), loadCapacity(path)]));
}

async function edit(volumePath: string, run: () => Promise<[number, StickContents]>): Promise<number> {
  try {
    const [changed, contents] = await run();
    sticks.byPath[volumePath] = contents;
    sticks.error = '';
    return changed;
  } catch (err) {
    sticks.error = String(err);
    return 0;
  }
}

/** Adds the tracks the stick does not hold yet; returns how many were added. */
export function addTracksToStickAll(volumePath: string, trackIds: number[]): Promise<number> {
  return edit(volumePath, () => stickAddTracks(volumePath, trackIds));
}

export function removeTracksFromStickAll(volumePath: string, trackIds: number[]): Promise<number> {
  return edit(volumePath, () => stickRemoveTracks(volumePath, trackIds));
}

/** Puts playlists on the stick; returns how many were new. */
export function addPlaylistsToStick(volumePath: string, playlistIds: number[]): Promise<number> {
  return edit(volumePath, () => stickAddPlaylists(volumePath, playlistIds));
}

export function removePlaylistFromStick(volumePath: string, playlistId: number): Promise<number> {
  return edit(volumePath, () => stickRemovePlaylists(volumePath, [playlistId]));
}

const GB = 1_000_000_000;

function gb(bytes: number): string {
  const value = bytes / GB;
  return value >= 100 ? String(Math.round(value)) : value.toFixed(1);
}

export interface StickSpace {
  total: string;
  used: string;
  free: string;
  usedPct: number;
}

/** Sizes as shown in the stick header ("29.8 GB", "7.6 GB used", "22.2 GB free"), or null when unknown. */
export function stickSpace(capacity: VolumeCapacity | null | undefined): StickSpace | null {
  if (!capacity || capacity.total_bytes <= 0) return null;
  const used = Math.max(0, capacity.total_bytes - capacity.free_bytes);
  return {
    total: `${gb(capacity.total_bytes)} GB`,
    used: `${gb(used)} GB`,
    free: `${gb(capacity.free_bytes)} GB`,
    usedPct: Math.min(100, (used / capacity.total_bytes) * 100),
  };
}

/** "41.2 of 64 GB" (used of total), or null when the capacity is unknown. */
export function capacityLabel(capacity: VolumeCapacity | null | undefined): string | null {
  if (!capacity || capacity.total_bytes <= 0) return null;
  const used = Math.max(0, capacity.total_bytes - capacity.free_bytes);
  const total = capacity.total_bytes / GB;
  const totalText = total >= 10 ? String(Math.round(total)) : total.toFixed(1);
  return `${(used / GB).toFixed(1)} of ${totalText} GB`;
}
