import {
  addTracksToPlaylist,
  importDroppedTracks,
  importFiles,
  library,
  playlistForSidebarSource,
} from './library.svelte.ts';
import { createPlaylistAndOpen } from './new-playlist.svelte.ts';
import { dnd } from './dnd.svelte.ts';
import { importDropTarget } from './ui.svelte.ts';
import { uniqueName } from '../lib/playlistTree.ts';
import { libraryImportMessage, playlistImportMessage, type ImportOutcome } from '../lib/importDuplicates.ts';
import {
  dropGroups,
  dropPointToCss,
  dropPositionScale,
  fileDropTargetFor,
  type DropPoint,
  type FileDropTarget,
} from '../lib/fileDrop.ts';

export const LOOSE_FILES_PLAYLIST_NAME = 'New playlist';

export const fileDrop = $state({
  /** Files hover the content area: they will go into the open view. */
  overView: false,
});

export function hoverFileDrop(position: DropPoint): FileDropTarget {
  const point = dropPointToCss(position, dropPositionScale(navigator.userAgent, window.devicePixelRatio));
  const target = fileDropTargetFor(document.elementFromPoint(point.x, point.y));
  dnd.dropNewPlaylist = target.kind === 'new-playlist';
  dnd.dropPlaylistId = target.kind === 'playlist' ? target.playlistId : null;
  dnd.dropFolderId = target.kind === 'folder' ? target.folderId : null;
  fileDrop.overView = target.kind === 'view';
  return target;
}

export function clearFileDropHover() {
  dnd.dropNewPlaylist = false;
  dnd.dropPlaylistId = null;
  dnd.dropFolderId = null;
  fileDrop.overView = false;
}

function playlistIdFor(target: FileDropTarget): number | null {
  if (target.kind === 'playlist') return target.playlistId;
  if (target.kind === 'view') return playlistForSidebarSource(importDropTarget())?.id ?? null;
  return null;
}

function duplicateMatches(outcome: ImportOutcome) {
  const skipped = new Set(outcome.skipped_ids);
  return outcome.plan.duplicates
    .filter((d) => skipped.has(d.existing_id))
    .map((d) => ({ path: d.new_path, id: d.existing_id }));
}

async function createPlaylistsFromDrop(paths: string[], outcome: ImportOutcome, parentId: number) {
  const groups = dropGroups(library.tracks, paths, duplicateMatches(outcome));
  const taken = library.playlists.map((p) => p.name);
  const names: string[] = [];
  for (const group of groups) {
    const name = uniqueName(taken, group.folder ?? LOOSE_FILES_PLAYLIST_NAME);
    taken.push(name);
    names.push(name);
    await createPlaylistAndOpen(group.ids, parentId, name);
  }
  const tracks = new Set(groups.flatMap((g) => g.ids)).size;
  library.statusMessage = playlistImportMessage(outcome, { kind: 'created', names, tracks });
}

export async function importDroppedPaths(paths: string[], target: FileDropTarget) {
  const playlistId = playlistIdFor(target);
  if (target.kind === 'view' && playlistId == null) {
    await importFiles(paths);
    return;
  }
  const result = await importDroppedTracks(paths);
  if (!result) return;
  const { outcome, ids } = result;
  if (ids.length === 0) {
    library.statusMessage = libraryImportMessage(outcome);
    return;
  }
  if (target.kind === 'new-playlist' || target.kind === 'folder') {
    await createPlaylistsFromDrop(paths, outcome, target.kind === 'folder' ? target.folderId : 0);
    return;
  }
  const playlist = library.playlists.find((p) => p.id === playlistId);
  if (!playlist) return;
  const added = await addTracksToPlaylist(playlist.id, ids);
  library.statusMessage = playlistImportMessage(outcome, {
    kind: 'added',
    name: playlist.name,
    tracks: ids.length,
    addedToPlaylist: added,
  });
}
