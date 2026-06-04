/**
 * dnd.svelte.ts — Shared drag-and-drop session state (tracks, columns, playlists).
 */

export type DragKind = 'tracks' | 'column' | 'playlist' | null;

export const dnd = $state({
  kind: null as DragKind,
  /** Track IDs being dragged */
  trackIds: [] as number[],
  /** Playlist the drag started from (null = library / all tracks) */
  sourcePlaylistId: null as number | null,
  ghostLabel: '',
  ghostCount: 1,
  pointerX: 0,
  pointerY: 0,
  /** Row insert index in the current table body */
  dropRowIndex: null as number | null,
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
  x: number,
  y: number
) {
  dnd.kind = 'tracks';
  dnd.trackIds = trackIds;
  dnd.sourcePlaylistId = sourcePlaylistId;
  dnd.ghostLabel = label;
  dnd.ghostCount = trackIds.length;
  dnd.pointerX = x;
  dnd.pointerY = y;
  dnd.dropRowIndex = null;
  dnd.dropPlaylistId = null;
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
  dnd.ghostLabel = '';
  dnd.ghostCount = 1;
  dnd.dropRowIndex = null;
  dnd.dropPlaylistId = null;
  dnd.columnDragKey = null;
  dnd.columnDropKey = null;
  dnd.playlistDragId = null;
  dnd.playlistDropId = null;
}
