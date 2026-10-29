/**
 * library.svelte.ts — Reactive library store utilizing Svelte 5 runes.
 * Interfaces with tauri.svelte.ts service layer and provides fallback mock data.
 */

import {
  getVersion,
  getLibraryPath,
  loadState as tauriLoadState,
  resumeAnalysis as tauriResumeAnalysis,
  saveState as tauriSaveState,
  planImport,
  commitImport,
  removeTracks as tauriRemoveTracks,
  setTestCues as tauriSetTestCues,
  analyzeTracks as tauriAnalyzeTracks,
  analyzeTrackIds as tauriAnalyzeTrackIds,
  prioritizeAnalysis as tauriPrioritizeAnalysis,
  getMountedVolumes as tauriGetMountedVolumes,
  readUsbState as tauriReadUsbState,
  ejectVolume as tauriEjectVolume,
  revealInFileManager,
  wipeUsb as tauriWipeUsb,
  listenToAnalysisProgress,
  listenToAnalysisPartial,
  listenToAnalysisComplete,
  listenToWriteComplete,
  lastTauriError,
  getIpcDiagnostic,
  getAnalysisData,
  invalidateAnalysisCache,
  changeLibraryPath,
  resetLibrary as tauriResetLibrary,
  type TrackInfo,
  type PlaylistInput,
  type SyncReport
} from '../services/tauri.svelte.ts';
import { loadSticks } from './sticks.svelte.ts';
import { onStickSyncFinished, stickSync, syncStick } from './stick-sync.svelte.ts';
import {
  isValidAnalysisPayload,
  isValidWaveformPreview,
  decodeTrackPreview,
  decodeBandPreview,
  readyAnalysisFields,
} from '../services/analysis-data.ts';
import { rowBandsFor, rowBandsFromBandBytes } from '../services/waveform-bands.ts';
import { artwork } from './artwork.svelte.ts';
import { loadCuesFromPayload } from './cues.svelte.ts';
import type { CuePointPayload } from '../lib/cues.ts';
import {
  descendantIds,
  movePlaylist,
  removeWithDescendants,
  sidebarTreeRows,
  uniqueName,
} from '../lib/playlistTree.ts';
import { trackIdsUnderPaths } from '../lib/fileDrop.ts';
import { importWithDuplicateCheck, type ImportOutcome } from '../lib/importDuplicates.ts';
import { completeImportActivity, failImportActivity, startImportActivity } from './import-activity.svelte.ts';
import { askAboutDuplicates } from './import-duplicates.svelte.ts';
import { outermostIds } from '../lib/playlistSelection.ts';
import { expandFolder, folderState } from './sidebar-folders.svelte.ts';
import { fileStateOf } from './file-status.svelte.ts';
import { fileStateGlyph } from '../lib/fileHealth.ts';

const peakLoadInflight = new Map<number, Promise<void>>();

/** Shallow-bump so list rows re-render when nested track fields change. */
export function bumpLibraryTracks() {
  resetSearchIndex();
  library.tracks = [...library.tracks];
  scheduleSearchIndexWarmup();
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
  library.playlists.push({ id: newId, name, track_ids: [], parent_id: 0, is_folder: false });
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
let recentlyAddedCount = 0;

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
          { kind: 'leaf', icon: 'clock-3',  label: 'Recently Added',  count: recentlyAddedCount },
        ],
      },
      playlists: {
        label: 'PLAYLISTS',
        showAdd: true,
        rows: sidebarTreeRows(
          this.playlists,
          new Set(folderState.collapsed),
          (p) => p.name.startsWith('Favorites ')
        ).map((row) => ({
          kind: row.isFolder ? 'folder' : 'playlist',
          label: row.name,
          count: row.isFolder ? undefined : row.trackCount,
          id: row.id,
          depth: row.depth,
          expanded: row.isFolder ? row.expanded : undefined,
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

const haystackCache = new WeakMap<object, string[]>();

/**
 * Library tracks as plain objects (`library.tracks` wraps these in reactive
 * proxies). Reading text through the proxies costs ~1 s at 30k tracks; the
 * plain objects see the same mutations without that cost.
 */
let plainTracks: Record<string, unknown>[] = [];
let haystackById: Map<number, string> | null = null;
let haystackCursor = 0;
let warmupTimer: ReturnType<typeof setTimeout> | undefined;
let warmupGeneration = 0;

const WARMUP_CHUNK = 2000;
const WARMUP_DELAY_MS = 250;

function resetSearchIndex() {
  haystackById = null;
  haystackCursor = 0;
  warmupGeneration++;
}

function fillSearchIndex(limit: number) {
  if (!haystackById) haystackById = new Map();
  const end = Math.min(plainTracks.length, haystackCursor + limit);
  for (; haystackCursor < end; haystackCursor++) {
    const t = plainTracks[haystackCursor];
    haystackById.set(t.id as number, trackHaystack(t));
  }
}

function haystackByIdFor(): Map<number, string> {
  fillSearchIndex(plainTracks.length);
  return haystackById!;
}

function whenIdle(run: () => void) {
  if (typeof requestIdleCallback === 'function') requestIdleCallback(run, { timeout: 2000 });
  else setTimeout(run, 0);
}

/** Builds the search text in idle-time chunks so the first keystroke finds it ready. */
function scheduleSearchIndexWarmup() {
  clearTimeout(warmupTimer);
  warmupTimer = setTimeout(() => {
    const generation = warmupGeneration;
    const step = () => {
      if (generation !== warmupGeneration) return;
      if (haystackCursor < plainTracks.length) {
        fillSearchIndex(WARMUP_CHUNK);
        whenIdle(step);
      } else {
        haystacksFor(library.tracks);
      }
    };
    whenIdle(step);
  }, WARMUP_DELAY_MS);
}

function trackHaystack(track: Record<string, unknown>): string {
  const raw = track.raw as Record<string, unknown> | undefined;
  return [
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
    raw?.title,
    raw?.artist,
    raw?.album,
    raw?.genre,
    raw?.key,
    raw?.source_path,
  ]
    .map((v) => String(v ?? '').toLowerCase())
    .join(' ');
}

/**
 * Search text per track, cached per source array. Track text only changes
 * together with `bumpLibraryTracks()` / `loadState()`, which swap the array.
 */
function haystacksFor<T extends Record<string, unknown>>(tracks: T[]): string[] {
  let haystacks = haystackCache.get(tracks);
  if (!haystacks) {
    const byId = haystackByIdFor();
    haystacks = tracks.map((t) => byId.get(t.id as number) ?? trackHaystack(t));
    haystackCache.set(tracks, haystacks);
  }
  return haystacks;
}

let lastFilter: { tracks: object; query: string; terms: string[]; indices: number[]; result: unknown[] } | null = null;

function narrowsLastFilter(terms: string[]): boolean {
  return !!lastFilter && lastFilter.terms.every((old) => terms.some((term) => term.includes(old)));
}

/** Match tracks against a whitespace-separated query (all terms must match). */
export function filterTracks<T extends Record<string, unknown>>(
  tracks: T[],
  query: string
): T[] {
  const trimmed = query.trim().toLowerCase();
  if (!trimmed) return tracks;
  if (lastFilter && lastFilter.tracks === tracks && lastFilter.query === trimmed) {
    return lastFilter.result as T[];
  }
  const terms = trimmed.split(/\s+/).filter(Boolean);
  const haystacks = haystacksFor(tracks);
  const candidates = lastFilter && lastFilter.tracks === tracks && narrowsLastFilter(terms) ? lastFilter.indices : null;
  const indices: number[] = [];
  const matches = (i: number) => terms.every((term) => haystacks[i].includes(term));
  if (candidates) {
    for (const i of candidates) if (matches(i)) indices.push(i);
  } else {
    for (let i = 0; i < haystacks.length; i++) if (matches(i)) indices.push(i);
  }
  const result = indices.length === tracks.length ? tracks : indices.map((i) => tracks[i]);
  lastFilter = { tracks, query: trimmed, terms, indices, result };
  return result;
}

let indexCache: { source: object; length: number; byId: Map<number, number> } | null = null;

function libraryTrackById(id: number) {
  const tracks = library.tracks;
  const ids = plainTracks.length === tracks.length ? plainTracks : tracks;
  if (!indexCache || indexCache.source !== ids || indexCache.length !== ids.length) {
    indexCache = { source: ids, length: ids.length, byId: new Map(ids.map((t, i) => [t.id as number, i])) };
  }
  const track = tracks[indexCache.byId.get(id) ?? -1];
  return track?.id === id ? track : tracks.find((t) => t.id === id);
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
  const playlist = library.playlists.find((p) => p.name === active && !p.is_folder);
  if (playlist) {
    return playlist.track_ids.map(libraryTrackById).filter(Boolean);
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

function knownTrackIds(ids: number[]): number[] {
  return [...new Set(ids)].filter((id) => id > 0 && library.tracks.some((t) => t.id === id));
}

const analysisStartListeners = new Set<(trackIds: number[]) => void>();

/** Called with the ids of tracks whose analysis (re)starts, after their rows were cleared. */
export function onAnalysisStart(listener: (trackIds: number[]) => void): () => void {
  analysisStartListeners.add(listener);
  return () => analysisStartListeners.delete(listener);
}

/**
 * Analyse a track now — used when it is loaded into the player or played.
 * The backend moves it to its fast lane (queued or not) or starts a run when idle;
 * the waveform arrives first as `analysis-partial`, the full result as `analysis-progress`.
 */
export function requestBackgroundAnalysis(trackId: number) {
  const track = library.tracks.find((t) => t.id === trackId);
  if (!track || (track.analyzed && !track.analysisPending)) return;

  if (isTrackAnalyzing(trackId)) {
    library.analyzingTrackIds = [trackId, ...library.analyzingTrackIds.filter((id) => id !== trackId)];
  } else {
    markTracksPendingAnalysis([trackId]);
    library.analyzing = true;
    library.analyzingTrackIds = [...library.analyzingTrackIds, trackId];
    library.statusMessage = 'Analyzing track…';
  }
  void tauriPrioritizeAnalysis(trackId);
}

function trackMatchesAnalyzedFilename(track: { title?: string; filePath?: string }, fileName: string) {
  const base = track.filePath?.split(/[/\\]/).pop();
  return base === fileName || track.title === fileName;
}

/** The row preview a track already carries is the one the analysis sends: keep the object so its bands and bitmaps stay cached. */
function sameBytes(current: unknown, next: ArrayLike<number>): boolean {
  if (!current || typeof current !== 'object') return false;
  const bytes = current as ArrayLike<number>;
  if (bytes.length !== next.length) return false;
  for (let i = 0; i < next.length; i++) if (bytes[i] !== next[i]) return false;
  return true;
}

function applyWaveformAndCuesFromAnalysis(
  track: Record<string, unknown>,
  res: {
    waveform_preview?: number[];
    waveform_row_bands?: string | null;
    cue_points?: CuePointPayload[];
    bpm?: number;
    key?: string;
    bpm_locked?: boolean;
    key_locked?: boolean;
    analysis_pending?: boolean;
    analysis_stages?: number;
  },
): boolean {
  const searchTextBefore = `${track.bpm}|${track.key}|${track.analyzed}`;
  const ready = readyAnalysisFields(res);
  const bandPreview = ready.waveform ? decodeBandPreview(res.waveform_row_bands) : null;
  if (!ready.waveform) {
    track.bandsPreview = undefined;
    track.preview = undefined;
  } else if (bandPreview) {
    if (!sameBytes(track.bandsPreview, bandPreview)) track.bandsPreview = bandPreview;
    track.preview = undefined;
  } else if (res.waveform_preview?.length) {
    track.bandsPreview = undefined;
    track.preview = Uint8Array.from(res.waveform_preview);
  }
  if (res.cue_points) loadCuesFromPayload(track.id as number, res.cue_points);
  if (ready.bpm && res.bpm) {
    track.bpm = Number(res.bpm).toFixed(1);
    track.bpmExact = Number(res.bpm);
  }
  if (ready.key && res.key) track.key = res.key;
  track.bpmLocked = res.bpm_locked === true;
  track.keyLocked = res.key_locked === true;
  track.analyzed = true;
  track.analysisPending = res.analysis_pending === true;
  track.analysisLoaded = true;
  return searchTextBefore !== `${track.bpm}|${track.key}|${track.analyzed}`;
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
  if (isTrackAnalyzing(id)) return;
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
      if (applyWaveformAndCuesFromAnalysis(track, res)) bumpLibraryTracks();
      performance.mark('ff:detail-ready');
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

/** Puts tracks into the state a first analysis starts from: no waveform, and no BPM or key unless hand-set. */
function markTracksPendingAnalysis(ids: number[]) {
  const restarted: number[] = [];
  for (const id of ids) {
    const track = library.tracks.find((t) => t.id === id);
    if (!track || track.analysisPending) continue;
    track.preview = undefined;
    track.bandsPreview = undefined;
    track.analysisLoaded = false;
    track.analysisPending = true;
    if (!track.bpmLocked) {
      track.bpm = '—';
      track.bpmExact = 0;
    }
    if (!track.keyLocked) track.key = '—';
    restarted.push(id);
  }
  if (restarted.length === 0) return;
  bumpLibraryTracks();
  analysisStartListeners.forEach((listener) => listener(restarted));
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

function handleAnalysisPartialEvent(payload: { track_id: number }) {
  const track = library.tracks.find((t) => t.id === payload.track_id);
  if (track) void refreshTrackAnalysisDisplay(track);
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
    bpmExact: t.tempo / 100,
    bpmLocked: false,
    keyLocked: false,
    key: keyStr,
    time: timeStr,
    fav: undefined,
    genre: t.genre || '—',
    year: '—',
    rating: t.rating ?? 0,
    color: t.color ?? null,
    filePath: t.source_path,
    comment: '',
    analyzed,
    analysisPending: t.analysis_pending ?? false,
    raw: t,
    get cover() {
      return artwork.get(this.id, this.raw?.has_artwork);
    },
    get rowCover() {
      return artwork.getRow(this.id, this.raw?.has_artwork);
    },
    get status() {
      return fileStateGlyph(fileStateOf(this.id));
    },
    get bands() {
      return this.bandsPreview ? rowBandsFromBandBytes(this.bandsPreview) : rowBandsFor(this.preview);
    }
  };

  // Embedded preview → paint the mini waveform immediately, no N+1 fetch: the 3-band row
  // preview when the track has band data, else the mono PWAV.
  const bandPreview = decodeBandPreview(t.waveform_bands);
  if (bandPreview) {
    local.bandsPreview = bandPreview;
  } else {
    const preview = decodeTrackPreview(t.waveform_preview);
    if (preview && isValidWaveformPreview(preview)) {
      local.preview = Uint8Array.from(preview);
    }
  }

  return local;
}

/* ── Actions ────────────────────────────────────────────────────────── */

/** Loads full state from library backend / mock fallback. */
export async function loadState() {
  try {
    artwork.clear();
    invalidateAnalysisCache();
    const state = await tauriLoadState();
    recentlyAddedCount = state.tracks.filter((t) => t.tempo > 0).length;
    plainTracks = state.tracks.map((t, idx) => mapTrackInfoToLocal(t, idx));
    resetSearchIndex();
    library.tracks = plainTracks;
    library.playlists = state.playlists;
    scheduleSearchIndexWarmup();

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
export const LAUNCH_ANALYSIS_DELAY_MS = 5000;

let launchAnalysisTimer: ReturnType<typeof setTimeout> | undefined;

/** Resumes unfinished analysis a few seconds after launch so startup stays fast. Runs once. */
export function scheduleLaunchAnalysis() {
  if (launchAnalysisTimer !== undefined) return;
  launchAnalysisTimer = setTimeout(async () => {
    const queued = await tauriResumeAnalysis();
    if (queued === 0) return;
    library.analyzing = true;
    if (!library.syncing) library.statusMessage = 'Resuming analysis…';
  }, LAUNCH_ANALYSIS_DELAY_MS);
}

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
      listenToAnalysisPartial(handleAnalysisPartialEvent);
      listenToAnalysisComplete(handleAnalysisCompleteEvent);
      listenToWriteComplete(() => {
        library.syncing = false;
        library.statusMessage = 'Ready';
        refreshUsbState();
      });
    }
    scheduleLaunchAnalysis();
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
  return library.playlists.find((p) => p.name === active && !p.is_folder) ?? null;
}

function importErrorDetail(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

async function finishImport(newTracks: TrackInfo[], options?: ImportOptions) {
  if (newTracks.length === 0) return;

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

function scanAndAdd(paths: string[]) {
  return importWithDuplicateCheck(paths, { plan: planImport, ask: askAboutDuplicates, commit: commitImport });
}

/** Scan directory for new tracks */
export function importDirectory(path: string, options?: ImportOptions) {
  return importFiles([path], options);
}

/** Scan dropped files/folders */
export async function importFiles(paths: string[], options?: ImportOptions) {
  try {
    startImportActivity();
    const outcome = await scanAndAdd(paths);
    await finishImport(outcome.added, options);
    library.tauriError = lastTauriError;
    completeImportActivity(outcome);
  } catch (err) {
    failImportActivity(importErrorDetail(err));
    library.tauriError = lastTauriError;
    console.error('importFiles failed:', err);
  }
}

export async function importDroppedTracks(paths: string[]): Promise<{ outcome: ImportOutcome; ids: number[] } | null> {
  try {
    startImportActivity();
    const outcome = await scanAndAdd(paths);
    await loadState();
    library.tauriError = lastTauriError;
    if (outcome.added.length > 0) {
      library.pendingAnalyzeTrackIds = outcome.added.map((t) => t.id);
      library.showAnalysisPrompt = true;
    }
    const ids = [...new Set([...trackIdsUnderPaths(library.tracks, paths), ...outcome.skipped_ids])];
    return { outcome, ids };
  } catch (err) {
    failImportActivity(importErrorDetail(err));
    library.tauriError = lastTauriError;
    console.error('importDroppedTracks failed:', err);
    return null;
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
  const pendingIds = library.tracks.filter((t) => !t.analyzed || t.analysisPending).map((t) => t.id);
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
  const unique = knownTrackIds(ids);
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

/** Empty every favorite slot (the slots themselves stay). */
export async function clearAllFavorites() {
  let changed = false;
  for (const pl of library.playlists) {
    if (isFavoritePlaylistName(pl.name) && pl.track_ids.length > 0) {
      pl.track_ids = [];
      changed = true;
    }
  }
  if (!changed) return;
  await saveState();
  syncTrackFavoriteFields();
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
    const mounted = await tauriGetMountedVolumes();
    const changed = mounted.length !== library.volumes.length || mounted.some((v) => !library.volumes.includes(v));
    library.volumes = mounted;
    if (changed) void loadSticks(mounted);
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

onStickSyncFinished(async (path) => {
  if (library.activeVolume === path) await refreshUsbState();
});

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
  const volume = library.activeVolume;
  try {
    const res = await tauriReadUsbState(volume);
    if (library.activeVolume !== volume) return;
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
    if (library.activeVolume !== volume) return;
    library.statusMessage = 'Failed to read USB';
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

export async function revealFile(path: string) {
  try {
    await revealInFileManager(path);
  } catch (err) {
    library.statusMessage = String(err);
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

/** Syncs the open stick: exactly its All tracks and the playlists put on it. */
export async function syncToUsb() {
  if (!library.activeVolume) return;
  await syncStick(library.activeVolume);
}

/** Syncs every mounted stick in turn; stops at the first one that needs an answer or fails. */
export async function syncAllSticks() {
  for (const volume of library.volumes) {
    await syncStick(volume);
    if (stickSync.stage === 'confirm' || stickSync.stage === 'failed') return;
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

function nextPlaylistId(): number {
  return library.playlists.length > 0 ? Math.max(...library.playlists.map((p) => p.id)) + 1 : 1;
}

export async function addPlaylist(name: string, trackIds: number[] = [], parentId = 0): Promise<number> {
  const newId = nextPlaylistId();
  library.playlists.push({
    id: newId,
    name,
    track_ids: [...new Set(trackIds)],
    parent_id: parentId,
    is_folder: false,
  });
  if (parentId !== 0) expandFolder(parentId);
  await saveState();
  return newId;
}

export function defaultFolderName(): string {
  return uniqueName(library.playlists.map((p) => p.name), 'New folder');
}

export async function addFolder(parentId = 0, name = defaultFolderName()): Promise<number> {
  const newId = nextPlaylistId();
  library.playlists.push({ id: newId, name, track_ids: [], parent_id: parentId, is_folder: true });
  if (parentId !== 0) expandFolder(parentId);
  await saveState();
  return newId;
}

export async function duplicatePlaylist(id: number): Promise<number | null> {
  const source = library.playlists.find((p) => p.id === id && !p.is_folder);
  if (!source) return null;
  const copy = {
    ...source,
    id: nextPlaylistId(),
    name: uniqueName(library.playlists.map((p) => p.name), `${source.name} copy`),
    track_ids: [...source.track_ids],
  };
  const at = library.playlists.findIndex((p) => p.id === id) + 1;
  library.playlists = [...library.playlists.slice(0, at), copy, ...library.playlists.slice(at)];
  await saveState();
  return copy.id;
}

export function playlistsInFolder(folderId: number): PlaylistInput[] {
  const inside = descendantIds(library.playlists, folderId);
  return library.playlists.filter((p) => inside.has(p.id) && !p.is_folder);
}

/** Deletes a playlist, or a folder with everything inside it. */
export async function deletePlaylist(id: number) {
  library.playlists = removeWithDescendants(library.playlists, id);
  await saveState();
}

/** Deletes several playlists and folders (with everything inside folders) in one save. */
export async function deletePlaylists(ids: number[]) {
  for (const id of outermostIds(library.playlists, ids)) {
    library.playlists = removeWithDescendants(library.playlists, id);
  }
  await saveState();
}

/** Moves a playlist or folder into `parentId` (0 is the top level), next to `anchorId` when given. */
export async function movePlaylistInTree(
  id: number,
  parentId: number,
  anchorId: number | null = null,
  placeAfter = false
): Promise<boolean> {
  const moved = movePlaylist(library.playlists, id, parentId, anchorId, placeAfter);
  if (!moved) return false;
  library.playlists = moved;
  if (parentId !== 0) expandFolder(parentId);
  await saveState();
  return true;
}

/** Moves several playlists and folders together, keeping their relative order. */
export async function movePlaylistsInTree(
  ids: number[],
  parentId: number,
  anchorId: number | null = null,
  placeAfter = false
): Promise<boolean> {
  let playlists = library.playlists;
  let anchor = anchorId;
  let after = placeAfter;
  let movedAny = false;
  for (const id of outermostIds(playlists, ids)) {
    if (id === anchor) continue;
    const moved = movePlaylist(playlists, id, parentId, anchor, after);
    if (!moved) continue;
    playlists = moved;
    movedAny = true;
    if (placeAfter) {
      anchor = id;
      after = true;
    }
  }
  if (!movedAny) return false;
  library.playlists = playlists;
  if (parentId !== 0) expandFolder(parentId);
  await saveState();
  return true;
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

export async function addTracksToPlaylist(playlistId: number, trackIds: number[]): Promise<number> {
  const pl = library.playlists.find(p => p.id === playlistId);
  if (!pl) return 0;
  const fresh = [...new Set(trackIds)].filter(tid => !pl.track_ids.includes(tid));
  if (fresh.length === 0) return 0;
  pl.track_ids.push(...fresh);
  await saveState();
  if (isFavoritePlaylistName(pl.name)) syncTrackFavoriteFields();
  return fresh.length;
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

/** Resolve playlist by sidebar label (name). */
export function playlistByName(name: string): PlaylistInput | undefined {
  return library.playlists.find(p => p.name === name && !p.is_folder);
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


