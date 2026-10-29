import { describe, expect, it } from 'vitest';
import {
  applyUiSettings,
  backupFileName,
  collectUiSettings,
  includeText,
  lastBackupText,
  parseUiSettings,
  restoreLines,
  restoreSummary,
} from './backup.ts';

function memoryStorage(initial: Record<string, string>) {
  const map = new Map(Object.entries(initial));
  return {
    get length() {
      return map.size;
    },
    key: (i: number) => [...map.keys()][i] ?? null,
    getItem: (k: string) => map.get(k) ?? null,
    setItem: (k: string, v: string) => void map.set(k, v),
    removeItem: (k: string) => void map.delete(k),
    dump: () => Object.fromEntries(map),
  };
}

describe('ui settings', () => {
  it('collects only the app settings', () => {
    const storage = memoryStorage({ 'fourfour.quantize.v1': '1', 'ff.sidebar.collapsedFolders': '[]', 'fourfour.mock.tags': '{}', other: 'x' });
    expect(collectUiSettings(storage)).toEqual({ 'fourfour.quantize.v1': '1', 'ff.sidebar.collapsedFolders': '[]' });
  });

  it('restores exactly the backed-up settings and leaves foreign keys', () => {
    const storage = memoryStorage({ 'fourfour.quantize.v1': '0', 'fourfour.tableColumns.v3': 'x', other: 'keep' });
    applyUiSettings(storage, { 'fourfour.quantize.v1': '1', evil: 'no' });
    expect(storage.dump()).toEqual({ 'fourfour.quantize.v1': '1', other: 'keep' });
  });

  it('parses only string maps', () => {
    expect(parseUiSettings('{"a":"1","b":2}')).toEqual({ a: '1' });
    expect(parseUiSettings('[1]')).toBeNull();
    expect(parseUiSettings('nope')).toBeNull();
    expect(parseUiSettings(null)).toBeNull();
  });
});

describe('backup copy', () => {
  const now = new Date(2026, 9, 29, 12);

  it('names the file by day', () => {
    expect(backupFileName(now)).toBe('library-backup-2026-10-29.tar.gz');
  });

  it('describes the last backup and the sizes', () => {
    expect(lastBackupText(null, now)).toBe('No backup yet.');
    const last = { path: '/b/library-backup-2026-10-17.tar.gz', at_ms: new Date(2026, 9, 17, 20).getTime(), bytes: 38_000_000, includes_audio: false };
    expect(lastBackupText(last, now)).toBe('Last backup 12 days ago: library-backup-2026-10-17.tar.gz (38.0 MB).');
    expect(includeText({ library_bytes: 38_000_000, audio_bytes: 61_000_000_000, audio_files: 188 })).toBe(
      'Library and analysis: about 38.0 MB. With audio: about 61.0 GB (188 audio files).',
    );
  });

  it('says what a restore replaces', () => {
    const preview = {
      path: '/b/library-backup-2026-10-17.tar.gz',
      created_at: new Date(2026, 9, 17, 20).toISOString(),
      track_count: 176,
      playlist_count: 4,
      includes_audio: false,
      audio_files: 0,
      current_tracks: 188,
    };
    expect(restoreSummary(preview, now)).toBe('library-backup-2026-10-17.tar.gz · 176 tracks · made 12 days ago');
    expect(restoreLines(preview).map((l) => l.text)).toEqual([
      'Replaces your current 188 tracks, playlists and settings; they are saved as a backup first',
      'No audio inside: tracks point to the same files as before',
      'Audio files on this computer are never deleted',
    ]);
    expect(restoreLines({ ...preview, includes_audio: true, audio_files: 1 })[1].text).toBe('1 audio file inside: you choose a folder for them next');
  });
});
