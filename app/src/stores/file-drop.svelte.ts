import {
  addTracksToPlaylist,
  defaultPlaylistName,
  importFiles,
  importFilesForPlaylist,
  library,
  playlistForSidebarSource,
} from './library.svelte.ts';
import { createPlaylistAndOpen } from './new-playlist.svelte.ts';
import { dnd } from './dnd.svelte.ts';
import { importDropTarget } from './ui.svelte.ts';
import { uniqueName } from '../lib/playlistTree.ts';
import {
  dropPointToCss,
  droppedFolderName,
  fileDropTargetFor,
  type DropPoint,
  type FileDropTarget,
} from '../lib/fileDrop.ts';

export function hoverFileDrop(position: DropPoint): FileDropTarget {
  const point = dropPointToCss(position, window.devicePixelRatio);
  const target = fileDropTargetFor(document.elementFromPoint(point.x, point.y));
  dnd.dropNewPlaylist = target.kind === 'new-playlist';
  dnd.dropPlaylistId = target.kind === 'playlist' ? target.playlistId : null;
  return target;
}

export function clearFileDropHover() {
  dnd.dropNewPlaylist = false;
  dnd.dropPlaylistId = null;
}

function playlistIdFor(target: FileDropTarget): number | null {
  if (target.kind === 'playlist') return target.playlistId;
  if (target.kind === 'view') return playlistForSidebarSource(importDropTarget())?.id ?? null;
  return null;
}

export async function importDroppedPaths(paths: string[], target: FileDropTarget) {
  const playlistId = playlistIdFor(target);
  if (target.kind === 'view' && playlistId == null) {
    await importFiles(paths);
    return;
  }
  const ids = await importFilesForPlaylist(paths);
  if (ids.length === 0) return;
  if (target.kind === 'new-playlist') {
    const folderName = droppedFolderName(library.tracks, paths) ?? defaultPlaylistName();
    const name = uniqueName(library.playlists.map((p) => p.name), folderName);
    await createPlaylistAndOpen(ids, 0, name);
    library.statusMessage = `Created “${name}” with ${ids.length} track${ids.length === 1 ? '' : 's'}`;
    return;
  }
  const playlist = library.playlists.find((p) => p.id === playlistId);
  if (!playlist) return;
  const added = await addTracksToPlaylist(playlist.id, ids);
  library.statusMessage = `Added ${added} track${added === 1 ? '' : 's'} to “${playlist.name}”`;
}
