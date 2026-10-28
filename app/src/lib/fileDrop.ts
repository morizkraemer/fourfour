export type FileDropTarget =
  | { kind: 'view' }
  | { kind: 'playlist'; playlistId: number }
  | { kind: 'new-playlist' };

export interface HitElement {
  closest(selector: string): { getAttribute(name: string): string | null } | null;
}

export interface DropPoint {
  x: number;
  y: number;
}

/** Tauri reports drop positions in physical pixels; the DOM hit-tests in CSS pixels. */
export function dropPointToCss(position: DropPoint, devicePixelRatio: number): DropPoint {
  const scale = devicePixelRatio > 0 ? devicePixelRatio : 1;
  return { x: position.x / scale, y: position.y / scale };
}

export function fileDropTargetFor(hit: HitElement | null): FileDropTarget {
  if (hit?.closest('[data-new-playlist-drop]')) return { kind: 'new-playlist' };
  const playlist = hit?.closest('[data-drop-track-playlist-id]');
  const id = playlist ? Number(playlist.getAttribute('data-drop-track-playlist-id')) : NaN;
  if (Number.isFinite(id)) return { kind: 'playlist', playlistId: id };
  return { kind: 'view' };
}

function normalizePath(path: string): string {
  return path.replace(/\\/g, '/').replace(/\/+$/, '');
}

function isInside(folder: string, file: string): boolean {
  return normalizePath(file).startsWith(`${normalizePath(folder)}/`);
}

/** Library tracks that live in, or are, one of the dropped files and folders. */
export function trackIdsUnderPaths(
  tracks: ReadonlyArray<{ id: number; filePath?: string }>,
  droppedPaths: readonly string[]
): number[] {
  const files = new Set(droppedPaths.map(normalizePath));
  return tracks
    .filter((t) => {
      if (!t.filePath) return false;
      return files.has(normalizePath(t.filePath)) || droppedPaths.some((p) => isInside(p, t.filePath!));
    })
    .map((t) => t.id);
}

/** Name of the first dropped folder that holds library tracks, or null when only files were dropped. */
export function droppedFolderName(
  tracks: ReadonlyArray<{ filePath?: string }>,
  droppedPaths: readonly string[]
): string | null {
  const folder = droppedPaths.find((p) => tracks.some((t) => t.filePath && isInside(p, t.filePath)));
  if (!folder) return null;
  return normalizePath(folder).split('/').pop() || null;
}
