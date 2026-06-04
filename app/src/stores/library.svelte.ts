/**
 * library.svelte.ts — Reactive library store utilizing Svelte 5 runes.
 * Interfaces with tauri.svelte.ts service layer and provides fallback mock data.
 */

import {
  isTauri,
  getVersion,
  getLibraryPath,
  loadState as tauriLoadState,
  saveState as tauriSaveState,
  scanDirectory as tauriScanDirectory,
  scanFiles as tauriScanFiles,
  removeTracks as tauriRemoveTracks,
  setTestCues as tauriSetTestCues,
  analyzeTracks as tauriAnalyzeTracks,
  getMountedVolumes as tauriGetMountedVolumes,
  readUsbState as tauriReadUsbState,
  writeUsb as tauriWriteUsb,
  ejectVolume as tauriEjectVolume,
  wipeUsb as tauriWipeUsb,
  listenToAnalysisProgress,
  listenToWriteComplete,
  type TrackInfo,
  type PlaylistInput,
  type SyncReport
} from '../services/tauri.svelte.ts';

/* ── Table column set (matches module-table.artboard.js) ────────────── */
export const TABLE_COLUMNS = [
  { key: 'fav', label: 'FAV', width: 20 },
  { key: 'index', label: '#', width: 24 },
  { key: 'wave', label: 'PREVIEW', width: 120 },
  { key: 'cover', label: '', width: 18 },
  { key: 'title', label: 'TITLE', flex: true },
  { key: 'artist', label: 'ARTIST', width: 180 },
  { key: 'label', label: 'LABEL', width: 130 },
  { key: 'bpm', label: 'BPM', width: 50, sort: 'desc' },
  { key: 'key', label: 'KEY', width: 36 },
  { key: 'time', label: 'TIME', width: 48 },
];

/* ── Main Library Store State ───────────────────────────────────────── */
export const library = $state({
  tracks: [] as any[],
  playlists: [] as PlaylistInput[],
  volumes: [] as string[],
  
  // Active states
  activeVolume: null as string | null,
  usbTracks: [] as any[],
  usbPlaylists: [] as any[],

  // App settings/status
  appVersion: '0.0.0',
  libraryPath: '',
  analyzing: false,
  syncing: false,
  statusMessage: 'Ready',
  analysisProgress: { current: 0, total: 0, message: '' },

  // Getters
  get trackCount() {
    return this.tracks.length;
  },

  get sidebarData() {
    return {
      favorites: {
        label: 'FAVORITES',
        rows: [
          { kind: 'favorite', digit: 1, label: 'Peak Time',  count: this.playlists.find(p => p.name === 'Peak Time')?.track_ids.length ?? 0 },
          { kind: 'favorite', digit: 2, label: 'Warmup',     count: this.playlists.find(p => p.name === 'Warmup')?.track_ids.length ?? 0 },
        ],
      },
      library: {
        label: 'LIBRARY',
        rows: [
          { kind: 'leaf', icon: 'list',     label: 'All Tracks',      count: this.tracks.length },
          { kind: 'leaf', icon: 'clock-3',  label: 'Recently Added',  count: this.tracks.filter(t => t.raw?.tempo > 0).length },
          { kind: 'leaf', icon: 'disc',     label: 'Unfiled',         count: this.tracks.filter(t => t.raw?.tempo === 0).length },
        ],
      },
      playlists: {
        label: 'PLAYLISTS',
        showAdd: true,
        rows: this.playlists.map(p => ({
          kind: 'playlist',
          label: p.name,
          count: p.track_ids.length,
          id: p.id
        })),
      },
      usb: {
        label: 'USB DEVICES',
        rows: this.volumes.map(v => ({
          kind: 'usb',
          label: v.replace(/\\/g, '/').split('/').pop() || v,
          status: this.activeVolume === v ? 'online' : 'offline',
          path: v
        })),
      },
    };
  }
});

/* ── Mapper ─────────────────────────────────────────────────────────── */
function mapTrackInfoToLocal(t: TrackInfo, idx: number) {
  const indexStr = String(idx + 1).padStart(2, '0');
  const bpmStr = t.tempo > 0 ? (t.tempo / 100).toFixed(1) : '—';
  const keyStr = t.key || '—';
  
  let timeStr = '—';
  if (t.duration_secs > 0) {
    const mins = Math.floor(t.duration_secs / 60);
    const secs = Math.floor(t.duration_secs % 60);
    timeStr = `${mins}:${String(secs).padStart(2, '0')}`;
  }

  return {
    id: t.id,
    index: indexStr,
    title: t.title || t.source_path.split(/[/\\]/).pop() || 'Unknown Title',
    artist: t.artist || '—',
    label: '—',
    bpm: bpmStr,
    key: keyStr,
    time: timeStr,
    fav: undefined,
    genre: t.genre || '—',
    year: undefined,
    filePath: t.source_path,
    cues: t.has_cues ? [{ name: 'Cues Injected', position: '0:00' }] : [],
    comment: '',
    raw: t
  };
}

/* ── Actions ────────────────────────────────────────────────────────── */

/** Loads full state from library backend / mock fallback. */
export async function loadState() {
  try {
    const state = await tauriLoadState();
    library.tracks = state.tracks.map((t, idx) => mapTrackInfoToLocal(t, idx));
    library.playlists = state.playlists;
  } catch (err) {
    console.error('loadState failed:', err);
  }
}

/** Saves playlists to backend. Tracks are already auto-persisted. */
export async function saveState() {
  try {
    await tauriSaveState(library.playlists);
  } catch (err) {
    console.error('saveState failed:', err);
  }
}

/** Initialize library store, triggers on boot */
export async function initLibrary() {
  // Get Version and Path
  getVersion().then(v => library.appVersion = v);
  getLibraryPath().then(p => library.libraryPath = p);

  // Load Tracks and Playlists
  await loadState();

  // Load Pinned USB Drives
  await refreshVolumes();

  // Wire event listeners for Tauri (will be no-op outside Tauri)
  listenToAnalysisProgress((progress) => {
    library.analysisProgress = progress;
    library.statusMessage = progress.message || `Analyzing (${progress.current}/${progress.total})`;
  });

  listenToWriteComplete((report) => {
    library.syncing = false;
    library.statusMessage = 'Ready';
    refreshUsbState();
  });
}

/** Scan directory for new tracks */
export async function importDirectory(path: string) {
  try {
    const newTracks = await tauriScanDirectory(path);
    if (newTracks.length > 0) {
      await loadState();
    }
  } catch (err) {
    console.error('importDirectory failed:', err);
  }
}

/** Scan dropped files/folders */
export async function importFiles(paths: string[]) {
  try {
    const newTracks = await tauriScanFiles(paths);
    if (newTracks.length > 0) {
      await loadState();
    }
  } catch (err) {
    console.error('importFiles failed:', err);
  }
}

/** Delete tracks from library */
export async function removeTracks(ids: number[]) {
  try {
    await tauriRemoveTracks(ids);
    library.playlists.forEach(pl => {
      pl.track_ids = pl.track_ids.filter(tid => !ids.includes(tid));
    });
    await loadState();
  } catch (err) {
    console.error('removeTracks failed:', err);
  }
}

/** Trigger analysis on all pending tracks */
export async function analyzeTracks() {
  if (library.analyzing) return;
  library.analyzing = true;
  library.statusMessage = 'Starting analysis...';
  try {
    await tauriAnalyzeTracks();
    await loadState();
  } catch (err) {
    console.error('analyzeTracks failed:', err);
  } finally {
    library.analyzing = false;
    library.statusMessage = 'Ready';
    library.analysisProgress = { current: 0, total: 0, message: '' };
  }
}

/** Set test hot cues on selected track IDs */
export async function setTestCuesOnSelected(ids: number[]) {
  try {
    await tauriSetTestCues(ids);
    await loadState();
  } catch (err) {
    console.error('setTestCuesOnSelected failed:', err);
  }
}

/* ── USB drive management ───────────────────────────────────────────── */

export async function refreshVolumes() {
  try {
    library.volumes = await tauriGetMountedVolumes();
    // Validate activeVolume still exists
    if (library.activeVolume && !library.volumes.includes(library.activeVolume)) {
      library.activeVolume = null;
      library.usbTracks = [];
      library.usbPlaylists = [];
    }
  } catch (err) {
    console.error('refreshVolumes failed:', err);
  }
}

export async function selectVolume(path: string | null) {
  library.activeVolume = path;
  await refreshUsbState();
}

export async function refreshUsbState() {
  if (!library.activeVolume) {
    library.usbTracks = [];
    library.usbPlaylists = [];
    return;
  }
  try {
    const res = await tauriReadUsbState(library.activeVolume);
    if (res) {
      library.usbTracks = res.tracks.map((t, idx) => ({
        id: t.id,
        index: String(idx + 1).padStart(2, '0'),
        title: t.title,
        artist: t.artist,
        album: t.album,
        label: '—',
        bpm: t.bpm > 0 ? t.bpm.toFixed(1) : '—',
        key: t.key || '—',
        time: t.duration > 0 ? `${Math.floor(t.duration / 60)}:${String(Math.floor(t.duration % 60)).padStart(2, '0')}` : '—',
        genre: t.genre || '—',
        filePath: t.usb_path
      }));
      library.usbPlaylists = res.playlists;
    } else {
      library.usbTracks = [];
      library.usbPlaylists = [];
    }
  } catch (err) {
    console.error('refreshUsbState failed:', err);
    library.usbTracks = [];
    library.usbPlaylists = [];
  }
}

export async function wipeVolume(path: string) {
  try {
    await tauriWipeUsb(path);
    await refreshUsbState();
  } catch (err) {
    console.error('wipeVolume failed:', err);
  }
}

export async function ejectVolume(path: string) {
  try {
    await tauriEjectVolume(path);
    await refreshVolumes();
  } catch (err) {
    console.error('ejectVolume failed:', err);
  }
}

export async function syncToUsb() {
  if (!library.activeVolume || library.syncing) return;
  library.syncing = true;
  library.statusMessage = 'Syncing tracks to USB...';
  try {
    await tauriWriteUsb(library.activeVolume, library.playlists);
    // Write complete event handles resetting state
  } catch (err) {
    console.error('syncToUsb failed:', err);
    library.syncing = false;
    library.statusMessage = 'Ready';
  }
}

/* ── Playlist management ────────────────────────────────────────────── */

export async function addPlaylist(name: string) {
  const newId = library.playlists.length > 0 ? Math.max(...library.playlists.map(p => p.id)) + 1 : 1;
  library.playlists.push({
    id: newId,
    name,
    track_ids: []
  });
  await saveState();
}

export async function deletePlaylist(id: number) {
  library.playlists = library.playlists.filter(p => p.id !== id);
  await saveState();
}

export async function addTracksToPlaylist(playlistId: number, trackIds: number[]) {
  const pl = library.playlists.find(p => p.id === playlistId);
  if (!pl) return;
  trackIds.forEach(tid => {
    if (!pl.track_ids.includes(tid)) {
      pl.track_ids.push(tid);
    }
  });
  await saveState();
}

export async function removeTrackFromPlaylist(playlistId: number, trackId: number) {
  const pl = library.playlists.find(p => p.id === playlistId);
  if (!pl) return;
  pl.track_ids = pl.track_ids.filter(tid => tid !== trackId);
  await saveState();
}

/* ── Change local library directory ─────────────────────────────────── */

export async function changeLocalLibraryPath(folderPath: string) {
  try {
    const dbPath = await changeLibraryPath(folderPath);
    library.libraryPath = dbPath;
    await loadState();
  } catch (err) {
    console.error('changeLocalLibraryPath failed:', err);
  }
}


