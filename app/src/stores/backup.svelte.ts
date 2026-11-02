import {
  exportBackup,
  getBackupStatus,
  listenToBackupProgress,
  pickBackupDestination,
  pickBackupFile,
  pickDirectory,
  readBackup,
  recoverRestoreBackup,
  restoreBackup,
  type BackupPreview,
  type BackupStatus,
} from '../services/tauri.svelte.ts';
import { library } from './library.svelte.ts';
import { applyUiSettings, backupFileName, collectUiSettings, parseUiSettings } from '../lib/backup.ts';

export const backup = $state({
  status: null as BackupStatus | null,
  includeAudio: false,
  exporting: false,
  progress: null as { done: number; total: number } | null,
  exportError: '',
  preview: null as BackupPreview | null,
  restoring: false,
  restoreError: '',
});

function message(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

async function withProgress<T>(phase: 'export' | 'restore', run: () => Promise<T>): Promise<T> {
  backup.progress = null;
  const stop = listenToBackupProgress((p) => {
    if (p.phase === phase) backup.progress = { done: p.done, total: p.total };
  });
  try {
    return await run();
  } finally {
    stop();
    backup.progress = null;
  }
}

export async function loadBackupStatus() {
  try {
    backup.status = await getBackupStatus();
  } catch (err) {
    console.error('get_backup_status failed:', err);
  }
}

export async function saveBackup() {
  const path = await pickBackupDestination(backupFileName());
  if (!path) return;
  backup.exporting = true;
  backup.exportError = '';
  try {
    const ui = JSON.stringify(collectUiSettings(localStorage));
    const last = await withProgress('export', () => exportBackup(path, backup.includeAudio, ui));
    backup.status = backup.status ? { ...backup.status, last } : { sizes: { library_bytes: 0, audio_bytes: 0, audio_files: 0 }, last };
  } catch (err) {
    backup.exportError = `Backup failed: ${message(err)}`;
  } finally {
    backup.exporting = false;
  }
}

export async function chooseRestoreFile() {
  const path = await pickBackupFile();
  if (!path) return;
  backup.restoreError = '';
  try {
    backup.preview = await readBackup(path);
  } catch (err) {
    backup.restoreError = message(err);
  }
}

export function cancelRestore() {
  if (backup.restoring) return;
  backup.preview = null;
}

export async function confirmRestore(reload: () => void = () => location.reload()) {
  const preview = backup.preview;
  if (!preview || backup.restoring) return;
  const audioFolder = preview.includes_audio ? await pickDirectory() : null;
  if (preview.includes_audio && !audioFolder) return;
  backup.restoring = true;
  backup.restoreError = '';
  try {
    const restore: (path: string, audioFolder: string | null) => Promise<{ ui_settings: string | null }> = library.openFailure ? recoverRestoreBackup : restoreBackup;
    const outcome = await withProgress('restore', () => restore(preview.path, audioFolder));
    const ui = parseUiSettings(outcome.ui_settings);
    if (ui) applyUiSettings(localStorage, ui);
    reload();
  } catch (err) {
    backup.restoreError = message(err);
    backup.restoring = false;
  }
}
