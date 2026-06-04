/**
 * selection.svelte.ts — Track selection state.
 * Drives row highlighting, detail pane mode, and multi-select operations.
 */

import { tracks } from './library.svelte.ts';

/* ── Reactive state ─────────────────────────────────────────────────── */

/** Selection state: set of selected track indices (0-based). */
export const selection = $state({
  /** Track indices that are selected */
  indices: new Set([1]),  // Voidwalker pre-selected
});

export function selectionCount() {
  return selection.indices.size;
}

/** The single selected track, or null if 0 or 2+ selected. */
export function selectedTrack() {
  if (selection.indices.size !== 1) return null;
  const idx = selection.indices.values().next().value;
  return tracks[idx] ?? null;
}

/** All selected tracks (for multi-select detail pane). */
export function selectedTracks() {
  return [...selection.indices].sort((a, b) => a - b).map(i => tracks[i]).filter(Boolean);
}

/* ── Actions ────────────────────────────────────────────────────────── */

/** Select a single track, clearing previous selection. */
export function select(index) {
  selection.indices = new Set([index]);
}

/** Toggle a track in/out of the selection (Cmd+click). */
export function toggle(index) {
  const next = new Set(selection.indices);
  if (next.has(index)) {
    next.delete(index);
  } else {
    next.add(index);
  }
  selection.indices = next;
}

/** Select a contiguous range (Shift+click). */
export function selectRange(from, to) {
  const lo = Math.min(from, to);
  const hi = Math.max(from, to);
  const next = new Set();
  for (let i = lo; i <= hi; i++) next.add(i);
  selection.indices = next;
}

/** Select all tracks. */
export function selectAll() {
  const next = new Set();
  for (let i = 0; i < tracks.length; i++) next.add(i);
  selection.indices = next;
}

/** Clear selection. */
export function clear() {
  selection.indices = new Set();
}
