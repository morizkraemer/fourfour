export type FileDropTarget =
  | { kind: 'view' }
  | { kind: 'playlist'; playlistId: number }
  | { kind: 'new-playlist' }
  | { kind: 'folder'; folderId: number };

export interface HitElement {
  closest(selector: string): { getAttribute(name: string): string | null } | null;
}

export interface DropPoint {
  x: number;
  y: number;
}

/**
 * Tauri types drop positions as physical pixels, but wry only reports physical pixels on
 * Windows (WebView2); WKWebView (macOS) and WebKitGTK report CSS points already.
 */
export function dropPositionScale(userAgent: string, devicePixelRatio: number): number {
  if (!/Windows/.test(userAgent)) return 1;
  return devicePixelRatio > 0 ? devicePixelRatio : 1;
}

export function dropPointToCss(position: DropPoint, scale: number): DropPoint {
  return { x: position.x / scale, y: position.y / scale };
}

export function fileDropTargetFor(hit: HitElement | null): FileDropTarget {
  if (hit?.closest('[data-new-playlist-drop]')) return { kind: 'new-playlist' };
  const playlist = hit?.closest('[data-drop-track-playlist-id]');
  const id = playlist ? Number(playlist.getAttribute('data-drop-track-playlist-id')) : NaN;
  if (Number.isFinite(id)) return { kind: 'playlist', playlistId: id };
  const folder = hit?.closest('[data-playlist-folder]');
  const folderId = folder ? Number(folder.getAttribute('data-playlist-id')) : NaN;
  if (Number.isFinite(folderId)) return { kind: 'folder', folderId };
  return { kind: 'view' };
}

function normalizePath(path: string): string {
  if (path.indexOf('\\') < 0 && !path.endsWith('/')) return path;
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
  const folders = droppedPaths.map((p) => `${normalizePath(p)}/`);
  const ids: number[] = [];
  for (const track of tracks) {
    if (!track.filePath) continue;
    const path = normalizePath(track.filePath);
    if (files.has(path) || folders.some((folder) => path.startsWith(folder))) ids.push(track.id);
  }
  return ids;
}

function isSame(a: string, b: string): boolean {
  return normalizePath(a) === normalizePath(b);
}

export interface DropGroup {
  folder: string | null;
  ids: number[];
}

export function dropGroups(
  tracks: ReadonlyArray<{ id: number; filePath?: string }>,
  droppedPaths: readonly string[],
  matches: ReadonlyArray<{ path: string; id: number }> = []
): DropGroup[] {
  const groups: DropGroup[] = [];
  const loose: number[] = [];
  for (const dropped of droppedPaths) {
    const inside = [
      ...tracks.filter((t) => t.filePath && isInside(dropped, t.filePath)).map((t) => t.id),
      ...matches.filter((m) => isInside(dropped, m.path)).map((m) => m.id),
    ];
    if (inside.length > 0) {
      groups.push({ folder: normalizePath(dropped).split('/').pop() || null, ids: [...new Set(inside)] });
      continue;
    }
    loose.push(
      ...tracks.filter((t) => t.filePath && isSame(dropped, t.filePath)).map((t) => t.id),
      ...matches.filter((m) => isSame(dropped, m.path)).map((m) => m.id)
    );
  }
  if (loose.length > 0) groups.push({ folder: null, ids: [...new Set(loose)] });
  return groups;
}
