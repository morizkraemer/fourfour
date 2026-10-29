import { listenToImportProgress } from '../services/tauri.svelte.ts';
import { importSummary, type ImportOutcome, type ImportSummary } from '../lib/importDuplicates.ts';

export const IMPORT_RESULT_SHOWN_MS = 6000;

export const importActivity = $state({
  running: false,
  done: 0,
  total: 0,
  resultVisible: false,
  failed: false,
  summary: null as ImportSummary | null,
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
  showResult({ headline: 'Import failed', lines: detail ? [detail] : [] }, true);
}
