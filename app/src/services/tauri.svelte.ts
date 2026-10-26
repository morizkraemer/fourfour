import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open as dialogOpen } from '@tauri-apps/plugin-dialog';
import {
  isValidAnalysisPayload,
  sanitizeAnalysisPayload,
} from './analysis-data.ts';
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
}

export interface PlaylistInput {
  id: number;
  name: string;
  track_ids: number[];
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
  { id: 1, source_path: '/Music/Solomun - Hyperion.mp3', title: 'Hyperion', artist: 'Solomun', album: 'Diynamic Hits', genre: 'Melodic House', tempo: 12400, key: '9A', duration_secs: 462, bitrate: 320, file_size: 18482010, has_artwork: true, has_cues: false },
  { id: 2, source_path: '/Music/Tale Of Us - Voidwalker.flac', title: 'Voidwalker', artist: 'Tale Of Us', album: 'Endless Run', genre: 'Melodic Techno', tempo: 12200, key: '7A', duration_secs: 494, bitrate: 1040, file_size: 64281920, has_artwork: true, has_cues: true },
  { id: 3, source_path: '/Music/Bicep - Drift.mp3', title: 'Drift', artist: 'Bicep', album: 'Drift EP', genre: 'Breakbeat', tempo: 12800, key: '5B', duration_secs: 415, bitrate: 320, file_size: 16601020, has_artwork: true, has_cues: false },
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
    has_artwork: false,
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
  { id: 1, name: 'Peak Time', track_ids: [1, 2] },
  { id: 2, name: 'Warmup', track_ids: [3] }
];

let mockVolumes = ['/Volumes/SanDisk 64GB', '/Volumes/VAULT666'];

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

export async function changeLibraryPath(folderPath: string): Promise<string> {
  try {
    return await invoke<string>('change_library_path', { folderPath });
  } catch (err) {
    lastTauriError = `change_library_path fail: ${String(err)}`;
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

export async function loadState(): Promise<LoadedState> {
  try {
    const res = await invoke<LoadedState>('load_state');
    lastTauriError = 'invoke(load_state) OK';
    return res;
  } catch (err) {
    lastTauriError = `load_state fail: ${String(err)}`;
    if (useBrowserMocks()) {
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

export async function scanDirectory(path: string): Promise<TrackInfo[]> {
  try {
    return await invoke<TrackInfo[]>('scan_directory', { path });
  } catch (err) {
    lastTauriError = `scan_directory fail: ${String(err)}`;
    if (useBrowserMocks()) {
      const newTrack: TrackInfo = {
        id: Date.now(),
        source_path: path + '/NewTrack.mp3',
        title: 'Scanned Track ' + (mockTracks.length + 1),
        artist: 'Unknown Artist',
        album: 'Scanned Album',
        genre: 'Techno',
        tempo: 12500,
        key: '1A',
        duration_secs: 360,
        bitrate: 320,
        file_size: 14400000,
        has_artwork: false,
        has_cues: false,
      };
      mockTracks.push(newTrack);
      return [newTrack];
    }
    throw err;
  }
}

export async function scanFiles(paths: string[]): Promise<TrackInfo[]> {
  try {
    return await invoke<TrackInfo[]>('scan_files', { paths });
  } catch (err) {
    lastTauriError = `scan_files fail: ${String(err)}`;
    if (useBrowserMocks()) {
      const newTracks = paths.map((p, i) => ({
        id: Date.now() + i,
        source_path: p,
        title: p.split('/').pop() || 'Dropped Track',
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
      }));
      mockTracks.push(...newTracks);
      return newTracks;
    }
    throw err;
  }
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

export async function analyzeTrackIds(ids: number[]): Promise<TrackInfo[]> {
  try {
    return await invoke<TrackInfo[]>('analyze_track_ids', { ids });
  } catch (err) {
    lastTauriError = `analyze_track_ids fail: ${String(err)}`;
    return new Promise((resolve) => {
      setTimeout(() => {
        mockTracks.forEach((t) => {
          if (ids.includes(t.id)) {
            t.tempo = 12600 + (t.id % 7) * 100;
            t.key = '3A';
            t.duration_secs = 380;
          }
        });
        resolve(mockTracks);
      }, 800);
    });
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

export async function writeUsb(outputDir: string, playlists: PlaylistInput[]): Promise<SyncReport> {
  try {
    return await invoke<SyncReport>('write_usb', { outputDir, playlists });
  } catch (err) {
    lastTauriError = `write_usb fail: ${String(err)}`;
    return {
      tracks_added: 2,
      tracks_updated: 0,
      tracks_replaced: 0,
      tracks_removed: 0,
      tracks_unchanged: 1,
    };
  }
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
      const clean = sanitizeAnalysisPayload(data);
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

const ARTWORK_THUMB_PX = 512;

export async function getTrackArtwork(trackId: number): Promise<ArrayBuffer | null> {
  try {
    const bytes = await invoke<ArrayBuffer>('get_track_artwork', { trackId, maxPx: ARTWORK_THUMB_PX });
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
  let unsubscribe: () => void = () => {};
  try {
    listen<{
      current: number;
      total: number;
      message: string;
      track_id?: number;
    }>('analysis-progress', (event) => {
      callback(event.payload);
    }).then(unsub => {
      unsubscribe = unsub;
    }).catch(err => {
      lastTauriError = `listen(analysis-progress) fail: ${String(err)}`;
    });
  } catch (err) {
    lastTauriError = `listen(analysis-progress) fail: ${String(err)}`;
  }
  return () => unsubscribe();
}

/** The waveform of a prioritised track is stored; tempo, grid and key follow with the full result. */
export function listenToAnalysisPartial(callback: (payload: { track_id: number }) => void) {
  let unsubscribe: () => void = () => {};
  try {
    listen<{ track_id: number }>('analysis-partial', (event) => {
      callback(event.payload);
    })
      .then((unsub) => {
        unsubscribe = unsub;
      })
      .catch((err) => {
        lastTauriError = `listen(analysis-partial) fail: ${String(err)}`;
      });
  } catch (err) {
    lastTauriError = `listen(analysis-partial) fail: ${String(err)}`;
  }
  return () => unsubscribe();
}

export function listenToAnalysisComplete(callback: () => void) {
  let unsubscribe: () => void = () => {};
  try {
    listen<void>('analysis-complete', () => {
      callback();
    })
      .then((unsub) => {
        unsubscribe = unsub;
      })
      .catch((err) => {
        lastTauriError = `listen(analysis-complete) fail: ${String(err)}`;
      });
  } catch (err) {
    lastTauriError = `listen(analysis-complete) fail: ${String(err)}`;
  }
  return () => unsubscribe();
}

export function listenToWriteComplete(callback: (payload: SyncReport) => void) {
  let unsubscribe: () => void = () => {};
  try {
    listen<SyncReport>('write-complete', (event) => {
      callback(event.payload);
    }).then(unsub => {
      unsubscribe = unsub;
    }).catch(err => {
      lastTauriError = `listen(write-complete) fail: ${String(err)}`;
    });
  } catch (err) {
    lastTauriError = `listen(write-complete) fail: ${String(err)}`;
  }
  return () => unsubscribe();
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
  remap_from?: string | null;
  remap_to?: string | null;
  include_missing: boolean;
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
  remap: { from_prefix: string; to_prefix: string } | null;
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

export async function rekordboxImportPreview(
  request: RekordboxImportRequest,
): Promise<RekordboxImportPreview> {
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
