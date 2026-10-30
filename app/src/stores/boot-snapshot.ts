import { rowBandsFor, rowBandsFromBandBytes } from '../services/waveform-bands.ts';

export const BOOT_SNAPSHOT_KEY = 'fourfour.bootSnapshot.v1';
export const BOOT_SNAPSHOT_MAX_ROWS = 120;

const VIEW_GETTERS = new Set(['cover', 'rowCover', 'status', 'bands', 'raw']);

type Cell = string | number | boolean | null;

export interface BootWave {
  kind: 'bands' | 'mono';
  data: string;
}

export interface BootRow {
  cells: Record<string, Cell>;
  wave?: BootWave;
}

export interface BootSnapshot {
  libraryPath: string;
  view: string;
  total: number;
  rows: BootRow[];
}

function bytesToBase64(bytes: ArrayLike<number>): string {
  let text = '';
  for (let i = 0; i < bytes.length; i++) text += String.fromCharCode(bytes[i]);
  return btoa(text);
}

function base64ToBytes(data: string): Uint8Array {
  const text = atob(data);
  const bytes = new Uint8Array(text.length);
  for (let i = 0; i < text.length; i++) bytes[i] = text.charCodeAt(i);
  return bytes;
}

function waveOf(track: Record<string, unknown>): BootWave | undefined {
  const bandsPreview = track.bandsPreview as ArrayLike<number> | undefined;
  if (bandsPreview?.length) return { kind: 'bands', data: bytesToBase64(bandsPreview) };
  const wire = track.bandsWire as string | number[] | undefined;
  if (typeof wire === 'string' && wire) return { kind: 'bands', data: wire };
  if (Array.isArray(wire) && wire.length) return { kind: 'bands', data: bytesToBase64(wire) };
  const preview = track.preview as ArrayLike<number> | undefined;
  if (preview?.length) return { kind: 'mono', data: bytesToBase64(preview) };
  return undefined;
}

export function bootRowOf(track: Record<string, unknown>): BootRow {
  const cells: Record<string, Cell> = {};
  for (const key of Object.keys(track)) {
    if (VIEW_GETTERS.has(key)) continue;
    const value = track[key];
    if (value === null || typeof value === 'string' || typeof value === 'number' || typeof value === 'boolean') cells[key] = value;
  }
  const wave = waveOf(track);
  return wave ? { cells, wave } : { cells };
}

export function bootSnapshotOf(libraryPath: string, view: string, tracks: Record<string, unknown>[], rows: number): BootSnapshot {
  const count = Math.max(0, Math.min(rows, BOOT_SNAPSHOT_MAX_ROWS, tracks.length));
  return { libraryPath, view, total: tracks.length, rows: tracks.slice(0, count).map(bootRowOf) };
}

export function bootRowTrack(row: BootRow): Record<string, unknown> {
  const bytes = row.wave ? base64ToBytes(row.wave.data) : null;
  const bands = !bytes ? null : row.wave!.kind === 'bands' ? rowBandsFromBandBytes(bytes) : rowBandsFor(bytes);
  return { ...row.cells, bands };
}

function isBootSnapshot(value: unknown): value is BootSnapshot {
  const s = value as BootSnapshot;
  return !!s && typeof s.libraryPath === 'string' && typeof s.view === 'string' && typeof s.total === 'number' && Array.isArray(s.rows);
}

export function readBootSnapshot(libraryPath: string): BootSnapshot | null {
  try {
    const raw = localStorage.getItem(BOOT_SNAPSHOT_KEY);
    if (!raw) return null;
    const snapshot = JSON.parse(raw);
    if (!isBootSnapshot(snapshot) || snapshot.libraryPath !== libraryPath || snapshot.rows.length === 0) return null;
    return snapshot;
  } catch {
    return null;
  }
}

export function writeBootSnapshot(snapshot: BootSnapshot) {
  try {
    if (snapshot.rows.length === 0) localStorage.removeItem(BOOT_SNAPSHOT_KEY);
    else localStorage.setItem(BOOT_SNAPSHOT_KEY, JSON.stringify(snapshot));
  } catch (err) {
    console.warn('boot snapshot not saved:', err);
  }
}

export function clearBootSnapshot() {
  try {
    localStorage.removeItem(BOOT_SNAPSHOT_KEY);
  } catch {
    return;
  }
}
