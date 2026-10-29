import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open as dialogOpen, save as dialogSave } from '@tauri-apps/plugin-dialog';
import { unlistenWhenReady } from '../lib/lateUnlisten.ts';
import {
  isValidAnalysisPayload,
  sanitizeAnalysisPayload,
} from './analysis-data.ts';
import type { CuePointPayload } from '../lib/cues.ts';
import { gridForStorage, type BeatMarker, type EditedGrid } from '../lib/beatGrid.ts';
import { mockAnalysisData } from '../mocks/analysis.ts';
import { perfAnalysisData } from '../mocks/analysis-perf.ts';
import { MOCK_PROFILE_ORDER, rowBandsBase64, syntheticBands } from '../mocks/synthetic-bands.ts';

export let lastTauriError = 'None';

export function getIpcDiagnostic(): string {
  if (typeof window === 'undefined') return 'window undefined';
  const hasIpc = (window as any).__TAURI_IPC__ !== undefined;
  const hasInternals = (window as any).__TAURI_INTERNALS__ !== undefined;
  const hasTauriGlobal = (window as any).__TAURI__ !== undefined;
  return `IPC: ${hasIpc}, Internals: ${hasInternals}, Global: ${hasTauriGlobal}`;
}

// Detect if running inside Tauri (evaluated dynamically to avoid race conditions at module load time)
export function getIsTauri(): boolean {
  if (typeof window === 'undefined') return false;
  return (window as any).__TAURI_IPC__ !== undefined || (window as any).__TAURI_INTERNALS__ !== undefined;
}

/** Browser-only Vite preview — use mock fallbacks instead of failing invoke. */
function useBrowserMocks(): boolean {
  return !getIsTauri();
}

export interface TrackInfo {
  id: number;
  source_path: string;
  title: string;
  artist: string;
  album: string;
  genre: string;
  tempo: number; // BPM * 100
  key: string;
  duration_secs: number;
  bitrate: number;
  file_size: number;
  has_artwork: boolean;
  has_cues: boolean;
  /** Authoritative analysis flag from the DB index (not `tempo > 0`). */
  has_analysis?: boolean;
  /** Waveform (and maybe key) are stored; key or BPM/grid still to come. */
  analysis_pending?: boolean;
  /** 400-byte mono preview embedded at load (base64 from Tauri, byte array in mocks). */
  waveform_preview?: string | number[] | null;
  /** 3-band row preview (128 columns of low, mid, high bytes): replaces `waveform_preview` when the track has band data. */
  waveform_bands?: string | number[] | null;
  /** Rekordbox star rating 1-5, absent when unrated. */
  rating?: number;
  /** Rekordbox colour label: a colour name or `#rrggbb`. */
  color?: string;
}

export interface PlaylistInput {
  id: number;
  name: string;
  track_ids: number[];
  /** Containing folder; 0 is the top level. */
  parent_id: number;
  is_folder: boolean;
}

export interface LoadedState {
  tracks: TrackInfo[];
  playlists: PlaylistInput[];
}

export interface SyncReport {
  tracks_added: number;
  tracks_updated: number;
  tracks_replaced: number;
  tracks_removed: number;
  tracks_unchanged: number;
}

export interface UsbTrackInfo {
  id: number;
  title: string;
  artist: string;
  album: string;
  genre: string;
  key: string;
  bpm: number;
  duration: number;
  usb_path: string;
}

export interface UsbPlaylistInfo {
  id: number;
  name: string;
  track_count: number;
}

export interface UsbStateResponse {
  tracks: UsbTrackInfo[];
  playlists: UsbPlaylistInfo[];
}

// Fallback Mock Data for Browser Mode
let mockTracks: TrackInfo[] = [
  { id: 1, source_path: '/Music/Solomun - Hyperion.mp3', title: 'Hyperion', artist: 'Solomun', album: 'Diynamic Hits', genre: 'Melodic House', tempo: 12400, key: '9A', duration_secs: 462, bitrate: 320, file_size: 18482010, has_artwork: true, has_cues: false, has_analysis: true, analysis_pending: false },
  { id: 2, source_path: '/Music/Tale Of Us - Voidwalker.flac', title: 'Voidwalker', artist: 'Tale Of Us', album: 'Endless Run', genre: 'Melodic Techno', tempo: 12200, key: '7A', duration_secs: 494, bitrate: 1040, file_size: 64281920, has_artwork: true, has_cues: true, has_analysis: true, analysis_pending: false },
  { id: 3, source_path: '/Music/Bicep - Drift.mp3', title: 'Drift', artist: 'Bicep', album: 'Drift EP', genre: 'Breakbeat', tempo: 12800, key: '5B', duration_secs: 415, bitrate: 320, file_size: 16601020, has_artwork: true, has_cues: false, has_analysis: true, analysis_pending: false },
];

function mockTrackCountFromUrl(): number {
  if (typeof location === 'undefined') return 0;
  const n = Number(new URLSearchParams(location.search).get('mock'));
  return Number.isInteger(n) && n > 0 ? n : 0;
}

const ROW_PREVIEW_VARIANTS = 128;
const rowBandPreviews = new Map<number, string>();

function syntheticRowBands(i: number): string {
  const variant = (i * 31) % ROW_PREVIEW_VARIANTS;
  let preview = rowBandPreviews.get(variant);
  if (!preview) {
    preview = rowBandsBase64(syntheticBands(1200, variant + 1, 180 + variant, 118 + (variant % 14), MOCK_PROFILE_ORDER[variant % MOCK_PROFILE_ORDER.length]));
    rowBandPreviews.set(variant, preview);
  }
  return preview;
}

const mockArtworkDelayMs =
  typeof location !== 'undefined' ? Number(new URLSearchParams(location.search).get('artwork')) || 0 : 0;

async function mockArtworkBytes(trackId: number, maxPx: number): Promise<ArrayBuffer> {
  await new Promise((resolve) => setTimeout(resolve, mockArtworkDelayMs));
  const size = Math.min(maxPx, 512);
  const canvas = new OffscreenCanvas(size, size);
  const ctx = canvas.getContext('2d')!;
  ctx.fillStyle = `hsl(${(trackId * 47) % 360} 60% 45%)`;
  ctx.fillRect(0, 0, size, size);
  const blob = await canvas.convertToBlob({ type: 'image/jpeg', quality: 0.85 });
  return blob.arrayBuffer();
}

function syntheticMockTrack(i: number): TrackInfo {
  const analyzed = i % 10 !== 0;
  return {
    id: i + 1,
    source_path: `/Music/Artist ${i % 400}/Track ${i}.mp3`,
    title: `Track ${i}`,
    artist: `Artist ${i % 400}`,
    album: `Album ${i % 1500}`,
    genre: ['House', 'Techno', 'Drum & Bass', 'Disco', 'Ambient'][i % 5],
    tempo: analyzed ? 9000 + (i % 80) * 100 : 0,
    key: analyzed ? `${(i % 12) + 1}${i % 2 === 0 ? 'A' : 'B'}` : '',
    duration_secs: 180 + (i % 300),
    bitrate: 320,
    file_size: 9_600_000,
    has_artwork: mockArtworkDelayMs > 0,
    has_cues: false,
    has_analysis: analyzed,
    waveform_bands: analyzed ? syntheticRowBands(i) : null,
  };
}

const syntheticTrackCount = mockTrackCountFromUrl();
const mockLegacyPayload =
  typeof location !== 'undefined' && new URLSearchParams(location.search).has('legacy');
if (syntheticTrackCount > 0) {
  mockTracks = Array.from({ length: syntheticTrackCount }, (_, i) => syntheticMockTrack(i));
}

let mockPlaylists: PlaylistInput[] = [
  { id: 1, name: 'Peak Time', track_ids: [1, 2], parent_id: 0, is_folder: false },
  { id: 2, name: 'Warmup', track_ids: [3], parent_id: 0, is_folder: false }
];

let mockVolumes = ['/Volumes/SanDisk 64GB', '/Volumes/VAULT666'];

function mockCountFromUrl(param: string): number | null {
  if (typeof location === 'undefined') return null;
  const raw = new URLSearchParams(location.search).get(param);
  const n = Number(raw);
  return raw !== null && Number.isInteger(n) && n >= 0 ? n : null;
}

const mockPlaylistCount = mockCountFromUrl('playlists');
if (mockPlaylistCount !== null) {
  const names = ['Peak Time', 'Warmup', 'Sunrise Closers', 'Deep Cuts', 'Afterhours', 'Road Trip', 'Sunday Slow', 'B2B Prep'];
  mockPlaylists = Array.from({ length: mockPlaylistCount }, (_, i) => ({
    id: i + 1,
    name: names[i] ?? `Playlist ${i + 1}`,
    track_ids: [i * 2 + 1, i * 2 + 2],
    parent_id: 0,
    is_folder: false,
  }));
}
const mockStickCount = mockCountFromUrl('sticks');
if (mockStickCount !== null) mockVolumes = mockVolumes.slice(0, mockStickCount);

export async function getVersion(): Promise<string> {
  try {
    return await invoke<string>('app_version');
  } catch (err) {
    lastTauriError = `app_version fail: ${String(err)}`;
    return '0.1.0-mock';
  }
}

export async function getLibraryPath(): Promise<string> {
  try {
    return await invoke<string>('get_library_path');
  } catch (err) {
    lastTauriError = `get_library_path fail: ${String(err)}`;
    if (!useBrowserMocks()) throw err;
    return '/mock/user/library.db';
  }
}

export async function pickDirectory(): Promise<string | null> {
  if (useBrowserMocks()) {
    const path = prompt('Enter mock directory path:');
    return path || null;
  }
  try {
    const result = await dialogOpen({ directory: true, multiple: false });
    return typeof result === 'string' ? result : Array.isArray(result) ? result[0] : null;
  } catch (err) {
    lastTauriError = `pickDirectory fail: ${String(err)}`;
    throw err;
  }
}

export async function pickBackupFile(): Promise<string | null> {
  const result = await dialogOpen({ multiple: false, filters: [{ name: 'Library backup', extensions: ['gz'] }] });
  return typeof result === 'string' ? result : Array.isArray(result) ? (result[0] ?? null) : null;
}

export async function pickBackupDestination(defaultPath: string): Promise<string | null> {
  return (await dialogSave({ defaultPath, filters: [{ name: 'Library backup', extensions: ['gz'] }] })) ?? null;
}

export interface BackupSizes {
  library_bytes: number;
  audio_bytes: number;
  audio_files: number;
}

export interface LastBackup {
  path: string;
  at_ms: number;
  bytes: number;
  includes_audio: boolean;
}

export interface BackupStatus {
  sizes: BackupSizes;
  last: LastBackup | null;
}

export interface BackupPreview {
  path: string;
  created_at: string;
  track_count: number;
  playlist_count: number;
  includes_audio: boolean;
  audio_files: number;
  current_tracks: number;
}

export interface RestoreOutcome {
  ui_settings: string | null;
  safety_path: string;
  relinked: number;
}

export interface BackupProgress {
  phase: 'export' | 'restore';
  done: number;
  total: number;
}

export function getBackupStatus(): Promise<BackupStatus> {
  return invoke<BackupStatus>('get_backup_status');
}

export function exportBackup(path: string, includeAudio: boolean, uiSettings: string): Promise<LastBackup> {
  return invoke<LastBackup>('export_backup', { path, includeAudio, uiSettings });
}

export function readBackup(path: string): Promise<BackupPreview> {
  return invoke<BackupPreview>('read_backup', { path });
}

export function restoreBackup(path: string, audioFolder: string | null): Promise<RestoreOutcome> {
  return invoke<RestoreOutcome>('restore_backup', { path, audioFolder });
}

export function listenToBackupProgress(callback: (progress: BackupProgress) => void) {
  return subscribe('backup-progress', callback);
}

export async function changeLibraryPath(folderPath: string): Promise<string> {
  try {
    return await invoke<string>('change_library_path', { folderPath });
  } catch (err) {
    lastTauriError = `change_library_path fail: ${String(err)}`;
    if (!useBrowserMocks()) throw err;
    return folderPath + '/library.db';
  }
}

/** Wipe the library database at the configured path and reopen an empty library. */
export async function resetLibrary(): Promise<string> {
  try {
    const path = await invoke<string>('reset_library');
    lastTauriError = 'invoke(reset_library) OK';
    return path;
  } catch (err) {
    lastTauriError = `reset_library fail: ${String(err)}`;
    if (useBrowserMocks()) {
      mockTracks = [];
      mockPlaylists = [];
      return '/mock/user/library.db';
    }
    throw err;
  }
}

export interface TrackFields {
  title: string;
  artist: string;
  album: string;
  genre: string;
}

export type TrackFieldEdits = Partial<TrackFields>;

const MOCK_TAGS_KEY = 'fourfour.mock.tags';

function mockTagStore(): Record<string, TrackFields> {
  try {
    return JSON.parse(localStorage.getItem(MOCK_TAGS_KEY) ?? '{}');
  } catch {
    return {};
  }
}

function applyMockTags(): void {
  const stored = mockTagStore();
  mockTracks = mockTracks.map((t) => (stored[t.id] ? { ...t, ...stored[t.id] } : t));
}

function updateMockTrackFields(trackId: number, edits: TrackFieldEdits): TrackFields {
  const track = mockTracks.find((t) => t.id === trackId);
  if (!track) throw new Error(`Track ${trackId} is not in the library`);
  const title = edits.title?.trim();
  if (title === '') throw new Error('Title cannot be empty');
  const next: TrackFields = {
    title: title ?? track.title,
    artist: edits.artist?.trim() ?? track.artist,
    album: edits.album?.trim() ?? track.album,
    genre: edits.genre?.trim() ?? track.genre,
  };
  Object.assign(track, next);
  try {
    localStorage.setItem(MOCK_TAGS_KEY, JSON.stringify({ ...mockTagStore(), [trackId]: next }));
  } catch {
    return next;
  }
  return next;
}

/** Edit the library tags (title, artist, album, genre) of a track; file tags are never touched. */
export async function updateTrackFields(trackId: number, edits: TrackFieldEdits): Promise<TrackFields> {
  try {
    return await invoke<TrackFields>('update_track_fields', { trackId, edits });
  } catch (err) {
    if (!useBrowserMocks()) throw err;
    return updateMockTrackFields(trackId, edits);
  }
}

export async function loadState(): Promise<LoadedState> {
  try {
    const res = await invoke<LoadedState>('load_state');
    lastTauriError = 'invoke(load_state) OK';
    return res;
  } catch (err) {
    lastTauriError = `load_state fail: ${String(err)}`;
    if (useBrowserMocks()) {
      applyMockTags();
      return { tracks: mockTracks, playlists: mockPlaylists };
    }
    throw err;
  }
}

export async function saveState(playlists: PlaylistInput[]): Promise<void> {
  try {
    return await invoke<void>('save_state', { playlists });
  } catch (err) {
    lastTauriError = `save_state fail: ${String(err)}`;
    mockPlaylists = playlists;
  }
}

export type DuplicateReason = 'same_file' | 'same_tags' | 'same_path';

export interface DuplicateRow {
  new_path: string;
  file_name: string;
  existing_id: number;
  title: string;
  artist: string;
  duration_secs: number;
  reason: DuplicateReason;
}

export interface SkippedFiles {
  unsupported: number;
  unsupported_exts: string[];
  unreadable: number;
}

export interface ImportPlan {
  token: number;
  new_count: number;
  already_in_library: number;
  duplicates: DuplicateRow[];
  skipped: SkippedFiles;
}

export interface ImportCommit {
  added: TrackInfo[];
  skipped_ids: number[];
}

let mockPendingImport: TrackInfo[] = [];

function mockScannedTrack(path: string, i: number): TrackInfo {
  return {
    id: Date.now() + i,
    source_path: path,
    title: path.split('/').pop() || 'Dropped Track',
    artist: 'Dropped Artist',
    album: 'Dropped Album',
    genre: 'House',
    tempo: 12000,
    key: '8A',
    duration_secs: 300,
    bitrate: 320,
    file_size: 12000000,
    has_artwork: false,
    has_cues: false,
  };
}

export async function planImport(paths: string[]): Promise<ImportPlan> {
  try {
    return await invoke<ImportPlan>('plan_import', { paths });
  } catch (err) {
    lastTauriError = `plan_import fail: ${String(err)}`;
    if (useBrowserMocks()) {
      mockPendingImport = paths.map(mockScannedTrack);
      return {
        token: 0,
        new_count: mockPendingImport.length,
        already_in_library: 0,
        duplicates: [],
        skipped: { unsupported: 0, unsupported_exts: [], unreadable: 0 },
      };
    }
    throw err;
  }
}

export async function commitImport(token: number, skipDuplicates: boolean): Promise<ImportCommit> {
  try {
    return await invoke<ImportCommit>('commit_import', { token, skipDuplicates });
  } catch (err) {
    lastTauriError = `commit_import fail: ${String(err)}`;
    if (useBrowserMocks()) {
      const added = mockPendingImport;
      mockPendingImport = [];
      mockTracks.push(...added);
      return { added, skipped_ids: [] };
    }
    throw err;
  }
}

export async function revealInFileManager(path: string): Promise<void> {
  await invoke<void>('reveal_in_finder', { path });
}

export async function removeTracks(ids: number[]): Promise<void> {
  try {
    return await invoke<void>('remove_tracks', { ids });
  } catch (err) {
    lastTauriError = `remove_tracks fail: ${String(err)}`;
    mockTracks = mockTracks.filter(t => !ids.includes(t.id));
    mockPlaylists.forEach(pl => {
      pl.track_ids = pl.track_ids.filter(tid => !ids.includes(tid));
    });
  }
}

export async function setTestCues(ids: number[]): Promise<TrackInfo[]> {
  try {
    return await invoke<TrackInfo[]>('set_test_cues', { ids });
  } catch (err) {
    lastTauriError = `set_test_cues fail: ${String(err)}`;
    mockTracks.forEach(t => {
      if (ids.includes(t.id)) {
        t.has_cues = true;
      }
    });
    return mockTracks;
  }
}

export async function enqueueAnalysis(ids: number[], front = false): Promise<number> {
  try {
    return await invoke<number>('enqueue_analysis', { ids, front });
  } catch (err) {
    lastTauriError = `enqueue_analysis fail: ${String(err)}`;
    return 0;
  }
}

export async function prioritizeAnalysis(trackId: number): Promise<void> {
  try {
    await invoke<void>('prioritize_analysis', { trackId });
    lastTauriError = 'invoke(prioritize_analysis) OK';
  } catch (err) {
    lastTauriError = `prioritize_analysis fail: ${String(err)}`;
  }
}

const MOCK_STAGES_ALL = 7;
const MOCK_STAGES_WAVEFORM_AND_KEY = 3;
const MOCK_FAST_PASS_MS = 700;
const MOCK_GRID_PASS_MS = 1500;
const MOCK_TRACK_GAP_MS = 1600;
const mockStages = new Map<number, number>();
const mockAnalysisListeners = {
  progress: new Set<(payload: { current: number; total: number; message: string; track_id?: number }) => void>(),
  partial: new Set<(payload: { track_id: number }) => void>(),
  complete: new Set<() => void>(),
};

/** Browser stand-in for the staged run: waveform and key first, then BPM and grid, one `analysis-*` event per stage. */
function runMockAnalysis(ids: number[]): Promise<TrackInfo[]> {
  ids.forEach((id) => mockStages.set(id, 0));
  return new Promise((resolve) => {
    ids.forEach((id, index) => {
      const start = index * MOCK_TRACK_GAP_MS;
      setTimeout(() => {
        const track = mockTracks.find((t) => t.id === id);
        if (track) track.key = '3A';
        mockStages.set(id, MOCK_STAGES_WAVEFORM_AND_KEY);
        invalidateAnalysisCache(id);
        mockAnalysisListeners.partial.forEach((cb) => cb({ track_id: id }));
      }, start + MOCK_FAST_PASS_MS);
      setTimeout(() => {
        const track = mockTracks.find((t) => t.id === id);
        if (track) {
          track.tempo = 12600 + (id % 7) * 100;
          track.has_analysis = true;
        }
        mockStages.set(id, MOCK_STAGES_ALL);
        invalidateAnalysisCache(id);
        const done = index + 1;
        mockAnalysisListeners.progress.forEach((cb) =>
          cb({ current: done, total: ids.length, message: `Analyzed ${track?.title ?? id} (${done}/${ids.length})`, track_id: id }),
        );
        if (done === ids.length) {
          mockAnalysisListeners.complete.forEach((cb) => cb());
          resolve(mockTracks);
        }
      }, start + MOCK_GRID_PASS_MS);
    });
  });
}

export async function analyzeTrackIds(ids: number[]): Promise<TrackInfo[]> {
  try {
    return await invoke<TrackInfo[]>('analyze_track_ids', { ids });
  } catch (err) {
    lastTauriError = `analyze_track_ids fail: ${String(err)}`;
    return runMockAnalysis(ids);
  }
}

const MOCK_SETTINGS_KEY = 'fourfour.mock.settings';

function mockSettings(): Record<string, unknown> {
  try {
    return JSON.parse(localStorage.getItem(MOCK_SETTINGS_KEY) ?? '{}');
  } catch {
    return {};
  }
}

function writeMockSetting(key: string, value: unknown) {
  try {
    localStorage.setItem(MOCK_SETTINGS_KEY, JSON.stringify({ ...mockSettings(), [key]: value }));
  } catch {
    mockSettingFallback[key] = value;
  }
}

const mockSettingFallback: Record<string, unknown> = {};

function readMockSetting<T>(key: string, fallback: T): T {
  const value = mockSettings()[key] ?? mockSettingFallback[key];
  return (value ?? fallback) as T;
}

/** Whether never-analysed tracks are queued at launch (default on). */
export async function getAutoAnalyzeNew(): Promise<boolean> {
  try {
    return await invoke<boolean>('get_auto_analyze_new');
  } catch (err) {
    if (!useBrowserMocks()) throw err;
    return readMockSetting('auto_analyze_new_tracks', true);
  }
}

export async function setAutoAnalyzeNew(enabled: boolean): Promise<void> {
  try {
    await invoke('set_auto_analyze_new', { enabled });
  } catch (err) {
    if (!useBrowserMocks()) throw err;
    writeMockSetting('auto_analyze_new_tracks', enabled);
  }
}

/** Stored analysis backend: 'rust', 'python', or null for automatic. */
export async function getAnalysisBackend(): Promise<string | null> {
  try {
    return await invoke<string | null>('get_analysis_backend');
  } catch (err) {
    if (!useBrowserMocks()) throw err;
    return readMockSetting<string | null>('analysis_backend', null);
  }
}

export async function setAnalysisBackend(backend: string | null): Promise<void> {
  try {
    await invoke('set_analysis_backend', { backend });
  } catch (err) {
    if (!useBrowserMocks()) throw err;
    writeMockSetting('analysis_backend', backend);
  }
}

/** Stop the active analysis run: queued tracks are dropped, in-flight ones aborted. */
export async function cancelAnalysis(): Promise<void> {
  await invokeOrMock('cancel_analysis', {});
}

/** Queues stages left missing by an earlier session (and new tracks, per setting); returns how many. */
export async function resumeAnalysis(): Promise<number> {
  try {
    return await invoke<number>('resume_analysis');
  } catch (err) {
    lastTauriError = `resume_analysis fail: ${String(err)}`;
    return 0;
  }
}

export async function analyzeTracks(): Promise<TrackInfo[]> {
  try {
    return await invoke<TrackInfo[]>('analyze_tracks');
  } catch (err) {
    lastTauriError = `analyze_tracks fail: ${String(err)}`;
    return new Promise((resolve) => {
      setTimeout(() => {
        mockTracks.forEach(t => {
          if (t.tempo === 0 || !t.key || t.key === '—') {
            t.tempo = 12600;
            t.key = '3A';
            t.duration_secs = 380;
          }
        });
        resolve(mockTracks);
      }, 1500);
    });
  }
}

export async function getMountedVolumes(): Promise<string[]> {
  try {
    return await invoke<string[]>('get_mounted_volumes');
  } catch (err) {
    lastTauriError = `get_mounted_volumes fail: ${String(err)}`;
    return mockVolumes;
  }
}

export async function readUsbState(path: string): Promise<UsbStateResponse | null> {
  try {
    return await invoke<UsbStateResponse | null>('read_usb_state', { path });
  } catch (err) {
    lastTauriError = `read_usb_state fail: ${String(err)}`;
    if (!useBrowserMocks()) throw err;
    return {
      tracks: [
        { id: 1, title: 'USB Track 1', artist: 'Artist A', album: 'Album A', genre: 'House', key: '5A', bpm: 124.0, duration: 400, usb_path: '/Contents/Artist A/Album A/01 Track 1.mp3' }
      ],
      playlists: [
        { id: 1, name: 'USB Playlist 1', track_count: 1 }
      ]
    };
  }
}

export interface StickContents {
  stick_id: string | null;
  track_ids: number[];
  playlist_ids: number[];
}

export interface VolumeCapacity {
  total_bytes: number;
  free_bytes: number;
}

export type SyncSelection =
  | { kind: 'whole_library' }
  | { kind: 'playlists'; playlist_ids: number[]; track_ids: number[] };

export interface SyncTarget {
  volume: string;
  selection: SyncSelection;
}

export interface PlanTrack {
  track_id: number | null;
  title: string;
  artist: string;
  size_bytes: number | null;
}

export interface UsbSyncPlan {
  volume: string;
  stick_id: string | null;
  playlist_count: number;
  add: PlanTrack[];
  remove: PlanTrack[];
  update_count: number;
  unchanged_count: number;
  skipped_unanalyzed: PlanTrack[];
  foreign_kept_count: number;
  bytes_to_add: number;
  bytes_to_free: number;
  bytes_needed: number;
  free_bytes: number | null;
  fits: boolean | null;
}

export type SyncPhase = 'checking-space' | 'copying' | 'writing-database' | 'done' | 'failed';

export interface StickProgress {
  volume: string;
  phase: SyncPhase;
  done: number;
  total: number;
  message: string;
}

export interface StickReport {
  volume: string;
  path: string;
  status: 'done' | 'failed';
  report: SyncReport | null;
  error: string | null;
  skipped_unanalyzed: number;
  elapsed_ms: number;
}

const GB = 1_000_000_000;
const MOCK_TRACK_MARGIN = 128 * 1024;
const MOCK_BASE_MARGIN = 16 * 1024 * 1024;

const mockCapacities: Record<string, VolumeCapacity> = {
  '/Volumes/SanDisk 64GB': { total_bytes: 64 * GB, free_bytes: 22.8 * GB },
  '/Volumes/VAULT666': { total_bytes: 8 * GB, free_bytes: 100_000_000 },
};

const mockSticks = new Map<string, StickContents>();
const mockWritten = new Map<string, Set<number>>([['/Volumes/SanDisk 64GB', new Set([1, 2])]]);
const mockSyncListeners = new Set<(progress: StickProgress) => void>();

function mockStick(path: string): StickContents {
  return mockSticks.get(path) ?? { stick_id: null, track_ids: [], playlist_ids: [] };
}

function mockEditStick(path: string, edit: (stick: StickContents) => number): [number, StickContents] {
  const current = mockStick(path);
  const stick = {
    stick_id: current.stick_id ?? `mock-${path.split('/').pop()}`,
    track_ids: [...current.track_ids],
    playlist_ids: [...current.playlist_ids],
  };
  const changed = edit(stick);
  mockSticks.set(path, stick);
  return [changed, stick];
}

function appendNew(list: number[], ids: number[]): number {
  const fresh = [...new Set(ids)].filter((id) => !list.includes(id));
  list.push(...fresh);
  return fresh.length;
}

function dropIds(list: number[], ids: number[]): number {
  const before = list.length;
  const kept = list.filter((id) => !ids.includes(id));
  list.splice(0, list.length, ...kept);
  return before - kept.length;
}

function mockWantedTrackIds(selection: SyncSelection): number[] {
  if (selection.kind === 'whole_library') return mockTracks.map((t) => t.id);
  const wanted = new Set(selection.track_ids);
  for (const playlist of mockPlaylists) {
    if (selection.playlist_ids.includes(playlist.id)) playlist.track_ids.forEach((id) => wanted.add(id));
  }
  return [...wanted];
}

function mockPlanTrack(id: number): PlanTrack {
  const track = mockTracks.find((t) => t.id === id);
  return {
    track_id: id,
    title: track?.title ?? `Track ${id}`,
    artist: track?.artist ?? '',
    size_bytes: track?.file_size ?? null,
  };
}

function mockPlan(target: SyncTarget): UsbSyncPlan {
  const wanted = mockWantedTrackIds(target.selection);
  const analyzed = wanted.filter((id) => mockTracks.find((t) => t.id === id)?.has_analysis);
  const written = mockWritten.get(target.volume) ?? new Set<number>();
  const add = analyzed.filter((id) => !written.has(id)).map(mockPlanTrack);
  const remove = [...written].filter((id) => !analyzed.includes(id)).map(mockPlanTrack);
  const bytesToAdd = add.reduce((sum, t) => sum + (t.size_bytes ?? 0), 0);
  const needed = bytesToAdd + MOCK_BASE_MARGIN + MOCK_TRACK_MARGIN * analyzed.length;
  const free = mockCapacities[target.volume]?.free_bytes ?? null;
  return {
    volume: target.volume.split('/').pop() ?? target.volume,
    stick_id: mockStick(target.volume).stick_id,
    playlist_count: target.selection.kind === 'playlists' ? target.selection.playlist_ids.length : mockPlaylists.length,
    add,
    remove,
    update_count: 0,
    unchanged_count: analyzed.length - add.length,
    skipped_unanalyzed: wanted.filter((id) => !analyzed.includes(id)).map(mockPlanTrack),
    foreign_kept_count: 0,
    bytes_to_add: bytesToAdd,
    bytes_to_free: remove.reduce((sum, t) => sum + (t.size_bytes ?? 0), 0),
    bytes_needed: needed,
    free_bytes: free,
    fits: free === null ? null : needed <= free,
  };
}

function mockSyncSteps(target: SyncTarget): Promise<StickReport> {
  const plan = mockPlan(target);
  const volume = plan.volume;
  const total = plan.add.length + plan.unchanged_count;
  const emit = (phase: SyncPhase, done: number, message: string) =>
    mockSyncListeners.forEach((listener) => listener({ volume, phase, done, total, message }));
  return new Promise((resolve) => {
    emit('checking-space', 0, 'Checking free space');
    if (plan.fits === false) {
      const error = `Not enough space on ${volume}. Nothing was written.`;
      emit('failed', 0, error);
      resolve({ volume, path: target.volume, status: 'failed', report: null, error, skipped_unanalyzed: plan.skipped_unanalyzed.length, elapsed_ms: 0 });
      return;
    }
    let step = 0;
    const timer = setInterval(() => {
      step += 1;
      if (step <= plan.add.length) {
        emit('copying', plan.unchanged_count + step, `Copying ${plan.add[step - 1].artist} - ${plan.add[step - 1].title}`);
        return;
      }
      clearInterval(timer);
      emit('writing-database', total, 'Writing database');
      const kept = new Set(mockWantedTrackIds(target.selection).filter((id) => mockTracks.find((t) => t.id === id)?.has_analysis));
      mockWritten.set(target.volume, kept);
      const capacity = mockCapacities[target.volume];
      if (capacity) capacity.free_bytes = Math.max(0, capacity.free_bytes - plan.bytes_to_add + plan.bytes_to_free);
      emit('done', total, 'Safe to eject');
      resolve({
        volume,
        path: target.volume,
        status: 'done',
        report: {
          tracks_added: plan.add.length,
          tracks_updated: 0,
          tracks_replaced: 0,
          tracks_removed: plan.remove.length,
          tracks_unchanged: plan.unchanged_count,
        },
        error: null,
        skipped_unanalyzed: plan.skipped_unanalyzed.length,
        elapsed_ms: step * 150,
      });
    }, 150);
  });
}

export async function getStick(path: string): Promise<StickContents> {
  try {
    return await invoke<StickContents>('get_stick', { path });
  } catch (err) {
    lastTauriError = `get_stick fail: ${String(err)}`;
    if (!useBrowserMocks()) throw err;
    return mockStick(path);
  }
}

async function editStick(
  command: string,
  args: Record<string, unknown>,
  path: string,
  mock: (stick: StickContents) => number
): Promise<[number, StickContents]> {
  try {
    return await invoke<[number, StickContents]>(command, { path, ...args });
  } catch (err) {
    lastTauriError = `${command} fail: ${String(err)}`;
    if (!useBrowserMocks()) throw err;
    return mockEditStick(path, mock);
  }
}

export function stickAddTracks(path: string, trackIds: number[]) {
  return editStick('stick_add_tracks', { trackIds }, path, (s) => appendNew(s.track_ids, trackIds));
}

export function stickRemoveTracks(path: string, trackIds: number[]) {
  return editStick('stick_remove_tracks', { trackIds }, path, (s) => dropIds(s.track_ids, trackIds));
}

export function stickAddPlaylists(path: string, playlistIds: number[]) {
  return editStick('stick_add_playlists', { playlistIds }, path, (s) => appendNew(s.playlist_ids, playlistIds));
}

export function stickRemovePlaylists(path: string, playlistIds: number[]) {
  return editStick('stick_remove_playlists', { playlistIds }, path, (s) => dropIds(s.playlist_ids, playlistIds));
}

export async function getVolumeCapacity(path: string): Promise<VolumeCapacity | null> {
  try {
    return await invoke<VolumeCapacity>('get_volume_capacity', { path });
  } catch (err) {
    lastTauriError = `get_volume_capacity fail: ${String(err)}`;
    if (!useBrowserMocks()) return null;
    return mockCapacities[path] ? { ...mockCapacities[path] } : null;
  }
}

export async function planUsbSync(target: SyncTarget): Promise<UsbSyncPlan> {
  try {
    return await invoke<UsbSyncPlan>('plan_usb_sync', { target });
  } catch (err) {
    lastTauriError = `plan_usb_sync fail: ${String(err)}`;
    if (!useBrowserMocks()) throw err;
    return mockPlan(target);
  }
}

export async function syncUsbMany(targets: SyncTarget[]): Promise<StickReport[]> {
  try {
    return await invoke<StickReport[]>('sync_usb_many', { targets });
  } catch (err) {
    lastTauriError = `sync_usb_many fail: ${String(err)}`;
    if (!useBrowserMocks()) throw err;
    return Promise.all(targets.map(mockSyncSteps));
  }
}

export function listenToUsbSyncProgress(callback: (progress: StickProgress) => void) {
  if (useBrowserMocks()) {
    mockSyncListeners.add(callback);
    return () => mockSyncListeners.delete(callback);
  }
  return unlistenWhenReady(
    listen<StickProgress>('usb-sync-progress', (event) => callback(event.payload)),
    (err) => {
      lastTauriError = `listen(usb-sync-progress) fail: ${String(err)}`;
    },
  );
}

export async function ejectVolume(path: string): Promise<void> {
  try {
    return await invoke<void>('eject_volume', { path });
  } catch (err) {
    lastTauriError = `eject_volume fail: ${String(err)}`;
    mockVolumes = mockVolumes.filter(v => v !== path);
  }
}

export async function wipeUsb(path: string): Promise<void> {
  try {
    return await invoke<void>('wipe_usb', { path });
  } catch (err) {
    lastTauriError = `wipe_usb fail: ${String(err)}`;
  }
}

const ANALYSIS_CACHE_LIMIT = 48;
const analysisCache = new Map<number, any>();
const analysisInflight = new Map<number, Promise<any>>();

function cacheAnalysis(trackId: number, data: any) {
  analysisCache.delete(trackId);
  analysisCache.set(trackId, data);
  if (analysisCache.size > ANALYSIS_CACHE_LIMIT) {
    analysisCache.delete(analysisCache.keys().next().value as number);
  }
}

export function peekAnalysisData(trackId: number): any | undefined {
  const cached = analysisCache.get(trackId);
  if (!cached) return undefined;
  const clean = sanitizeAnalysisPayload(cached);
  if (!clean) {
    analysisCache.delete(trackId);
    return undefined;
  }
  cacheAnalysis(trackId, clean);
  return clean;
}

export function invalidateAnalysisCache(trackId?: number) {
  if (trackId == null) {
    analysisCache.clear();
    analysisInflight.clear();
  } else {
    analysisCache.delete(trackId);
    analysisInflight.delete(trackId);
  }
}

const mockBeatGrids = new Map<number, { beats: BeatMarker[]; offsetMs: number; anchorMs: number | null }>();

/**
 * Store an edited beat grid (exact beat times); the next analysis read of the track carries it. The library keeps whole
 * ms beats and the grid's sub-millisecond offset (`gridForStorage`), so the grid comes back exactly where it was set.
 */
export async function setBeatGrid(trackId: number, grid: EditedGrid): Promise<void> {
  const { beats, offsetMs } = gridForStorage(grid.beats);
  const anchorMs = grid.anchorMs;
  try {
    await invoke('set_beat_grid', { trackId, beats, offsetMs, anchorMs });
  } catch (err) {
    if (!useBrowserMocks()) throw err;
    mockBeatGrids.set(trackId, { beats, offsetMs, anchorMs });
  }
  invalidateAnalysisCache(trackId);
}

async function invokeOrMock(command: string, args: Record<string, unknown>): Promise<void> {
  try {
    await invoke(command, args);
  } catch (err) {
    if (!useBrowserMocks()) throw err;
  }
}

/** Set a track's Camelot key by hand; re-analysis keeps it. */
export async function setTrackKey(trackId: number, key: string): Promise<void> {
  await invokeOrMock('set_track_key', { trackId, key });
  invalidateAnalysisCache(trackId);
}

/** Set a track's star rating by hand (0 clears it); it shows over the imported rating. */
export async function setTrackRating(trackId: number, rating: number): Promise<void> {
  await invokeOrMock('set_track_rating', { trackId, rating });
}

/** Let re-analysis replace the hand-set BPM and key of a track again. */
export async function releaseUserEdits(trackId: number): Promise<void> {
  await invokeOrMock('release_user_edits', { trackId });
  invalidateAnalysisCache(trackId);
}

/** Set a track's own BPM by hand: the beat grid is re-spaced from its first downbeat, re-analysis keeps both. */
export async function rescaleBeatGrid(trackId: number, bpm: number): Promise<void> {
  await invokeOrMock('rescale_beat_grid', { trackId, bpm });
  invalidateAnalysisCache(trackId);
}

const MOCK_CUES_KEY = 'fourfour.mock.cues';

function mockCueStore(): Record<string, CuePointPayload[]> {
  try {
    return JSON.parse(localStorage.getItem(MOCK_CUES_KEY) ?? '{}');
  } catch {
    return {};
  }
}

/** Replace the cues of a track (slot 0 memory, 1 to 8 hot A to H); the next analysis read carries them. */
export async function setCues(trackId: number, cues: CuePointPayload[]): Promise<void> {
  try {
    await invoke('set_cues', { trackId, cues });
  } catch (err) {
    if (!useBrowserMocks()) throw err;
    try {
      localStorage.setItem(MOCK_CUES_KEY, JSON.stringify({ ...mockCueStore(), [trackId]: cues }));
    } catch {
      mockCueFallback.set(trackId, cues);
    }
  }
  invalidateAnalysisCache(trackId);
}

const mockCueFallback = new Map<number, CuePointPayload[]>();

function mockEditedCues(trackId: number): CuePointPayload[] | undefined {
  return mockCueStore()[trackId] ?? mockCueFallback.get(trackId);
}

/** The 2400 columns per second 3-band series of a track (`FFWD` bytes), measured from the audio on first use. `null` where there is no backend or no audio. */
export async function getWaveformDetail(trackId: number): Promise<ArrayBuffer | null> {
  try {
    return await invoke<ArrayBuffer>('get_waveform_detail', { trackId });
  } catch (err) {
    lastTauriError = `get_waveform_detail fail: ${String(err)}`;
    return null;
  }
}

/** `columns` columns of the zoom view from `startMs` to `endMs` (`FFWW` bytes), decoded from the audio now. Rejects when a newer request superseded it. */
export function getWaveformWindow(trackId: number, startMs: number, endMs: number, columns: number): Promise<ArrayBuffer> {
  return invoke<ArrayBuffer>('get_waveform_window', { trackId, startS: startMs / 1000, endS: endMs / 1000, columns });
}

async function fetchAnalysisData(trackId: number): Promise<any> {
  try {
    const data = await invoke<any>('get_analysis_data', { trackId });
    const clean = sanitizeAnalysisPayload(data);
    if (!clean) {
      throw new Error('Invalid analysis payload from backend');
    }
    cacheAnalysis(trackId, clean);
    return clean;
  } catch (err) {
    lastTauriError = `get_analysis_data fail: ${String(err)}`;
    analysisCache.delete(trackId);
    const mock = mockTracks.find((t) => t.id === trackId);
    if (useBrowserMocks() && mock) {
      const data =
        syntheticTrackCount > 0
          ? perfAnalysisData(mock.duration_secs, mock.tempo, mock.key, mockLegacyPayload, trackId)
          : mockAnalysisData(trackId, mock.duration_secs, mock.tempo, mock.key);
      const edited = mockBeatGrids.get(trackId);
      const editedCues = mockEditedCues(trackId);
      const clean = sanitizeAnalysisPayload({
        ...data,
        analysis_stages: mockStages.get(trackId) ?? MOCK_STAGES_ALL,
        ...(edited ? { beats: edited.beats, grid_offset_ms: edited.offsetMs, grid_anchor_ms: edited.anchorMs } : {}),
        ...(editedCues ? { cue_points: editedCues } : {}),
      });
      cacheAnalysis(trackId, clean);
      return clean;
    }
    throw err;
  }
}

export async function getAnalysisData(trackId: number, force = false): Promise<any> {
  if (!force) {
    const hit = peekAnalysisData(trackId);
    if (hit) return hit;
    const pending = analysisInflight.get(trackId);
    if (pending) return pending;
  }
  const request = fetchAnalysisData(trackId).finally(() => {
    if (analysisInflight.get(trackId) === request) analysisInflight.delete(trackId);
  });
  analysisInflight.set(trackId, request);
  return request;
}

const PREFETCH_DELAY_MS = 120;

/** Warm the analysis cache for tracks the user is likely to select or load next. */
export function prefetchAnalysisData(trackIds: number[]) {
  setTimeout(async () => {
    for (const trackId of trackIds) {
      if (analysisCache.has(trackId) || analysisInflight.has(trackId)) continue;
      await getAnalysisData(trackId).catch(() => undefined);
    }
  }, PREFETCH_DELAY_MS);
}

export const ARTWORK_DETAIL_PX = 512;
export const ARTWORK_ROW_PX = 64;

export async function getTrackArtwork(trackId: number, maxPx = ARTWORK_DETAIL_PX): Promise<ArrayBuffer | null> {
  if (useBrowserMocks() && mockArtworkDelayMs > 0) return mockArtworkBytes(trackId, maxPx);
  try {
    const bytes = await invoke<ArrayBuffer>('get_track_artwork', { trackId, maxPx });
    return bytes.byteLength > 0 ? bytes : null;
  } catch (err) {
    lastTauriError = `get_track_artwork fail: ${String(err)}`;
    return null;
  }
}

export function listenToAnalysisProgress(
  callback: (payload: {
    current: number;
    total: number;
    message: string;
    track_id?: number;
  }) => void,
) {
  if (useBrowserMocks()) mockAnalysisListeners.progress.add(callback);
  return subscribe('analysis-progress', callback);
}

/** The waveform of a prioritised track is stored; tempo, grid and key follow with the full result. */
export function listenToAnalysisPartial(callback: (payload: { track_id: number }) => void) {
  if (useBrowserMocks()) mockAnalysisListeners.partial.add(callback);
  return subscribe('analysis-partial', callback);
}

export function listenToAnalysisComplete(callback: () => void) {
  if (useBrowserMocks()) mockAnalysisListeners.complete.add(callback);
  return subscribe<void>('analysis-complete', () => callback());
}

export function listenToWriteComplete(callback: (payload: SyncReport) => void) {
  return subscribe('write-complete', callback);
}

function subscribe<T>(eventName: string, callback: (payload: T) => void): () => void {
  const onError = (err: unknown) => {
    lastTauriError = `listen(${eventName}) fail: ${String(err)}`;
  };
  try {
    return unlistenWhenReady(listen<T>(eventName, (event) => callback(event.payload)), onError);
  } catch (err) {
    onError(err);
    return () => {};
  }
}

export async function pickFile(
  filter?: { name: string; extensions: string[] },
): Promise<string | null> {
  if (useBrowserMocks()) {
    return prompt('Enter mock file path:') || null;
  }
  try {
    const result = await dialogOpen({
      directory: false,
      multiple: false,
      filters: filter ? [filter] : undefined,
    });
    return typeof result === 'string' ? result : Array.isArray(result) ? result[0] : null;
  } catch (err) {
    lastTauriError = `pickFile fail: ${String(err)}`;
    throw err;
  }
}

export type RekordboxSourceKind = 'master_db' | 'usb' | 'xml';

export type RekordboxImportRequest = {
  kind: RekordboxSourceKind;
  path: string;
  remaps: RekordboxRemap[];
  include_missing: boolean;
};

export type RekordboxRemap = { from_prefix: string; to_prefix: string };

export type RekordboxRemapStat = RekordboxRemap & {
  tracks: number;
  files_found: number;
  files_missing: number;
};

export type RekordboxImportPreview = {
  source: RekordboxSourceKind;
  tracks: number;
  playlists: number;
  folders: number;
  skipped_smart_playlists: number;
  hot_cues: number;
  memory_cues: number;
  tracks_with_grid: number;
  detected_root: string;
  remap_stats: RekordboxRemapStat[];
  unmapped: RekordboxRemapStat;
  files_found: number;
  files_missing: number;
  missing_examples: string[];
  conflicts: number;
  new_tracks: number;
  playlist_conflicts: number;
};

export type RekordboxImportReport = {
  tracks_imported: number;
  tracks_merged: number;
  tracks_skipped_missing: number;
  files_found: number;
  files_missing: number;
  playlists_imported: number;
  playlists_merged: number;
  grids_imported: number;
  hot_cues_imported: number;
  memory_cues_imported: number;
  cues_without_analysis: number;
};

export type RekordboxImportProgress = { stage: string; done: number; total: number };

export async function rekordboxDefaultMasterDb(): Promise<string | null> {
  if (useBrowserMocks()) return '/Users/merlin/Library/Pioneer/rekordbox/master.db';
  try {
    return await invoke<string | null>('rekordbox_default_master_db');
  } catch (err) {
    lastTauriError = `rekordbox_default_master_db fail: ${String(err)}`;
    return null;
  }
}

function mockRekordboxPreview(request: RekordboxImportRequest): RekordboxImportPreview {
  const stat = (from: string, to: string, tracks: number, missing: number): RekordboxRemapStat => ({
    from_prefix: from,
    to_prefix: to,
    tracks,
    files_found: tracks - missing,
    files_missing: missing,
  });
  const rules = request.remaps.map((r, i) => stat(r.from_prefix, r.to_prefix, [631, 698, 32][i] ?? 0, [21, 4, 0][i] ?? 0));
  const caught = rules.reduce((n, r) => n + r.tracks, 0);
  const missing = rules.reduce((n, r) => n + r.files_missing, 0);
  return {
    source: request.kind,
    tracks: 1382,
    playlists: 4,
    folders: 1,
    skipped_smart_playlists: 0,
    hot_cues: 212,
    memory_cues: 340,
    tracks_with_grid: 1374,
    detected_root: '/Users/merlinkraemer/local/2-Areas/1-Mukke',
    remap_stats: rules,
    unmapped: stat('', '', 1382 - caught, 1382 - caught),
    files_found: caught - missing,
    files_missing: 1382 - caught + missing,
    missing_examples: [],
    conflicts: 120,
    new_tracks: 1262,
    playlist_conflicts: 1,
  };
}

export async function rekordboxImportPreview(
  request: RekordboxImportRequest,
): Promise<RekordboxImportPreview> {
  if (useBrowserMocks()) return mockRekordboxPreview(request);
  try {
    return await invoke<RekordboxImportPreview>('rekordbox_import_preview', { request });
  } catch (err) {
    lastTauriError = `rekordbox_import_preview fail: ${String(err)}`;
    throw err;
  }
}

export async function rekordboxImportRun(
  request: RekordboxImportRequest,
): Promise<RekordboxImportReport> {
  if (useBrowserMocks()) {
    return {
      tracks_imported: 1262,
      tracks_merged: 120,
      tracks_skipped_missing: 0,
      files_found: 1382,
      files_missing: 0,
      playlists_imported: 3,
      playlists_merged: 1,
      grids_imported: 1374,
      hot_cues_imported: 212,
      memory_cues_imported: 340,
      cues_without_analysis: 0,
    };
  }
  try {
    return await invoke<RekordboxImportReport>('rekordbox_import_run', { request });
  } catch (err) {
    lastTauriError = `rekordbox_import_run fail: ${String(err)}`;
    throw err;
  }
}

export function listenToRekordboxImportProgress(
  callback: (payload: RekordboxImportProgress) => void,
) {
  let unsubscribe: () => void = () => {};
  listen<RekordboxImportProgress>('rekordbox-import-progress', (event) => {
    callback(event.payload);
  })
    .then((unsub) => {
      unsubscribe = unsub;
    })
    .catch((err) => {
      lastTauriError = `listen(rekordbox-import-progress) fail: ${String(err)}`;
    });
  return () => unsubscribe();
}

export interface LibraryHealth {
  running: boolean;
  checked_at_ms: number | null;
  duration_ms: number;
  total: number;
  missing: number[];
  error: number[];
  relocated: number[];
}

export interface Relinked {
  track_id: number;
  source_path: string;
}

export interface RelocateReport {
  matched: Relinked[];
  ambiguous: number[];
  not_found: number[];
}

function mockMissingIds(): number[] {
  if (typeof location === 'undefined') return [];
  const raw = new URLSearchParams(location.search).get('missing') ?? '';
  return raw.split(',').map(Number).filter((n) => Number.isInteger(n) && n > 0);
}

let mockHealth: LibraryHealth = {
  running: false,
  checked_at_ms: null,
  duration_ms: 0,
  total: 0,
  missing: mockMissingIds(),
  error: [],
  relocated: [],
};

function mockChecked(): LibraryHealth {
  mockHealth = { ...mockHealth, checked_at_ms: Date.now(), duration_ms: 12, total: mockTracks.length };
  return mockHealth;
}

export async function checkLibraryFiles(): Promise<LibraryHealth> {
  if (useBrowserMocks()) return mockChecked();
  return invoke<LibraryHealth>('check_library_files');
}

export async function getLibraryHealth(): Promise<LibraryHealth> {
  if (useBrowserMocks()) return mockHealth.checked_at_ms ? mockHealth : mockChecked();
  return invoke<LibraryHealth>('get_library_health');
}

export async function relocateMissingInFolder(folder: string): Promise<{ report: RelocateReport; health: LibraryHealth }> {
  if (useBrowserMocks()) {
    const matched = mockHealth.missing.map((id) => ({ track_id: id, source_path: `${folder}/track-${id}.mp3` }));
    mockHealth = { ...mockHealth, missing: [], relocated: [...mockHealth.relocated, ...matched.map((m) => m.track_id)] };
    return { report: { matched, ambiguous: [], not_found: [] }, health: mockHealth };
  }
  return invoke('relocate_missing_in_folder', { folder });
}

export async function locateTrackFile(trackId: number, path: string): Promise<{ relinked: Relinked; health: LibraryHealth }> {
  if (useBrowserMocks()) {
    mockHealth = {
      ...mockHealth,
      missing: mockHealth.missing.filter((id) => id !== trackId),
      relocated: [...mockHealth.relocated, trackId],
    };
    return { relinked: { track_id: trackId, source_path: path }, health: mockHealth };
  }
  return invoke('locate_track_file', { trackId, path });
}

export function listenToLibraryHealth(callback: (health: LibraryHealth) => void) {
  return subscribe('library-health', callback);
}
