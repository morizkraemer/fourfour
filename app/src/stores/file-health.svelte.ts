import {
  checkLibraryFiles,
  getLibraryHealth,
  listenToLibraryHealth,
  locateTrackFile,
  pickDirectory,
  pickFile,
  relocateMissingInFolder,
  type LibraryHealth,
  type Relinked,
  type RelocateReport,
} from '../services/tauri.svelte.ts';
import { RELOCATED_MARK_MS, type FileState } from '../lib/fileHealth.ts';
import { fileStatus } from './file-status.svelte.ts';
import { library, bumpLibraryTracks } from './library.svelte.ts';
import { ui } from './ui.svelte.ts';

const AUDIO_FILTER = { name: 'Audio', extensions: ['mp3', 'wav', 'aif', 'aiff', 'flac', 'm4a', 'aac', 'ogg', 'alac'] };

export const relocateResult = $state({ report: null as RelocateReport | null, error: '' });

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
    library.statusMessage = `Library check failed: ${String(err)}`;
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
    relocateResult.error = String(err);
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
    relocateResult.error = String(err);
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
