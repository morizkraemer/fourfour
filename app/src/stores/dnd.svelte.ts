/**
 * dnd.svelte.ts — Shared drag-and-drop session state (tracks, columns, playlists).
 */

import { ROW_HEIGHT } from '../lib/rowWindow.ts';

export type DragKind = 'tracks' | 'column' | 'playlist' | null;

export const dnd = $state({
  kind: null as DragKind,
  /** Track IDs being dragged */
  trackIds: [] as number[],
  /** Playlist the drag started from (null = library / all tracks) */
  sourcePlaylistId: null as number | null,
  /** Sidebar source id of the list panel the drag started in */
  dragOriginSourceId: null as string | null,
  ghostLabel: '',
  ghostCount: 1,
  pointerX: 0,
  pointerY: 0,
  /** Row insert index in the hovered table body */
  dropRowIndex: null as number | null,
  /** Sidebar source id of the list panel receiving a track drop */
  dropPanelSourceId: null as string | null,
  /** Sidebar playlist receiving a drop */
  dropPlaylistId: null as number | null,
  /** Column key being dragged or hovered for reorder */
  columnDragKey: null as string | null,
  columnDropKey: null as string | null,
  /** Playlist row reorder in sidebar */
  playlistDragId: null as number | null,
  playlistDropId: null as number | null,
});

export function startTrackDrag(
  trackIds: number[],
  label: string,
  sourcePlaylistId: number | null,
  originSourceId: string,
  x: number,
  y: number
) {
  dnd.kind = 'tracks';
  dnd.trackIds = trackIds;
  dnd.sourcePlaylistId = sourcePlaylistId;
  dnd.dragOriginSourceId = originSourceId;
  dnd.ghostLabel = label;
  dnd.ghostCount = trackIds.length;
  dnd.pointerX = x;
  dnd.pointerY = y;
  dnd.dropRowIndex = null;
  dnd.dropPanelSourceId = null;
  dnd.dropPlaylistId = null;
}

/** Hit-test track list panels under the pointer. */
export function updateTrackDropTarget(clientX: number, clientY: number) {
  if (dnd.kind !== 'tracks') return;

  const hit = document.elementFromPoint(clientX, clientY);
  if (!hit) {
    dnd.dropPanelSourceId = null;
    dnd.dropRowIndex = null;
    return;
  }

  const body = hit.closest('[data-track-drop-body]');
  if (!body) {
    dnd.dropPanelSourceId = null;
    dnd.dropRowIndex = null;
    return;
  }

  const sourceId = body.getAttribute('data-source-id');
  if (!sourceId) {
    dnd.dropPanelSourceId = null;
    dnd.dropRowIndex = null;
    return;
  }

  dnd.dropPanelSourceId = sourceId;
  dnd.dropPlaylistId = null;

  const rowCount = Number(body.getAttribute('data-row-count')) || 0;
  const bodyTop = body.getBoundingClientRect().top;
  const before = Math.floor((clientY - bodyTop - ROW_HEIGHT / 2) / ROW_HEIGHT) + 1;
  dnd.dropRowIndex = Math.min(Math.max(before, 0), rowCount);
}

export function startColumnDrag(key: string, x: number, y: number) {
  dnd.kind = 'column';
  dnd.columnDragKey = key;
  dnd.columnDropKey = null;
  dnd.pointerX = x;
  dnd.pointerY = y;
}

export function startPlaylistDrag(playlistId: number) {
  dnd.kind = 'playlist';
  dnd.playlistDragId = playlistId;
  dnd.playlistDropId = null;
}

export function updatePointer(x: number, y: number) {
  dnd.pointerX = x;
  dnd.pointerY = y;
}

export function endDrag() {
  dnd.kind = null;
  dnd.trackIds = [];
  dnd.sourcePlaylistId = null;
  dnd.dragOriginSourceId = null;
  dnd.ghostLabel = '';
  dnd.ghostCount = 1;
  dnd.dropRowIndex = null;
  dnd.dropPanelSourceId = null;
  dnd.dropPlaylistId = null;
  dnd.columnDragKey = null;
  dnd.columnDropKey = null;
  dnd.playlistDragId = null;
  dnd.playlistDropId = null;
}
