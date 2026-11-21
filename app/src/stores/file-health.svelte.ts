import {
  applyRelocateFolder,
  applyReconnects,
  checkLibraryFiles,
  findReconnectCandidates,
  getLibraryHealth,
  lastFolderRelocation,
  listenToLibraryHealth,
  locateTrackFile,
  pickDirectory,
  pickFile,
  previewRelocateFolder,
  relocateMissingInFolder,
  undoRelocateFolder,
  type FolderRelocation,
  type LibraryHealth,
  type ReconnectPlan,
  type Relinked,
  type RelocateReport,
  type RemapPreview,
} from '../services/tauri.svelte.ts';
import { RELOCATED_MARK_MS, type FileState } from '../lib/fileHealth.ts';
import { commonFolder } from '../lib/reconnect.ts';
import { failureText, reconnectFailureText, relocateFailureText } from '../lib/userErrors.ts';
import { fileStatus } from './file-status.svelte.ts';
import { library, bumpLibraryTracks } from './library.svelte.ts';
import { ui } from './ui.svelte.ts';

const AUDIO_FILTER = { name: 'Audio', extensions: ['mp3', 'wav', 'aif', 'aiff', 'flac', 'm4a', 'aac', 'ogg', 'alac'] };

export const relocateResult = $state({ report: null as RelocateReport | null, error: '' });

export const reconnectState = $state({ busy: false, plan: null as ReconnectPlan | null, error: '' });

export const folderRelocation = $state({
  open: false,
  oldRoot: '',
  newRoot: '',
  preview: null as RemapPreview | null,
  last: null as FolderRelocation | null,
  busy: false,
  error: '',
});

const relocatedTimers = new Map<number, ReturnType<typeof setTimeout>>();

function scheduleRelocatedFade(id: number) {
  clearTimeout(relocatedTimers.get(id));
  relocatedTimers.set(
    id,
    setTimeout(() => {
      relocatedTimers.delete(id);
      if (fileStatus.byId[id] === 'relocated') delete fileStatus.byId[id];
    }, RELOCATED_MARK_MS),
  );
}

function isOlderThanShown(health: LibraryHealth): boolean {
  if (fileStatus.checkedAt === null) return false;
  return health.checked_at_ms === null || health.checked_at_ms < fileStatus.checkedAt;
}

export function applyHealth(health: LibraryHealth, freshlyRelinked: number[] = []) {
  if (freshlyRelinked.length === 0 && isOlderThanShown(health)) return;
  const byId: Record<number, FileState> = {};
  for (const id of health.missing) byId[id] = 'missing';
  for (const id of health.error) byId[id] = 'error';
  for (const id of freshlyRelinked) {
    if (!byId[id]) {
      byId[id] = 'relocated';
      scheduleRelocatedFade(id);
    }
  }
  for (const id of relocatedTimers.keys()) if (!byId[id]) byId[id] = 'relocated';
  fileStatus.byId = byId;
  fileStatus.total = health.total;
  fileStatus.missing = health.missing.length;
  fileStatus.error = health.error.length;
  fileStatus.relocated = health.relocated.length;
  fileStatus.checkedAt = health.checked_at_ms;
  fileStatus.durationMs = health.duration_ms;
  fileStatus.running = health.running;
  if (fileStatus.missing + fileStatus.error === 0) fileStatus.onlyProblems = false;
}

let listening = false;

export async function initFileHealth() {
  if (!listening) {
    listening = true;
    listenToLibraryHealth((health) => applyHealth(health));
  }
  try {
    applyHealth(await getLibraryHealth());
  } catch (err) {
    console.error('getLibraryHealth failed:', err);
  }
}

export async function checkLibraryAgain() {
  fileStatus.running = true;
  try {
    applyHealth(await checkLibraryFiles());
  } catch (err) {
    fileStatus.running = false;
    console.info('checkLibraryFiles failed:', err);
    library.statusMessage = failureText("Couldn't check your files.", err);
  }
}

function patchTrackPaths(relinked: Relinked[]) {
  if (relinked.length === 0) return;
  const byId = new Map(relinked.map((r) => [r.track_id, r.source_path]));
  for (const track of library.tracks) {
    const path = byId.get(track.id);
    if (path === undefined) continue;
    track.filePath = path;
    if (track.raw) track.raw.source_path = path;
  }
  bumpLibraryTracks(byId.keys());
}

export async function relocateMissing() {
  const folder = await pickDirectory();
  if (!folder) return;
  fileStatus.relocating = true;
  relocateResult.error = '';
  try {
    const { report, health } = await relocateMissingInFolder(folder);
    patchTrackPaths(report.matched);
    applyHealth(health, report.matched.map((m) => m.track_id));
    relocateResult.report = report;
  } catch (err) {
    console.info('relocate failed:', err);
    relocateResult.error = relocateFailureText(err);
  } finally {
    fileStatus.relocating = false;
    fileStatus.dialogOpen = true;
  }
}

export async function locateTrack(trackId: number) {
  const path = await pickFile(AUDIO_FILTER);
  if (!path) return;
  relocateResult.error = '';
  try {
    const { relinked, health } = await locateTrackFile(trackId, path);
    patchTrackPaths([relinked]);
    applyHealth(health, [relinked.track_id]);
    if (relocateResult.report) {
      const { matched, ambiguous, not_found } = relocateResult.report;
      relocateResult.report = {
        matched: [...matched, relinked],
        ambiguous: ambiguous.filter((id) => id !== trackId),
        not_found: not_found.filter((id) => id !== trackId),
      };
    }
  } catch (err) {
    console.info('relocate failed:', err);
    relocateResult.error = relocateFailureText(err);
  }
}

export async function findMovedFiles() {
  const folder = await pickDirectory();
  if (!folder) return;
  reconnectState.busy = true;
  reconnectState.error = '';
  try {
    const plan = await findReconnectCandidates([folder]);
    let reconnected: Relinked[] = [];
    if (plan.reconnect.length > 0) {
      const { relinked, health } = await applyReconnects(plan.reconnect.map((r) => ({ track_id: r.track_id, path: r.source_path })));
      patchTrackPaths(relinked);
      applyHealth(health, relinked.map((r) => r.track_id));
      reconnected = relinked;
    }
    reconnectState.plan = { ...plan, reconnect: reconnected };
  } catch (err) {
    console.info('reconnect failed:', err);
    reconnectState.error = reconnectFailureText(err);
  } finally {
    reconnectState.busy = false;
    fileStatus.dialogOpen = true;
  }
}

export async function chooseReconnect(trackId: number, path: string) {
  reconnectState.error = '';
  try {
    const { relinked, health } = await applyReconnects([{ track_id: trackId, path }]);
    patchTrackPaths(relinked);
    applyHealth(health, [trackId]);
    const plan = reconnectState.plan;
    if (plan) {
      reconnectState.plan = {
        reconnect: [...plan.reconnect, ...relinked],
        needs_choice: plan.needs_choice.filter((c) => c.track_id !== trackId),
        still_missing: plan.still_missing,
      };
    }
  } catch (err) {
    console.info('reconnect failed:', err);
    reconnectState.error = reconnectFailureText(err);
  }
}

function missingTrackPaths(): string[] {
  return library.tracks.filter((t) => fileStatus.byId[t.id] === 'missing').map((t) => t.filePath);
}

async function refreshLastRelocation() {
  try {
    folderRelocation.last = await lastFolderRelocation();
  } catch (err) {
    console.info('lastFolderRelocation failed:', err);
  }
}

export async function openFolderRelocation() {
  folderRelocation.open = true;
  folderRelocation.error = '';
  folderRelocation.preview = null;
  folderRelocation.newRoot = '';
  folderRelocation.oldRoot = commonFolder(missingTrackPaths());
  await refreshLastRelocation();
}

export function closeFolderRelocation() {
  folderRelocation.open = false;
}

export async function previewFolderRelocation() {
  folderRelocation.preview = null;
  folderRelocation.error = '';
  const { oldRoot, newRoot } = folderRelocation;
  if (!oldRoot.trim() || !newRoot) return;
  folderRelocation.busy = true;
  try {
    const preview = await previewRelocateFolder(oldRoot.trim(), newRoot);
    if (oldRoot === folderRelocation.oldRoot && newRoot === folderRelocation.newRoot) folderRelocation.preview = preview;
  } catch (err) {
    console.info('relocate preview failed:', err);
    folderRelocation.error = reconnectFailureText(err);
  } finally {
    folderRelocation.busy = false;
  }
}

export async function chooseNewRoot() {
  const folder = await pickDirectory();
  if (!folder) return;
  folderRelocation.newRoot = folder;
  await previewFolderRelocation();
}

export async function applyFolderRelocation() {
  const { oldRoot, newRoot, preview } = folderRelocation;
  if (!preview || preview.found === 0) return;
  folderRelocation.busy = true;
  folderRelocation.error = '';
  try {
    const { applied, health } = await applyRelocateFolder(oldRoot.trim(), newRoot);
    patchTrackPaths(applied.remapped);
    applyHealth(health, applied.remapped.map((r) => r.track_id));
    folderRelocation.preview = null;
    folderRelocation.newRoot = '';
    await refreshLastRelocation();
  } catch (err) {
    console.info('relocate folder failed:', err);
    folderRelocation.error = reconnectFailureText(err);
  } finally {
    folderRelocation.busy = false;
  }
}

export async function undoFolderRelocation() {
  const last = folderRelocation.last;
  if (!last) return;
  folderRelocation.busy = true;
  folderRelocation.error = '';
  try {
    const { undone, health } = await undoRelocateFolder(last.id);
    patchTrackPaths(undone.reverted);
    applyHealth(health);
    folderRelocation.oldRoot = last.old_root;
    await refreshLastRelocation();
  } catch (err) {
    console.info('undo relocate failed:', err);
    folderRelocation.error = reconnectFailureText(err);
  } finally {
    folderRelocation.busy = false;
  }
}

export function showProblemTracks(on = true) {
  fileStatus.onlyProblems = on;
  if (on) ui.activeSidebarRow = 'All Tracks';
}

export function openHealthDialog() {
  fileStatus.dialogOpen = true;
}

export function closeHealthDialog() {
  fileStatus.dialogOpen = false;
}

export function problemTrackIds(state?: FileState): number[] {
  return Object.entries(fileStatus.byId)
    .filter(([, s]) => (state ? s === state : s !== 'relocated'))
    .map(([id]) => Number(id));
}
