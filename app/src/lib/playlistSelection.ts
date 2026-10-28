import { descendantIds, type TreePlaylist } from './playlistTree.ts';

export interface PlaylistSelection {
  ids: number[];
  anchor: number | null;
}

export interface SelectionModifiers {
  shift: boolean;
  toggle: boolean;
}

export const EMPTY_SELECTION: PlaylistSelection = { ids: [], anchor: null };

export function nextPlaylistSelection(
  order: readonly number[],
  current: PlaylistSelection,
  clickedId: number,
  mods: SelectionModifiers
): PlaylistSelection {
  if (mods.toggle) {
    const ids = current.ids.includes(clickedId)
      ? current.ids.filter((id) => id !== clickedId)
      : [...current.ids, clickedId];
    return { ids, anchor: clickedId };
  }
  const anchorIndex = current.anchor == null ? -1 : order.indexOf(current.anchor);
  const clickedIndex = order.indexOf(clickedId);
  if (mods.shift && anchorIndex >= 0 && clickedIndex >= 0) {
    const [from, to] = anchorIndex < clickedIndex ? [anchorIndex, clickedIndex] : [clickedIndex, anchorIndex];
    return { ids: order.slice(from, to + 1), anchor: current.anchor };
  }
  return { ids: [clickedId], anchor: clickedId };
}

export function pruneSelection(selection: PlaylistSelection, existing: ReadonlySet<number>): PlaylistSelection {
  const ids = selection.ids.filter((id) => existing.has(id));
  if (ids.length === selection.ids.length) return selection;
  return { ids, anchor: selection.anchor != null && existing.has(selection.anchor) ? selection.anchor : null };
}

/** Selected items that are not inside another selected folder, in the order given. */
export function outermostIds(playlists: readonly TreePlaylist[], ids: readonly number[]): number[] {
  const covered = new Set<number>();
  for (const id of ids) {
    if (playlists.find((p) => p.id === id)?.is_folder) {
      for (const inner of descendantIds(playlists, id)) covered.add(inner);
    }
  }
  return ids.filter((id) => !covered.has(id));
}

/** Playlists (not folders) a selection stands for: selected playlists plus everything inside selected folders. */
export function playlistIdsInSelection(playlists: readonly TreePlaylist[], ids: readonly number[]): number[] {
  const found = new Set<number>();
  for (const id of outermostIds(playlists, ids)) {
    const item = playlists.find((p) => p.id === id);
    if (!item) continue;
    if (!item.is_folder) {
      found.add(id);
      continue;
    }
    for (const inner of descendantIds(playlists, id)) {
      if (!playlists.find((p) => p.id === inner)?.is_folder) found.add(inner);
    }
  }
  return [...found];
}
