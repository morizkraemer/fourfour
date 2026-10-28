export function firstDroppedTrack<T extends { id: number }>(trackIds: number[], tracks: T[]): T | null {
  const first = trackIds[0];
  return first === undefined ? null : (tracks.find((t) => t.id === first) ?? null);
}
