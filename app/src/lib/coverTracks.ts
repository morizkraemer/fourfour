interface CoverTrack {
  id: number;
  raw?: { has_artwork?: boolean };
}

export function trackIdsWithCover(tracks: readonly CoverTrack[], limit = Infinity): number[] {
  const ids: number[] = [];
  for (const track of tracks) {
    if (ids.length >= limit) break;
    if (track.raw?.has_artwork) ids.push(track.id);
  }
  return ids;
}
