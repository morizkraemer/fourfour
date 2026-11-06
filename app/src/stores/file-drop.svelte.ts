import {
  addPlannedPlaylists,
  addTracksToPlaylist,
  analyzeImported,
  deletePlaylists,
  duplicateMatches,
  fillPlaylists,
  importDroppedTracks,
  importFiles,
  library,
  selectVolume,
  playlistForSidebarSource,
} from './library.svelte.ts';
import { createPlaylistAndOpen } from './new-playlist.svelte.ts';
import { dnd } from './dnd.svelte.ts';
import { importDropTarget, ui } from './ui.svelte.ts';
import { descendantIds, uniqueName } from '../lib/playlistTree.ts';
import { dropPlan, folderPlan, pendingNames, trackIdsForFiles } from '../lib/importPlaylists.ts';
import { surveyImport } from '../services/tauri.svelte.ts';
import { playlistImportMessage, type ImportOutcome } from '../lib/importDuplicates.ts';
import { clearPendingRows, completeImportActivity, showPendingRows, startImportActivity, type ImportedFolder } from './import-activity.svelte.ts';
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

async function showPlaylist(name: string) {
  ui.activeSidebarRow = name;
  await selectVolume(null);
}

type PlannedLayout = Awaited<ReturnType<typeof addPlannedPlaylists>>;

/** Lays out playlists for the dropped folders before their tags are read, so the drop shows at once. */
async function createPlannedPlaylists(paths: string[], parentId: number): Promise<PlannedLayout> {
  const layout = await addPlannedPlaylists(dropPlan(await surveyImport(paths), LOOSE_FILES_PLAYLIST_NAME), parentId);
  showPendingRows(Object.fromEntries(layout.leaves.map((leaf) => [leaf.id, pendingNames(leaf.files)])));
  const lastTop = layout.leaves.at(-1)?.top;
  const opened = layout.leaves.find((leaf) => leaf.top === lastTop);
  if (opened) await showPlaylist(opened.name);
  return layout;
}

/** Puts imported tracks into the laid-out playlists; playlists and folders left empty are removed. */
export async function fillPlannedPlaylists(layout: PlannedLayout, matches: { path: string; id: number }[]) {
  const idsFor = trackIdsForFiles(library.tracks, matches);
  const fills = layout.leaves.map((leaf) => ({ id: leaf.id, name: leaf.name, trackIds: idsFor(leaf.files) }));
  await fillPlaylists(fills);
  clearPendingRows(layout.leaves.map((leaf) => leaf.id));
  const empty = new Set(fills.filter((fill) => fill.trackIds.length === 0).map((fill) => fill.id));
  const holdsTracks = (folderId: number) =>
    [...descendantIds(library.playlists, folderId)].some((id) => !empty.has(id) && library.playlists.some((p) => p.id === id && !p.is_folder));
  const emptyFolders = layout.folderIds.filter((id) => !holdsTracks(id));
  if (empty.size > 0 || emptyFolders.length > 0) await deletePlaylists([...empty, ...emptyFolders]);
  return fills.filter((fill) => fill.trackIds.length > 0);
}

async function importIntoNewPlaylists(paths: string[], parentId: number) {
  startImportActivity();
  let layout: PlannedLayout = { leaves: [], folderIds: [] };
  try {
    layout = await createPlannedPlaylists(paths, parentId);
  } catch (err) {
    console.error('surveyImport failed:', err);
  }
  const result = await importDroppedTracks(paths);
  if (!result) {
    await fillPlannedPlaylists(layout, []);
    return;
  }
  const { outcome } = result;
  if (layout.leaves.length === 0) {
    completeImportActivity(outcome, await createPlaylistsFromDrop(paths, outcome, parentId));
  } else {
    const filled = await fillPlannedPlaylists(layout, duplicateMatches(outcome));
    if (filled.length > 0 && !filled.some((fill) => fill.name === ui.activeSidebarRow)) await showPlaylist(filled[filled.length - 1].name);
    const tracks = new Set(filled.flatMap((fill) => fill.trackIds)).size;
    const names = filled.map((fill) => fill.name);
    completeImportActivity(outcome, names.length > 0 ? playlistImportMessage(outcome, { kind: 'created', names, tracks }) : undefined);
  }
  analyzeImported(outcome.added.map((t) => t.id));
}

/** Turns an imported folder into playlists that mirror its subfolders; returns the names of the filled playlists. */
export async function playlistsFromImportedFolder(folder: ImportedFolder): Promise<string[]> {
  const ids = new Set(folder.ids);
  const files = [
    ...library.tracks.filter((t) => ids.has(t.id) && t.filePath).map((t) => t.filePath!),
    ...folder.matches.map((m) => m.path),
  ];
  const plan = folderPlan(folder.path, files);
  if (!plan) return [];
  const filled = await fillPlannedPlaylists(await addPlannedPlaylists([plan]), folder.matches);
  if (filled.length > 0) await showPlaylist(filled[0].name);
  return filled.map((fill) => fill.name);
}

async function createPlaylistsFromDrop(paths: string[], outcome: ImportOutcome, parentId: number): Promise<string> {
  const groups = dropGroups(library.tracks, paths, duplicateMatches(outcome));
  const taken = library.playlists.map((p) => p.name);
  const names: string[] = [];
  for (const group of groups) {
    const name = uniqueName(taken, group.folder ?? LOOSE_FILES_PLAYLIST_NAME);
    taken.push(name);
    names.push(name);
    await createPlaylistAndOpen(group.ids, parentId, name);
  }
  if (names.length > 0) {
    ui.activeSidebarRow = names[names.length - 1];
    await selectVolume(null);
  }
  const tracks = new Set(groups.flatMap((g) => g.ids)).size;
  return playlistImportMessage(outcome, { kind: 'created', names, tracks });
}

async function addDroppedTracksTo(playlistId: number | null, outcome: ImportOutcome, ids: number[]) {
  const playlist = library.playlists.find((p) => p.id === playlistId);
  if (!playlist || ids.length === 0) return undefined;
  const added = await addTracksToPlaylist(playlist.id, ids);
  return playlistImportMessage(outcome, {
    kind: 'added',
    name: playlist.name,
    tracks: ids.length,
    addedToPlaylist: added,
  });
}

export async function importDroppedPaths(paths: string[], target: FileDropTarget) {
  if (target.kind === 'new-playlist' || target.kind === 'folder') {
    await importIntoNewPlaylists(paths, target.kind === 'folder' ? target.folderId : 0);
    return;
  }
  const playlistId = playlistIdFor(target);
  if (playlistId == null) {
    await importFiles(paths);
    return;
  }
  const result = await importDroppedTracks(paths);
  if (!result) return;
  const { outcome, ids } = result;
  completeImportActivity(outcome, await addDroppedTracksTo(playlistId, outcome, ids));
  analyzeImported(outcome.added.map((t) => t.id));
}
