export interface TreePlaylist {
  id: number;
  name: string;
  track_ids: number[];
  parent_id: number;
  is_folder: boolean;
}

export interface SidebarTreeRow {
  id: number;
  name: string;
  depth: number;
  isFolder: boolean;
  expanded: boolean;
  trackCount: number;
}

export function sidebarTreeRows<T extends TreePlaylist>(
  playlists: readonly T[],
  collapsed: ReadonlySet<number>,
  hidden: (playlist: T) => boolean
): SidebarTreeRow[] {
  const rows: SidebarTreeRow[] = [];
  const visited = new Set<number>();
  const walk = (parentId: number, depth: number) => {
    for (const playlist of playlists) {
      if (playlist.parent_id !== parentId || visited.has(playlist.id)) continue;
      if (parentId === 0 && hidden(playlist)) continue;
      visited.add(playlist.id);
      const expanded = playlist.is_folder && !collapsed.has(playlist.id);
      rows.push({
        id: playlist.id,
        name: playlist.name,
        depth,
        isFolder: playlist.is_folder,
        expanded,
        trackCount: playlist.track_ids.length,
      });
      if (expanded) walk(playlist.id, depth + 1);
    }
  };
  walk(0, 0);
  return rows;
}

export function descendantIds(playlists: readonly TreePlaylist[], id: number): Set<number> {
  const found = new Set<number>();
  const pending = [id];
  while (pending.length > 0) {
    const parent = pending.pop()!;
    for (const playlist of playlists) {
      if (playlist.parent_id === parent && !found.has(playlist.id)) {
        found.add(playlist.id);
        pending.push(playlist.id);
      }
    }
  }
  return found;
}

export function canMoveInto(playlists: readonly TreePlaylist[], id: number, parentId: number): boolean {
  if (!playlists.some((p) => p.id === id)) return false;
  if (parentId === 0) return true;
  const parent = playlists.find((p) => p.id === parentId);
  if (!parent?.is_folder) return false;
  return parentId !== id && !descendantIds(playlists, id).has(parentId);
}

/**
 * Moves `id` into folder `parentId` (0 is the root). With an anchor the item lands right before it
 * (or after it); without one it becomes the folder's last child. Returns null when the move is not allowed.
 */
export function movePlaylist<T extends TreePlaylist>(
  playlists: readonly T[],
  id: number,
  parentId: number,
  anchorId: number | null = null,
  placeAfter = false
): T[] | null {
  if (!canMoveInto(playlists, id, parentId)) return null;
  if (anchorId === id) return null;
  const moved = { ...playlists.find((p) => p.id === id)!, parent_id: parentId };
  const rest = playlists.filter((p) => p.id !== id);
  const anchorIndex = anchorId == null ? -1 : rest.findIndex((p) => p.id === anchorId);
  const at = anchorIndex < 0 ? rest.length : anchorIndex + (placeAfter ? 1 : 0);
  return [...rest.slice(0, at), moved, ...rest.slice(at)];
}

export function removeWithDescendants<T extends TreePlaylist>(playlists: readonly T[], id: number): T[] {
  const gone = descendantIds(playlists, id);
  gone.add(id);
  return playlists.filter((p) => !gone.has(p.id));
}

export function uniqueName(taken: Iterable<string>, base: string): string {
  const used = new Set(taken);
  if (!used.has(base)) return base;
  let n = 2;
  while (used.has(`${base} ${n}`)) n += 1;
  return `${base} ${n}`;
}
