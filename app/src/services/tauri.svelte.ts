import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open as dialogOpen } from '@tauri-apps/plugin-dialog';
import {
  isValidAnalysisPayload,
  sanitizeAnalysisPayload,
} from './analysis-data.ts';

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
  /** 400-byte mono preview embedded at load so list rows skip the N+1 fetch. */
  waveform_preview?: number[] | null;
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

const analysisCache = new Map<number, any>();

export function peekAnalysisData(trackId: number): any | undefined {
  const cached = analysisCache.get(trackId);
  if (!cached) return undefined;
  const clean = sanitizeAnalysisPayload(cached);
  if (!clean) {
    analysisCache.delete(trackId);
    return undefined;
  }
  return clean;
}

export function invalidateAnalysisCache(trackId?: number) {
  if (trackId == null) analysisCache.clear();
  else analysisCache.delete(trackId);
}

export async function getAnalysisData(trackId: number, force = false): Promise<any> {
  if (!force) {
    const hit = peekAnalysisData(trackId);
    if (hit) return hit;
  }
  try {
    const data = await invoke<any>('get_analysis_data', { trackId });
    const clean = sanitizeAnalysisPayload(data);
    if (!clean) {
      throw new Error('Invalid analysis payload from backend');
    }
    analysisCache.set(trackId, clean);
    return clean;
  } catch (err) {
    lastTauriError = `get_analysis_data fail: ${String(err)}`;
    analysisCache.delete(trackId);
    throw err;
  }
}

export async function getTrackArtwork(trackId: number): Promise<Uint8Array | null> {
  try {
    return await invoke<Uint8Array | null>('get_track_artwork', { trackId });
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
