import { formatBytes } from '../stores/stick-sync.svelte.ts';
import type { BackupPreview, BackupSizes, LastBackup } from '../services/tauri.svelte.ts';

const UI_PREFIXES = ['fourfour.', 'ff.'];
const NOT_SETTINGS = 'fourfour.mock.';

type KeyStore = Pick<Storage, 'length' | 'key' | 'getItem' | 'setItem' | 'removeItem'>;

function isUiSetting(key: string): boolean {
  return UI_PREFIXES.some((p) => key.startsWith(p)) && !key.startsWith(NOT_SETTINGS);
}

function uiKeys(storage: KeyStore): string[] {
  const keys: string[] = [];
  for (let i = 0; i < storage.length; i++) {
    const key = storage.key(i);
    if (key !== null && isUiSetting(key)) keys.push(key);
  }
  return keys;
}

/** The app's own settings kept in this window (shortcuts, columns, panel widths, audio). */
export function collectUiSettings(storage: KeyStore): Record<string, string> {
  return Object.fromEntries(uiKeys(storage).map((key) => [key, storage.getItem(key) ?? '']));
}

/** Makes the window's settings exactly the backed-up ones; other sites' keys stay. */
export function applyUiSettings(storage: KeyStore, settings: Record<string, string>) {
  for (const key of uiKeys(storage)) {
    if (!(key in settings)) storage.removeItem(key);
  }
  for (const [key, value] of Object.entries(settings)) {
    if (isUiSetting(key)) storage.setItem(key, value);
  }
}

export function parseUiSettings(json: string | null): Record<string, string> | null {
  if (!json) return null;
  try {
    const parsed = JSON.parse(json);
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return null;
    return Object.fromEntries(Object.entries(parsed).filter(([, v]) => typeof v === 'string')) as Record<string, string>;
  } catch {
    return null;
  }
}

export function backupFileName(now = new Date()): string {
  const day = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-${String(now.getDate()).padStart(2, '0')}`;
  return `library-backup-${day}.tar.gz`;
}

function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

function daysAgo(atMs: number, now: Date): string {
  const at = new Date(atMs);
  const startOf = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  const days = Math.round((startOf(now) - startOf(at)) / 86_400_000);
  if (days <= 0) return 'today';
  if (days === 1) return 'yesterday';
  return `${days} days ago`;
}

export function lastBackupText(last: LastBackup | null, now = new Date()): string {
  if (!last) return 'No backup yet.';
  return `Last backup ${daysAgo(last.at_ms, now)}: ${fileName(last.path)} (${formatBytes(last.bytes)}).`;
}

export function lastBackupShort(last: LastBackup | null, now = new Date()): string {
  return last ? daysAgo(last.at_ms, now) : 'Never';
}

export function includeText(sizes: BackupSizes | null): string {
  if (!sizes) return 'Library and analysis, optionally with the audio files.';
  const audio = sizes.audio_files === 1 ? '1 audio file' : `${sizes.audio_files} audio files`;
  return `Library and analysis: about ${formatBytes(sizes.library_bytes)}. With audio: about ${formatBytes(sizes.library_bytes + sizes.audio_bytes)} (${audio}).`;
}

export function progressText(done: number, total: number): string {
  const percent = total > 0 ? Math.min(100, Math.floor((done / total) * 100)) : 0;
  return `${formatBytes(done)} of ${formatBytes(total)} · ${percent}%`;
}

export type RestoreLine = { status: 'warn' | 'online'; text: string };

export function restoreSummary(preview: BackupPreview, now = new Date()): string {
  const made = daysAgo(Date.parse(preview.created_at), now);
  const tracks = preview.track_count === 1 ? '1 track' : `${preview.track_count} tracks`;
  return `${fileName(preview.path)} · ${tracks} · made ${made}`;
}

export function restoreLines(preview: BackupPreview): RestoreLine[] {
  const current = preview.current_tracks === 1 ? '1 track' : `${preview.current_tracks} tracks`;
  const lines: RestoreLine[] = [
    { status: 'warn', text: `Replaces your current ${current}, playlists and settings; they are saved as a backup first` },
  ];
  if (preview.includes_audio) {
    const files = preview.audio_files === 1 ? '1 audio file' : `${preview.audio_files} audio files`;
    lines.push({ status: 'online', text: `${files} inside: you choose a folder for them next` });
  } else {
    lines.push({ status: 'online', text: 'No audio inside: tracks point to the same files as before' });
  }
  lines.push({ status: 'online', text: 'Audio files on this computer are never deleted' });
  return lines;
}
