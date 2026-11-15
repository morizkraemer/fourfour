/**
 * selection.svelte.ts — Track selection state.
 * Drives row highlighting, detail pane mode, and multi-select operations.
 * Rewritten to use Track IDs for stability with filtered and scanned list views.
 */

import { library } from './library.svelte.ts';
import { inspectorFollowsSelection } from './ui.svelte.ts';
import { artwork } from './artwork.svelte.ts';

/* ── Reactive state ─────────────────────────────────────────────────── */

export const selection = $state({
  /** Track IDs that are selected */
  ids: new Set<number>(),
});

artwork.keep('inspector', () => (selection.ids.size === 1 ? selection.ids : []));

export function selectionCount() {
  return selection.ids.size;
}

/** The single selected track, or null if 0 or 2+ selected. */
export function selectedTrack() {
  if (selection.ids.size !== 1) return null;
  const id = selection.ids.values().next().value;
  return library.tracks.find(t => t.id === id) ?? null;
}

/** All selected tracks (for multi-select detail pane). */
export function selectedTracks() {
  return library.tracks.filter(t => selection.ids.has(t.id));
}

/* ── Actions ────────────────────────────────────────────────────────── */

/** Select a single track, clearing previous selection. */
export function select(id: number) {
  selection.ids = new Set([id]);
  inspectorFollowsSelection(1);
}

/** Toggle a track in/out of the selection (Cmd+click). */
export function toggle(id: number) {
  const next = new Set(selection.ids);
  const adding = !next.has(id);
  if (adding) next.add(id);
  else next.delete(id);
  selection.ids = next;
  if (adding) inspectorFollowsSelection(next.size);
}

/** Select a contiguous range (Shift+click). */
export function selectRange(fromId: number, toId: number, currentList: any[]) {
  const fromIdx = currentList.findIndex(t => t.id === fromId);
  const toIdx = currentList.findIndex(t => t.id === toId);
  if (fromIdx === -1 || toIdx === -1) return;
  const lo = Math.min(fromIdx, toIdx);
  const hi = Math.max(fromIdx, toIdx);
  const next = new Set(selection.ids);
  for (let i = lo; i <= hi; i++) {
    if (currentList[i]) {
      next.add(currentList[i].id);
    }
  }
  selection.ids = next;
  inspectorFollowsSelection(next.size);
}

/** Select all tracks. */
export function selectAll(currentList: any[]) {
  const next = new Set<number>();
  currentList.forEach(t => next.add(t.id));
  selection.ids = next;
  inspectorFollowsSelection(next.size);
}

/** Replace selection with the given track IDs. */
export function setSelection(ids: Iterable<number>) {
  selection.ids = new Set(ids);
  inspectorFollowsSelection(selection.ids.size);
}

/** Clear selection. */
export function clear() {
  selection.ids = new Set();
}

