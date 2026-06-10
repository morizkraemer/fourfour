/**
 * library.svelte.ts — Reactive library store utilizing Svelte 5 runes.
 * Interfaces with tauri.svelte.ts service layer and provides fallback mock data.
 */

import {
  getVersion,
  getLibraryPath,
  loadState as tauriLoadState,
  saveState as tauriSaveState,
  scanDirectory as tauriScanDirectory,
  scanFiles as tauriScanFiles,
  removeTracks as tauriRemoveTracks,
  setTestCues as tauriSetTestCues,
  analyzeTracks as tauriAnalyzeTracks,
  analyzeTrackIds as tauriAnalyzeTrackIds,
  prioritizeAnalysis as tauriPrioritizeAnalysis,
  getMountedVolumes as tauriGetMountedVolumes,
  readUsbState as tauriReadUsbState,
  writeUsb as tauriWriteUsb,
  ejectVolume as tauriEjectVolume,
  wipeUsb as tauriWipeUsb,
  listenToAnalysisProgress,
  listenToAnalysisComplete,
  listenToWriteComplete,
  lastTauriError,
  getIpcDiagnostic,
  getAnalysisData,
  invalidateAnalysisCache,
  getTrackArtwork,
  changeLibraryPath,
  resetLibrary as tauriResetLibrary,
  type TrackInfo,
  type PlaylistInput,
  type SyncReport
} from '../services/tauri.svelte.ts';
import {
  isValidAnalysisPayload,
  isValidWaveformPreview,
  previewPeaksNormalized,
} from '../services/analysis-data.ts';

const peakLoadInflight = new Map<number, Promise<void>>();

/** Shallow-bump so list rows re-render when nested track fields change. */
export function bumpLibraryTracks() {
  library.tracks = [...library.tracks];
}

/** Numbered favorite slots (1–9), backed by `Favorites N` playlists. */
export const FAVORITE_SLOTS = 9;

/** Minimum favorite rows shown in the sidebar (empty placeholders included). */
export const FAVORITE_DEFAULT_VISIBLE = 2;

export function favoritePlaylistName(slot: number) {
  return `Favorites ${slot}`;
}

export function favoriteSlotFromName(name: string): number | null {
  const m = name.match(/^Favorites (\d+)$/);
  if (!m) return null;
  const slot = parseInt(m[1], 10);
  return slot >= 1 && slot <= FAVORITE_SLOTS ? slot : null;
}

export function isFavoritePlaylistName(name: string) {
  return favoriteSlotFromName(name) != null;
}

/** Lowest favorite slot a track belongs to (for the FAV column). */
export function syncTrackFavoriteFields() {
  const slotsByTrack = new Map<number, number[]>();
  for (const pl of library.playlists) {
    const slot = favoriteSlotFromName(pl.name);
    if (slot == null) continue;
    for (const tid of pl.track_ids) {
      const list = slotsByTrack.get(tid) ?? [];
      list.push(slot);
      slotsByTrack.set(tid, list);
    }
  }
  for (const track of library.tracks) {
    const slots = slotsByTrack.get(track.id);
    track.fav = slots?.length ? Math.min(...slots) : undefined;
  }
}

function pushFavoritePlaylist(slot: number) {
  const name = favoritePlaylistName(slot);
  const newId =
    library.playlists.length > 0 ? Math.max(...library.playlists.map((pl) => pl.id)) + 1 : 1;
  library.playlists.push({ id: newId, name, track_ids: [] });
}

/** Map slot number → favorite playlist (only existing playlists). */
export function favoritePlaylistsBySlot() {
  const bySlot = new Map<number, PlaylistInput>();
  for (const p of library.playlists) {
    const slot = favoriteSlotFromName(p.name);
    if (slot != null) bySlot.set(slot, p);
  }
  return bySlot;
}

/** Highest occupied favorite slot, or 0 when none exist. */
export function highestFavoriteSlot() {
  const slots = [...favoritePlaylistsBySlot().keys()];
  return slots.length ? Math.max(...slots) : 0;
}

/** Sidebar shows at least two rows; grows with favorites up to nine. */
export function favoriteSidebarDisplaySlots() {
  return Math.min(
    FAVORITE_SLOTS,
    Math.max(FAVORITE_DEFAULT_VISIBLE, highestFavoriteSlot())
  );
}

/** First slot without a playlist, or null when all nine exist. */
export function nextFavoriteSlot(): number | null {
  for (let slot = 1; slot <= FAVORITE_SLOTS; slot++) {
    if (!playlistByName(favoritePlaylistName(slot))) return slot;
  }
  return null;
}

export function canAddFavorite() {
  return nextFavoriteSlot() != null;
}

/** Slots 1–2 are permanent sidebar favorites; only 3+ can be removed. */
export function canRemoveFavoriteSlot(slot: number) {
  return slot > FAVORITE_DEFAULT_VISIBLE;
}

export function buildFavoriteSidebarRows() {
  const bySlot = favoritePlaylistsBySlot();
  const displaySlots = favoriteSidebarDisplaySlots();
  const rows = [];
  for (let slot = 1; slot <= displaySlots; slot++) {
    const p = bySlot.get(slot);
    rows.push({
      kind: 'favorite' as const,
      digit: slot,
      label: p?.name ?? favoritePlaylistName(slot),
      count: p?.track_ids.length ?? 0,
    });
  }
  return rows;
}

export async function ensureFavoriteSlots() {
  let changed = false;
  for (let slot = 3; slot <= FAVORITE_SLOTS; slot++) {
    const name = favoritePlaylistName(slot);
    const idx = library.playlists.findIndex((p) => p.name === name);
    if (idx < 0) continue;
    if (library.playlists[idx].track_ids.length === 0) {
      library.playlists.splice(idx, 1);
      changed = true;
    }
  }
  for (let slot = 1; slot <= FAVORITE_DEFAULT_VISIBLE; slot++) {
    const name = favoritePlaylistName(slot);
    if (library.playlists.some((p) => p.name === name)) continue;
    pushFavoritePlaylist(slot);
    changed = true;
  }
  if (changed) await saveState();
  syncTrackFavoriteFields();
}

/** Toggle membership in a favorite slot for each track (add / remove). */
export async function toggleFavoriteSlot(slot: number, trackIds: number[]) {
  if (slot < 1 || slot > FAVORITE_SLOTS || trackIds.length === 0) return;
  const pl = playlistByName(favoritePlaylistName(slot));
  if (!pl) return;
  const unique = [...new Set(trackIds)];
  for (const tid of unique) {
    const idx = pl.track_ids.indexOf(tid);
    if (idx >= 0) pl.track_ids.splice(idx, 1);
    else pl.track_ids.push(tid);
  }
  await saveState();
  syncTrackFavoriteFields();
}

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
  /** Track ids currently in an analysis batch (drives row + waveform placeholder animation). */
  analyzingTrackIds: [] as number[],
  syncing: false,
  statusMessage: 'Ready',
  analysisProgress: { current: 0, total: 0, message: '' },
  showAnalysisPrompt: false,
  /** Track IDs from the last import; used by the post-import analyze dialog. */
  pendingAnalyzeTrackIds: [] as number[],

  // Diagnostics
  tauriError: '',
  ipcDiagnostic: '',

  // Getters
  get trackCount() {
    return this.tracks.length;
  },

  get sidebarData() {
    return {
      favorites: {
        label: 'FAVORITES',
        rows: buildFavoriteSidebarRows(),
      },
      library: {
        label: 'LIBRARY',
        rows: [
          { kind: 'leaf', icon: 'list',     label: 'All Tracks',      count: this.tracks.length },
          { kind: 'leaf', icon: 'clock-3',  label: 'Recently Added',  count: this.tracks.filter(t => t.raw?.tempo > 0).length },
        ],
      },
      playlists: {
        label: 'PLAYLISTS',
        showAdd: true,
        rows: this.playlists
          .filter(p => !p.name.startsWith('Favorites '))
          .map(p => ({
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

/** Match tracks against a whitespace-separated query (all terms must match). */
export function filterTracks<T extends Record<string, unknown>>(
  tracks: T[],
  query: string
): T[] {
  const trimmed = query.trim().toLowerCase();
  if (!trimmed) return tracks;
  const terms = trimmed.split(/\s+/).filter(Boolean);
  return tracks.filter((track) => {
    const haystack = [
      track.title,
      track.artist,
      track.album,
      track.label,
      track.genre,
      track.key,
      track.bpm,
      track.time,
      track.filePath,
      track.index,
      track.raw?.title,
      track.raw?.artist,
      track.raw?.album,
      track.raw?.genre,
      track.raw?.key,
      track.raw?.source_path,
    ]
      .map((v) => String(v ?? '').toLowerCase())
      .join(' ');
    return terms.every((term) => haystack.includes(term));
  });
}

/** Tracks shown for a sidebar source (label, playlist name, or USB path). */
export function tracksForSidebarSource(source: string | null | undefined) {
  const active = source ?? 'All Tracks';
  if (!active || active === 'All Tracks') {
    return library.tracks;
  }
  if (active === 'Recently Added') {
    return library.tracks.filter((t) => t.raw?.tempo > 0);
  }
  if (library.volumes.includes(active)) {
    return library.usbTracks;
  }
  const playlist = library.playlists.find((p) => p.name === active);
  if (playlist) {
    const byId = new Map(library.tracks.map((t) => [t.id, t]));
    return playlist.track_ids.map((id) => byId.get(id)).filter(Boolean);
  }
  return library.tracks;
}

/** Human-readable title for a sidebar source id. */
export function sidebarSourceLabel(source: string) {
  if (!source || source === 'All Tracks') return 'All Tracks';
  if (library.volumes.includes(source)) {
    return source.replace(/\\/g, '/').split('/').pop() || source;
  }
  return source;
}

export function isTrackAnalyzing(trackId: number): boolean {
  return library.analyzingTrackIds.includes(trackId);
}

function unanalyzedIds(ids: number[]): number[] {
  return [...new Set(ids)].filter((id) => {
    if (id <= 0) return false;
    const track = library.tracks.find((t) => t.id === id);
    return track != null && !track.analyzed;
  });
}

/** Move a track to the front of the waiting analysis queue. */
function bumpAnalysisQueuePriority(trackId: number) {
  if (!library.analyzingTrackIds.includes(trackId)) return;
  library.analyzingTrackIds = [
    trackId,
    ...library.analyzingTrackIds.filter((id) => id !== trackId),
  ];
  void tauriPrioritizeAnalysis(trackId);
}

/**
 * Ensure a track is analyzed — used when loading into the player.
 * Starts a background run when idle; bumps queue priority when already queued.
 */
export function requestBackgroundAnalysis(trackId: number) {
  const track = library.tracks.find((t) => t.id === trackId);
  if (!track || track.analyzed) return;

  if (isTrackAnalyzing(trackId)) {
    bumpAnalysisQueuePriority(trackId);
    return;
  }

  void analyzeTrackIds([trackId]);
}

function trackMatchesAnalyzedFilename(track: { title?: string; filePath?: string }, fileName: string) {
  const base = track.filePath?.split(/[/\\]/).pop();
  return base === fileName || track.title === fileName;
}

function applyWaveformAndCuesFromAnalysis(
  track: Record<string, unknown>,
  res: {
    waveform_preview?: number[];
    cue_points?: Array<{ time_ms: number; hot_cue_number: number; color?: string }>;
    bpm?: number;
    key?: string;
  },
) {
  if (res.waveform_preview?.length) {
    track.peaks = previewPeaksNormalized(res.waveform_preview);
  }
  if (res.cue_points) {
    track.cues = res.cue_points.map((cue) => {
      const mins = Math.floor(cue.time_ms / 60000);
      const secs = Math.floor((cue.time_ms % 60000) / 1000);
      const posStr = `${mins}:${String(secs).padStart(2, '0')}`;
      return {
        name: cue.hot_cue_number === 0 ? 'Memory Cue' : `Hot Cue ${String.fromCharCode(64 + cue.hot_cue_number)}`,
        position: posStr,
        color: cue.color,
      };
    });
  }
  if (res.bpm) track.bpm = Number(res.bpm).toFixed(1);
  if (res.key) track.key = res.key;
  track.analyzed = true;
  track.analysisLoaded = true;
}

/**
 * Fetch the full analysis (cues + peaks) for a track once — used by the Detail
 * pane. Mini-waveform peaks already arrive embedded in `load_state`, so this
 * gates on `analysisLoaded` (cues) rather than peak presence; otherwise an
 * embedded preview would suppress cue loading entirely.
 */
export async function ensureTrackPeaksLoaded(track: Record<string, unknown>): Promise<void> {
  if (track.analysisLoaded || !track.analyzed) return;
  const id = track.id as number;
  const inflight = peakLoadInflight.get(id);
  if (inflight) return inflight;

  const job = (async () => {
    try {
      const res = await getAnalysisData(id);
      if (!isValidAnalysisPayload(res)) {
        track.analyzed = false;
        track.analysisLoaded = false;
        bumpLibraryTracks();
        return;
      }
      applyWaveformAndCuesFromAnalysis(track, res);
      bumpLibraryTracks();
    } catch (err) {
      console.warn('ensureTrackPeaksLoaded failed:', err);
      track.analyzed = false;
      track.analysisLoaded = false;
      bumpLibraryTracks();
    } finally {
      peakLoadInflight.delete(id);
    }
  })();
  peakLoadInflight.set(id, job);
  return job;
}

async function refreshTrackAnalysisDisplay(track: Record<string, unknown>, attempt = 0) {
  const id = track.id as number;
  invalidateAnalysisCache(id);
  try {
    const res = await getAnalysisData(id, true);
    if (!isValidAnalysisPayload(res)) {
      track.analyzed = false;
      track.analysisLoaded = false;
      bumpLibraryTracks();
      return;
    }
    applyWaveformAndCuesFromAnalysis(track, res);
    bumpLibraryTracks();
  } catch (err) {
    if (attempt < 4) {
      await new Promise((r) => setTimeout(r, 120 * (attempt + 1)));
      return refreshTrackAnalysisDisplay(track, attempt + 1);
    }
    console.warn('Failed to refresh analysis display:', err);
    track.analyzed = false;
    track.analysisLoaded = false;
    bumpLibraryTracks();
  }
}

function finishAnalyzingTrack(trackId: number) {
  library.analyzingTrackIds = library.analyzingTrackIds.filter((id) => id !== trackId);
  const track = library.tracks.find((t) => t.id === trackId);
  if (track) void refreshTrackAnalysisDisplay(track);
}

function markTracksPendingAnalysis(ids: number[]) {
  for (const id of ids) {
    const track = library.tracks.find((t) => t.id === id);
    if (!track) continue;
    track.peaks = undefined;
    track.analysisLoaded = false;
  }
}

function handleAnalysisProgressEvent(progress: {
  current: number;
  total: number;
  message: string;
  track_id?: number;
}) {
  library.analyzing = true;
  library.analysisProgress = progress;
  library.statusMessage = progress.message || `Analyzing (${progress.current}/${progress.total})`;

  if (progress.track_id) {
    finishAnalyzingTrack(progress.track_id);
    return;
  }

  const match = progress.message.match(/^(?:Analyzed|Failed) (.+?) \(\d+\/\d+\)$/);
  if (!match) return;
  const fileName = match[1];
  const track = library.tracks.find((t) => trackMatchesAnalyzedFilename(t, fileName));
  if (track) finishAnalyzingTrack(track.id);
}

function handleAnalysisCompleteEvent() {
  library.analyzing = false;
  library.analyzingTrackIds = [];
  library.analysisProgress = { current: 0, total: 0, message: '' };
  if (!library.syncing) {
    library.statusMessage = 'Ready';
  }
}

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

  // Prefer the authoritative DB flag; fall back to tempo for browser mocks.
  const analyzed = t.has_analysis ?? t.tempo > 0;

  const local: Record<string, any> = {
    id: t.id,
    index: indexStr,
    title: t.title || t.source_path.split(/[/\\]/).pop() || 'Unknown Title',
    artist: t.artist || '—',
    album: t.album || '—',
    label: '—',
    bpm: bpmStr,
    key: keyStr,
    time: timeStr,
    fav: undefined,
    genre: t.genre || '—',
    year: '—',
    filePath: t.source_path,
    cues: t.has_cues ? [{ name: 'Cues Injected', position: '0:00' }] : [],
    comment: '',
    analyzed,
    raw: t
  };

  // Embedded mono preview → paint the mini waveform immediately, no N+1 fetch.
  if (t.waveform_preview && isValidWaveformPreview(t.waveform_preview)) {
    local.peaks = previewPeaksNormalized(t.waveform_preview);
  }

  return local;
}

/* ── Actions ────────────────────────────────────────────────────────── */

function revokeTrackCoverUrls(tracks: { cover?: string }[]) {
  for (const t of tracks) {
    if (t.cover?.startsWith('blob:')) URL.revokeObjectURL(t.cover);
  }
}

/** Loads full state from library backend / mock fallback. */
export async function loadState() {
  try {
    revokeTrackCoverUrls(library.tracks);
    invalidateAnalysisCache();
    const state = await tauriLoadState();
    library.tracks = state.tracks.map((t, idx) => mapTrackInfoToLocal(t, idx));
    library.playlists = state.playlists;

    // Load artwork in the background. Mini-waveform peaks now arrive embedded in
    // load_state (see mapTrackInfoToLocal), so no per-track analysis fetch here —
    // cues load lazily when a row is selected (Detail pane → ensureTrackPeaksLoaded).
    library.tracks.forEach(track => {
      if (track.raw?.has_artwork && !track.cover) {
        getTrackArtwork(track.id).then(bytes => {
          if (bytes && bytes.length > 0) {
            const blob = new Blob([new Uint8Array(bytes)], { type: 'image/jpeg' });
            track.cover = URL.createObjectURL(blob);
          }
        }).catch(err => {
          console.warn('Failed to fetch artwork on load:', err);
        });
      }
    });

    await ensureFavoriteSlots();
    library.tauriError = lastTauriError;
  } catch (err) {
    console.error('loadState failed:', err);
    library.tauriError = lastTauriError;
    throw err;
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

let libraryListenersInstalled = false;

/** Initialize library store, triggers on boot */
export async function initLibrary() {
  library.ipcDiagnostic = getIpcDiagnostic();
  library.statusMessage = 'Loading library…';

  try {
    const [version, path] = await Promise.all([getVersion(), getLibraryPath()]);
    library.appVersion = version;
    library.libraryPath = path;

    await loadState();
    await refreshVolumes();

    if (!libraryListenersInstalled) {
      libraryListenersInstalled = true;
      listenToAnalysisProgress(handleAnalysisProgressEvent);
      listenToAnalysisComplete(handleAnalysisCompleteEvent);
      listenToWriteComplete(() => {
        library.syncing = false;
        library.statusMessage = 'Ready';
        refreshUsbState();
      });
    }
  } catch (err) {
    library.statusMessage = 'Failed to load library';
    console.error('initLibrary failed:', err);
  } finally {
    if (!library.analyzing && !library.syncing) {
      library.statusMessage = library.statusMessage.startsWith('Failed')
        ? library.statusMessage
        : 'Ready';
    }
  }
}

export type ImportOptions = {
  /** Sidebar source (playlist name, etc.) to add tracks into after scan. */
  targetSource?: string;
  /** Analyze immediately instead of showing the post-import prompt. */
  analyze?: boolean;
};

/** Playlist for a sidebar source, or null for library-wide / non-importable views. */
export function playlistForSidebarSource(source: string | null | undefined) {
  const active = source ?? 'All Tracks';
  if (
    !active ||
    active === 'All Tracks' ||
    active === 'Recently Added' ||
    library.volumes.includes(active)
  ) {
    return null;
  }
  return library.playlists.find((p) => p.name === active) ?? null;
}

function importErrorMessage(err: unknown): string {
  const detail = err instanceof Error ? err.message : String(err);
  return detail ? `Import failed: ${detail}` : 'Import failed';
}

async function finishImport(newTracks: TrackInfo[], options?: ImportOptions) {
  if (newTracks.length === 0) {
    library.statusMessage = 'No new audio files found (already in library or unsupported format).';
    return;
  }

  const playlist = playlistForSidebarSource(options?.targetSource);
  if (playlist) {
    const ids = newTracks.map((t) => t.id);
    await addTracksToPlaylist(playlist.id, ids);
  }

  await loadState();

  if (options?.analyze) {
    const ids = newTracks.map((t) => t.id);
    if (ids.length > 0) await analyzeTrackIds(ids);
  } else {
    library.pendingAnalyzeTrackIds = newTracks.map((t) => t.id);
    library.showAnalysisPrompt = true;
  }
}

/** Close the post-import analyze prompt without running analysis. */
export function dismissAnalysisPrompt() {
  library.showAnalysisPrompt = false;
  library.pendingAnalyzeTrackIds = [];
}

/** Run analysis for tracks pending from the last import, then close the prompt. */
export async function confirmAnalysisPrompt() {
  const ids = [...library.pendingAnalyzeTrackIds];
  dismissAnalysisPrompt();
  if (ids.length > 0) await analyzeTrackIds(ids);
}

/** Scan directory for new tracks */
export async function importDirectory(path: string, options?: ImportOptions) {
  try {
    library.statusMessage = 'Importing…';
    const newTracks = await tauriScanDirectory(path);
    await finishImport(newTracks, options);
    library.tauriError = lastTauriError;
    if (newTracks.length > 0) {
      library.statusMessage = `Imported ${newTracks.length} track${newTracks.length === 1 ? '' : 's'}`;
    }
  } catch (err) {
    library.statusMessage = importErrorMessage(err);
    library.tauriError = lastTauriError;
    console.error('importDirectory failed:', err);
  }
}

/** Scan dropped files/folders */
export async function importFiles(paths: string[], options?: ImportOptions) {
  try {
    library.statusMessage = 'Importing…';
    const newTracks = await tauriScanFiles(paths);
    await finishImport(newTracks, options);
    library.tauriError = lastTauriError;
    if (newTracks.length > 0) {
      library.statusMessage = `Imported ${newTracks.length} track${newTracks.length === 1 ? '' : 's'}`;
    }
  } catch (err) {
    library.statusMessage = importErrorMessage(err);
    library.tauriError = lastTauriError;
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
  const pendingIds = library.tracks.filter((t) => !t.analyzed).map((t) => t.id);
  if (pendingIds.length === 0) return;

  markTracksPendingAnalysis(pendingIds);
  library.analyzing = true;
  library.analyzingTrackIds = [...new Set([...library.analyzingTrackIds, ...pendingIds])];
  library.statusMessage = 'Starting analysis...';
  try {
    await tauriAnalyzeTracks();
  } catch (err) {
    console.error('analyzeTracks failed:', err);
    if (!library.syncing) library.statusMessage = 'Analysis failed';
  }
}

/** Analyze or re-analyze specific tracks by id */
export async function analyzeTrackIds(ids: number[]) {
  const unique = unanalyzedIds(ids);
  if (unique.length === 0) return;

  markTracksPendingAnalysis(unique);
  library.analyzing = true;
  library.analyzingTrackIds = [...new Set([...library.analyzingTrackIds, ...unique])];
  library.statusMessage =
    unique.length === 1 ? 'Analyzing track…' : `Analyzing ${unique.length} tracks…`;
  try {
    await tauriAnalyzeTrackIds(unique);
  } catch (err) {
    console.error('analyzeTrackIds failed:', err);
    if (!library.syncing) library.statusMessage = 'Analysis failed';
  }
}

/** Remove tracks from a playlist without deleting library entries */
export async function removeTracksFromPlaylist(playlistId: number, trackIds: number[]) {
  const pl = library.playlists.find((p) => p.id === playlistId);
  if (!pl || trackIds.length === 0) return;
  const drop = new Set(trackIds);
  pl.track_ids = pl.track_ids.filter((id) => !drop.has(id));
  await saveState();
  if (isFavoritePlaylistName(pl.name)) syncTrackFavoriteFields();
}

/** Remove every track from a playlist (favorites / curate lists). */
export async function clearPlaylistTracks(playlistId: number) {
  const pl = library.playlists.find((p) => p.id === playlistId);
  if (!pl || pl.track_ids.length === 0) return;
  pl.track_ids = [];
  await saveState();
  if (isFavoritePlaylistName(pl.name)) syncTrackFavoriteFields();
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

/**
 * Export tracks to a specific USB volume by triggering a full sync to that
 * device. Note: the `write_usb` backend command is a holistic sync — it
 * exports ALL library tracks + playlists to the target volume, not just the
 * dragged subset. Per-track selective export is not yet supported by the
 * backend. The `trackIds` parameter is reserved for future per-track export
 * support and logged here for traceability.
 */
export async function exportTracksToVolume(trackIds: number[], volumePath: string) {
  if (!library.volumes.includes(volumePath)) {
    console.error(
      `[organize] exportTracksToVolume: volume "${volumePath}" is not mounted. Aborting.`
    );
    return;
  }
  if (library.syncing) {
    console.error('[organize] exportTracksToVolume: a sync is already in progress. Aborting.');
    return;
  }
  // NOTE: write_usb is a full sync — trackIds logged for future selective-export support.
  console.log(`[organize] Exporting ${trackIds.length} dragged track(s) to ${volumePath} (full sync)`);
  library.syncing = true;
  library.statusMessage = 'Exporting to USB…';
  try {
    await tauriWriteUsb(volumePath, library.playlists);
    // write-complete event from backend resets syncing state via listenToWriteComplete
  } catch (err) {
    console.error('[organize] exportTracksToVolume failed:', err);
    library.syncing = false;
    library.statusMessage = 'Ready';
  }
}

/* ── Playlist management ────────────────────────────────────────────── */

/** Default name for a new user playlist (excludes favorite slots). */
export function defaultPlaylistName(): string {
  const userPlaylists = library.playlists.filter((p) => !p.name.startsWith('Favorites '));
  const used = new Set(userPlaylists.map((p) => p.name));
  let n = userPlaylists.length + 1;
  while (used.has(`Playlist ${n}`)) n += 1;
  return `Playlist ${n}`;
}

export async function renamePlaylist(id: number, name: string) {
  const pl = library.playlists.find((p) => p.id === id);
  if (!pl) return;
  const trimmed = name.trim();
  if (!trimmed) return;
  pl.name = trimmed;
  await saveState();
}

export async function addPlaylist(name: string): Promise<number> {
  const newId = library.playlists.length > 0 ? Math.max(...library.playlists.map(p => p.id)) + 1 : 1;
  library.playlists.push({
    id: newId,
    name,
    track_ids: []
  });
  await saveState();
  return newId;
}

export async function deletePlaylist(id: number) {
  library.playlists = library.playlists.filter(p => p.id !== id);
  await saveState();
}

/** Delete a numbered favorite slot playlist (slots 3+ only); returns removed playlist name. */
export async function removeFavoriteSlot(playlistId: number): Promise<string | null> {
  const pl = library.playlists.find((p) => p.id === playlistId);
  if (!pl || !isFavoritePlaylistName(pl.name)) return null;
  const slot = favoriteSlotFromName(pl.name);
  if (slot == null || !canRemoveFavoriteSlot(slot)) return null;
  const name = pl.name;
  library.playlists = library.playlists.filter((p) => p.id !== playlistId);
  let changed = true;
  for (let s = 1; s <= FAVORITE_DEFAULT_VISIBLE; s++) {
    const defaultName = favoritePlaylistName(s);
    if (library.playlists.some((p) => p.name === defaultName)) continue;
    pushFavoritePlaylist(s);
    changed = true;
  }
  if (changed) await saveState();
  syncTrackFavoriteFields();
  return name;
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
  if (isFavoritePlaylistName(pl.name)) syncTrackFavoriteFields();
}

export async function removeTrackFromPlaylist(playlistId: number, trackId: number) {
  const pl = library.playlists.find(p => p.id === playlistId);
  if (!pl) return;
  pl.track_ids = pl.track_ids.filter(tid => tid !== trackId);
  await saveState();
  if (isFavoritePlaylistName(pl.name)) syncTrackFavoriteFields();
}

export async function reorderPlaylistTracks(playlistId: number, orderedIds: number[]) {
  const pl = library.playlists.find(p => p.id === playlistId);
  if (!pl) return;
  pl.track_ids = orderedIds;
  await saveState();
}

/**
 * Move or copy tracks into a playlist. When removeFromSource is true and
 * sourcePlaylistId is set, tracks are removed from the source playlist.
 */
export async function moveTracksToPlaylist(opts: {
  trackIds: number[];
  toPlaylistId: number;
  sourcePlaylistId?: number | null;
  insertIndex?: number;
  removeFromSource?: boolean;
}) {
  const {
    trackIds,
    toPlaylistId,
    sourcePlaylistId = null,
    insertIndex,
    removeFromSource = true,
  } = opts;
  const target = library.playlists.find(p => p.id === toPlaylistId);
  if (!target || trackIds.length === 0) return;

  const unique = [...new Set(trackIds)];
  if (removeFromSource && sourcePlaylistId != null && sourcePlaylistId !== toPlaylistId) {
    const source = library.playlists.find(p => p.id === sourcePlaylistId);
    if (source) {
      source.track_ids = source.track_ids.filter(id => !unique.includes(id));
    }
  }

  const withoutDupes = target.track_ids.filter(id => !unique.includes(id));
  const idx =
    insertIndex != null && insertIndex >= 0
      ? Math.min(insertIndex, withoutDupes.length)
      : withoutDupes.length;
  target.track_ids = [
    ...withoutDupes.slice(0, idx),
    ...unique,
    ...withoutDupes.slice(idx),
  ];
  await saveState();
  if (
    isFavoritePlaylistName(target.name) ||
    (sourcePlaylistId != null &&
      isFavoritePlaylistName(library.playlists.find((p) => p.id === sourcePlaylistId)?.name ?? ''))
  ) {
    syncTrackFavoriteFields();
  }
}

export async function reorderPlaylists(dragId: number, dropId: number) {
  const from = library.playlists.findIndex(p => p.id === dragId);
  const to = library.playlists.findIndex(p => p.id === dropId);
  if (from < 0 || to < 0 || from === to) return;
  const [item] = library.playlists.splice(from, 1);
  library.playlists.splice(to, 0, item);
  library.playlists = [...library.playlists];
  await saveState();
}

/** Resolve playlist by sidebar label (name). */
export function playlistByName(name: string): PlaylistInput | undefined {
  return library.playlists.find(p => p.name === name);
}

/* ── Change local library directory ─────────────────────────────────── */

export async function changeLocalLibraryPath(folderPath: string) {
  try {
    library.statusMessage = 'Opening library…';
    const dbPath = await changeLibraryPath(folderPath);
    library.libraryPath = dbPath;
    dismissAnalysisPrompt();
    await loadState();
    library.statusMessage = 'Ready';
  } catch (err) {
    library.statusMessage = 'Failed to open library';
    console.error('changeLocalLibraryPath failed:', err);
    throw err;
  }
}

/**
 * Wipe the configured library database and reload empty state (debug).
 * Clears UI selection/playback; keeps the library path in config.
 */
export async function resetLibraryDatabase() {
  const { clear: clearSelection } = await import('./selection.svelte.ts');
  const { clearPlayerTrack } = await import('./player.svelte.ts');
  const { playbackStop } = await import('../services/playback.svelte.ts');

  try {
    library.statusMessage = 'Resetting library…';
    dismissAnalysisPrompt();
    await playbackStop();
    clearPlayerTrack();
    clearSelection();

    const dbPath = await tauriResetLibrary();
    library.libraryPath = dbPath;
    library.usbTracks = [];
    library.usbPlaylists = [];
    library.activeVolume = null;

    await loadState();
    library.statusMessage = 'Library reset — empty';
  } catch (err) {
    library.statusMessage = 'Library reset failed';
    console.error('resetLibraryDatabase failed:', err);
    throw err;
  }
}


