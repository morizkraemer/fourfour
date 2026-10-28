import {
  nextPlaylistSelection,
  pruneSelection,
  type SelectionModifiers,
} from '../lib/playlistSelection.ts';

export const sidebarSelection = $state({ ids: [] as number[], anchor: null as number | null });

export function selectSidebarPlaylist(order: readonly number[], id: number, mods: SelectionModifiers) {
  const next = nextPlaylistSelection(order, sidebarSelection, id, mods);
  sidebarSelection.ids = next.ids;
  sidebarSelection.anchor = next.anchor;
}

export function clearSidebarSelection() {
  sidebarSelection.ids = [];
  sidebarSelection.anchor = null;
}

export function pruneSidebarSelection(existing: ReadonlySet<number>) {
  const next = pruneSelection(sidebarSelection, existing);
  if (next === sidebarSelection) return;
  sidebarSelection.ids = next.ids;
  sidebarSelection.anchor = next.anchor;
}

export function isMultiSelected(id: number): boolean {
  return sidebarSelection.ids.length > 1 && sidebarSelection.ids.includes(id);
}
