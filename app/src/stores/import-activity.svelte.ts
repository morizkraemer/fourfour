import { listenToImportProgress } from '../services/tauri.svelte.ts';
import { importSummary, type ImportOutcome, type ImportSummary } from '../lib/importDuplicates.ts';
import { pathBaseName } from '../lib/pathName.ts';

export interface ImportedFolder {
  path: string;
  name: string;
  ids: number[];
  matches: { path: string; id: number }[];
}

export const IMPORT_RESULT_SHOWN_MS = 6000;

export const importActivity = $state({
  running: false,
  done: 0,
  total: 0,
  resultVisible: false,
  failed: false,
  summary: null as ImportSummary | null,
  folder: null as ImportedFolder | null,
  pendingRows: {} as Record<number, string[]>,
});

let hideTimer: ReturnType<typeof setTimeout> | undefined;
let listening = false;

export function initImportActivity() {
  if (listening) return;
  listening = true;
  listenToImportProgress(({ done, total }) => {
    if (!importActivity.running) return;
    importActivity.done = done;
    importActivity.total = total;
  });
}

export function startImportActivity() {
  clearTimeout(hideTimer);
  importActivity.running = true;
  importActivity.done = 0;
  importActivity.total = 0;
  importActivity.resultVisible = false;
  importActivity.folder = null;
}

function showResult(summary: ImportSummary, failed: boolean) {
  clearTimeout(hideTimer);
  importActivity.running = false;
  importActivity.failed = failed;
  importActivity.summary = summary;
  importActivity.resultVisible = true;
  hideTimer = setTimeout(() => {
    importActivity.resultVisible = false;
  }, IMPORT_RESULT_SHOWN_MS);
}

export function completeImportActivity(outcome: ImportOutcome, placement?: string) {
  showResult(importSummary(outcome, placement), false);
}

export function failImportActivity(detail: string) {
  showResult({ headline: 'Import failed', chip: 'Import failed', lines: detail ? [detail] : [] }, true);
}

/** The folder the last import read, so the activity menu can turn it into playlists. */
export function rememberImportedFolder(path: string, ids: number[], matches: { path: string; id: number }[]) {
  importActivity.folder = ids.length > 0 ? { path, name: pathBaseName(path) || path, ids, matches } : null;
}

/** Placeholder rows for playlists whose files are still being imported. */
export function showPendingRows(rows: Record<number, string[]>) {
  importActivity.pendingRows = { ...importActivity.pendingRows, ...rows };
}

export function clearPendingRows(ids: number[]) {
  const rows = { ...importActivity.pendingRows };
  for (const id of ids) delete rows[id];
  importActivity.pendingRows = rows;
}
