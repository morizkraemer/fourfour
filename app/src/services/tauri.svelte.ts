import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open as dialogOpen } from '@tauri-apps/plugin-dialog';

// Detect if running inside Tauri
export const isTauri = typeof window !== 'undefined' && (window as any).__TAURI_INTERNALS__ !== undefined;

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
  if (!isTauri) return '0.1.0-mock';
  return invoke<string>('app_version');
}

export async function getLibraryPath(): Promise<string> {
  if (!isTauri) return '/mock/user/library.db';
  return invoke<string>('get_library_path');
}

export async function pickDirectory(): Promise<string | null> {
  if (!isTauri) {
    const path = prompt('Enter mock directory path:');
    return path || null;
  }
  const result = await dialogOpen({ directory: true, multiple: false });
  return typeof result === 'string' ? result : Array.isArray(result) ? result[0] : null;
}

export async function changeLibraryPath(folderPath: string): Promise<string> {
  if (!isTauri) return folderPath + '/library.db';
  return invoke<string>('change_library_path', { folderPath });
}

export async function loadState(): Promise<LoadedState> {
  if (!isTauri) {
    return { tracks: mockTracks, playlists: mockPlaylists };
  }
  return invoke<LoadedState>('load_state');
}

export async function saveState(playlists: PlaylistInput[]): Promise<void> {
  if (!isTauri) {
    mockPlaylists = playlists;
    return;
  }
  return invoke<void>('save_state', { playlists });
}

export async function scanDirectory(path: string): Promise<TrackInfo[]> {
  if (!isTauri) {
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
  return invoke<TrackInfo[]>('scan_directory', { path });
}

export async function scanFiles(paths: string[]): Promise<TrackInfo[]> {
  if (!isTauri) {
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
  return invoke<TrackInfo[]>('scan_files', { paths });
}

export async function removeTracks(ids: number[]): Promise<void> {
  if (!isTauri) {
    mockTracks = mockTracks.filter(t => !ids.includes(t.id));
    mockPlaylists.forEach(pl => {
      pl.track_ids = pl.track_ids.filter(tid => !ids.includes(tid));
    });
    return;
  }
  return invoke<void>('remove_tracks', { ids });
}

export async function setTestCues(ids: number[]): Promise<TrackInfo[]> {
  if (!isTauri) {
    mockTracks.forEach(t => {
      if (ids.includes(t.id)) {
        t.has_cues = true;
      }
    });
    return mockTracks;
  }
  return invoke<TrackInfo[]>('set_test_cues', { ids });
}

export async function analyzeTracks(): Promise<TrackInfo[]> {
  if (!isTauri) {
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
  return invoke<TrackInfo[]>('analyze_tracks');
}

export async function getMountedVolumes(): Promise<string[]> {
  if (!isTauri) return mockVolumes;
  return invoke<string[]>('get_mounted_volumes');
}

export async function readUsbState(path: string): Promise<UsbStateResponse | null> {
  if (!isTauri) {
    return {
      tracks: [
        { id: 1, title: 'USB Track 1', artist: 'Artist A', album: 'Album A', genre: 'House', key: '5A', bpm: 124.0, duration: 400, usb_path: '/Contents/Artist A/Album A/01 Track 1.mp3' }
      ],
      playlists: [
        { id: 1, name: 'USB Playlist 1', track_count: 1 }
      ]
    };
  }
  return invoke<UsbStateResponse | null>('read_usb_state', { path });
}

export async function writeUsb(outputDir: string, playlists: PlaylistInput[]): Promise<SyncReport> {
  if (!isTauri) {
    return {
      tracks_added: 2,
      tracks_updated: 0,
      tracks_replaced: 0,
      tracks_removed: 0,
      tracks_unchanged: 1,
    };
  }
  return invoke<SyncReport>('write_usb', { outputDir, playlists });
}

export async function ejectVolume(path: string): Promise<void> {
  if (!isTauri) {
    mockVolumes = mockVolumes.filter(v => v !== path);
    return;
  }
  return invoke<void>('eject_volume', { path });
}

export async function wipeUsb(path: string): Promise<void> {
  if (!isTauri) return;
  return invoke<void>('wipe_usb', { path });
}

export async function getAnalysisData(trackId: number): Promise<any> {
  if (!isTauri) {
    return {
      waveform_preview: Array.from({ length: 400 }, () => Math.floor(Math.random() * 255)),
      waveform_color: Array.from({ length: 400 }, () => ({
        amp: Math.random(),
        r: Math.random(),
        g: Math.random(),
        b: Math.random(),
      })),
      bpm: 124.0,
      key: '9A',
      beats: [],
      cue_points: [],
      duration_ms: 300000
    };
  }
  return invoke<any>('get_analysis_data', { trackId });
}

export function listenToAnalysisProgress(callback: (payload: { current: number; total: number; message: string }) => void) {
  if (!isTauri) return () => {};
  let unsubscribe: () => void = () => {};
  listen<{ current: number; total: number; message: string }>('analysis-progress', (event) => {
    callback(event.payload);
  }).then(unsub => {
    unsubscribe = unsub;
  });
  return () => unsubscribe();
}

export function listenToWriteComplete(callback: (payload: SyncReport) => void) {
  if (!isTauri) return () => {};
  let unsubscribe: () => void = () => {};
  listen<SyncReport>('write-complete', (event) => {
    callback(event.payload);
  }).then(unsub => {
    unsubscribe = unsub;
  });
  return () => unsubscribe();
}
