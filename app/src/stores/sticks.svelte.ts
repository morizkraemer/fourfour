const ALL_TRACKS_KEY = 'fourfour.sticks.allTracks';

function load(): Record<string, number[]> {
  try {
    const parsed = JSON.parse(localStorage.getItem(ALL_TRACKS_KEY) ?? '{}');
    return parsed && typeof parsed === 'object' && !Array.isArray(parsed) ? parsed : {};
  } catch {
    return {};
  }
}

function persist() {
  try {
    localStorage.setItem(ALL_TRACKS_KEY, JSON.stringify(sticks.allTracks));
  } catch {
    return;
  }
}

/** Per stick (keyed by volume path): the stick's own "All tracks" list that syncs to it. */
export const sticks = $state({ allTracks: load() });

export function stickAllTrackIds(volumePath: string): number[] {
  return sticks.allTracks[volumePath] ?? [];
}

/** Adds the tracks the stick does not hold yet; returns how many were added. */
export function addTracksToStickAll(volumePath: string, trackIds: number[]): number {
  const held = stickAllTrackIds(volumePath);
  const fresh = [...new Set(trackIds)].filter((id) => !held.includes(id));
  if (fresh.length === 0) return 0;
  sticks.allTracks[volumePath] = [...held, ...fresh];
  persist();
  return fresh.length;
}
