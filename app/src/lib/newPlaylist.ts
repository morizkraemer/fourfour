export const ORGANIZE_DOCK_MIN_WINDOW_PX = 1100;

export function canDockOrganizeAt(windowWidth: number): boolean {
  return windowWidth >= ORGANIZE_DOCK_MIN_WINDOW_PX;
}

export function favouriteTrackIds(
  playlists: Array<{ name: string; track_ids: number[] }>,
  slotOf: (name: string) => number | null,
  onlySlot?: number
): number[] {
  const ordered = playlists
    .map((p) => ({ slot: slotOf(p.name), ids: p.track_ids }))
    .filter((p): p is { slot: number; ids: number[] } => p.slot != null)
    .filter((p) => onlySlot == null || p.slot === onlySlot)
    .sort((a, b) => a.slot - b.slot);
  return [...new Set(ordered.flatMap((p) => p.ids))];
}
