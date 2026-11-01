import {
  exportBackup,
  getBackupStatus,
  listenToBackupProgress,
  pickBackupDestination,
  pickBackupFile,
  pickDirectory,
  readBackup,
  restoreBackup,
  type BackupPreview,
  type BackupStatus,
} from '../services/tauri.svelte.ts';
import { applyUiSettings, backupFileName, collectUiSettings, parseUiSettings } from '../lib/backup.ts';
import { backupReadFailureText, backupRestoreFailureText, backupSaveFailureText } from '../lib/userErrors.ts';

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
    console.info('export_backup failed:', err);
    backup.exportError = backupSaveFailureText(err);
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
    console.info('read_backup failed:', err);
    backup.restoreError = backupReadFailureText(err);
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
    const outcome = await withProgress('restore', () => restoreBackup(preview.path, audioFolder));
    const ui = parseUiSettings(outcome.ui_settings);
    if (ui) applyUiSettings(localStorage, ui);
    reload();
  } catch (err) {
    console.info('restore_backup failed:', err);
    backup.restoreError = backupRestoreFailureText(err);
    backup.restoring = false;
  }
}
