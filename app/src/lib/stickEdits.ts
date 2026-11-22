export function withoutTracks(trackIds: number[], removed: number[]): number[] {
  const gone = new Set(removed);
  return trackIds.filter((id) => !gone.has(id));
}

export function moveTracks(trackIds: number[], moved: number[], beforeIndex: number): number[] {
  const moving = new Set(moved);
  const picked = trackIds.filter((id) => moving.has(id));
  if (picked.length === 0) return trackIds;
  const insertAt = trackIds.slice(0, Math.max(0, Math.min(beforeIndex, trackIds.length))).filter((id) => !moving.has(id)).length;
  const rest = trackIds.filter((id) => !moving.has(id));
  return [...rest.slice(0, insertAt), ...picked, ...rest.slice(insertAt)];
}

export function sameOrder(a: number[], b: number[]): boolean {
  return a.length === b.length && a.every((id, i) => id === b[i]);
}

export interface KeptList {
  playlist_id: number;
  track_ids?: number[];
}

export function playlistTracksOnStick(
  playlistId: number,
  libraryTrackIds: number[],
  kept: KeptList[] | undefined
): number[] {
  return kept?.find((k) => k.playlist_id === playlistId)?.track_ids ?? libraryTrackIds;
}

export interface StickSelection {
  kind: 'playlists';
  playlist_ids: number[];
  track_ids: number[];
  kept: { playlist_id: number; track_ids: number[] }[];
}

export function stickSelection(contents: { track_ids: number[]; playlist_ids: number[]; kept_as_is?: KeptList[] } | undefined): StickSelection {
  const playlistIds = contents?.playlist_ids ?? [];
  return {
    kind: 'playlists',
    playlist_ids: playlistIds,
    track_ids: contents?.track_ids ?? [],
    kept: (contents?.kept_as_is ?? []).flatMap((k) =>
      k.track_ids && playlistIds.includes(k.playlist_id) ? [{ playlist_id: k.playlist_id, track_ids: k.track_ids }] : []
    ),
  };
}
