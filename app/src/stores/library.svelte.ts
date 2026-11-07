/**
 * library.svelte.ts — Reactive library store utilizing Svelte 5 runes.
 * Interfaces with tauri.svelte.ts service layer and provides fallback mock data.
 */

import { untrack } from 'svelte';
import { appSettings, loadAppSettings, setAnalyzeAfterImport } from './app-settings.svelte.ts';
import {
  getVersion,
  getLibraryPath,
  loadState as tauriLoadState,
  getRowPreviews,
  refreshTrackLabels,
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
  listenToImportCovers,
  lastTauriError,
  getIpcDiagnostic,
  getAnalysisData,
  invalidateAnalysisCache,
  changeLibraryPath,
  resetLibrary as tauriResetLibrary,
  signalAnalysisUiBusy,
  type TrackInfo,
  type PlaylistInput,
  type SyncReport,
  type UsbLastSync
} from '../services/tauri.svelte.ts';
import { watchUiBusy } from '../services/ui-busy.ts';
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
import { readBootSnapshot, type BootSnapshot } from './boot-snapshot.ts';
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
import { isRawIds, rawIds } from '../lib/playlistIds.ts';
import { expandFolder, folderState } from './sidebar-folders.svelte.ts';
import { fileStateOf } from './file-status.svelte.ts';
import { fileStateGlyph } from '../lib/fileHealth.ts';

const peakLoadInflight = new Map<number, Promise<void>>();

const trackListLoadedListeners: (() => void)[] = [];

export function onTrackListLoaded(listener: () => void) {
  trackListLoadedListeners.push(listener);
}

/**
 * Call after changing fields of the tracks `changedIds` through their rows: copies them to the plain objects the search
 * text, counts and lookups read, rebuilds their search text and swaps the list so sorts and filters run again.
 */
export function bumpLibraryTracks(changedIds: Iterable<number>, writtenToPlain: readonly number[] = []) {
  const ids = [...changedIds];
  const searchTextChanged = copyRowFieldsToPlainTracks(ids);
  for (const id of writtenToPlain) searchTextChanged.push(id);
  if (haystackList) refreshHaystacks(searchTextChanged);
  const previous = trackList;
  trackList = trackList.slice();
  recordTrackListBump(previous, trackList, ids);
  scheduleSearchIndexWarmup();
}

const BUMP_LOG_LIMIT = 4096;
const BUMP_LOG_ID_LIMIT = 16384;
const listVersions = new WeakMap<object, number>();
let bumpLog: { version: number; ids: number[]; appended: boolean }[] = [];
let nextListVersion = 1;
let bumpLogIds = 0;

function recordTrackListBump(previous: object, next: object, ids: number[], appended = false) {
  let from = listVersions.get(previous);
  if (from === undefined) {
    from = nextListVersion++;
    listVersions.set(previous, from);
    bumpLog = [];
    bumpLogIds = 0;
  }
  const version = nextListVersion++;
  listVersions.set(next, version);
  bumpLog.push({ version, ids, appended });
  bumpLogIds += ids.length;
  while (bumpLog.length > 1 && (bumpLog.length > BUMP_LOG_LIMIT || bumpLogIds > BUMP_LOG_ID_LIMIT)) bumpLogIds -= bumpLog.shift()!.ids.length;
}

/**
 * The rows whose fields changed between an earlier `library.tracks` array and the current one, when the list was only
 * bumped in between (`bumpLibraryTracks`); `null` when it was replaced or the change is too old to tell.
 */
export function libraryRowsChangedSince(earlier: object): object[] | null {
  const positions = libraryPositionsChangedSince(earlier);
  if (!positions) return null;
  const current = untrack(() => trackList);
  return positions.map((position) => current[position]);
}

/** Like `libraryRowsChangedSince`, as positions in `library.tracks` (in the order the bumps touched them). */
export function libraryPositionsChangedSince(earlier: object): number[] | null {
  const changes = libraryChangesSince(earlier);
  return changes && !changes.grown ? changes.positions : null;
}

/**
 * What happened to `library.tracks` since an earlier array when it was only bumped and had rows appended in between
 * (`landImportedTracks`): `grown` when rows were appended (they sit after the `earlier.length` rows that were there,
 * and `positions` may name them too), `positions` of bumped rows; `null` when the list was replaced or the change is too
 * old to tell.
 */
export function libraryChangesSince(earlier: object): { positions: number[]; grown: boolean } | null {
  const current = untrack(() => trackList);
  if (earlier === current) return { positions: [], grown: false };
  const from = listVersions.get(earlier);
  const to = listVersions.get(current);
  if (from === undefined || to === undefined || bumpLog.length === 0 || bumpLog[0].version > from + 1) return null;
  const positions: number[] = [];
  let grown = false;
  for (const entry of bumpLog) {
    if (entry.version <= from || entry.version > to) continue;
    grown ||= entry.appended;
    for (const id of entry.ids) {
      const position = trackIndexById(id);
      if (current[position]?.id === id) positions.push(position);
    }
  }
  return { positions, grown };
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

function lowestFavoriteSlots(playlists: PlaylistInput[]): Map<number, number> {
  const lowest = new Map<number, number>();
  for (const pl of playlists) {
    const slot = favoriteSlotFromName(pl.name);
    if (slot == null) continue;
    for (const tid of pl.track_ids) {
      const current = lowest.get(tid);
      if (current === undefined || slot < current) lowest.set(tid, slot);
    }
  }
  return lowest;
}

/** Lowest favorite slot a track belongs to (for the FAV column). */
export function syncTrackFavoriteFields() {
  const lowest = lowestFavoriteSlots(library.playlists);
  library.tracks.forEach((track, i) => {
    const fav = lowest.get(track.id);
    const plain = plainTracks[i];
    const hasPlain = plain?.id === track.id;
    if (hasPlain ? plain.fav === fav : track.fav === fav) return;
    track.fav = fav;
    if (hasPlain) plain.fav = fav;
  });
}

function pushFavoritePlaylist(slot: number) {
  const name = favoritePlaylistName(slot);
  const newId =
    library.playlists.length > 0 ? Math.max(...library.playlists.map((pl) => pl.id)) + 1 : 1;
  library.playlists.push({ id: newId, name, track_ids: rawIds([]), parent_id: 0, is_folder: false });
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
}

/** Toggle membership in a favorite slot for each track (add / remove). */
export async function toggleFavoriteSlot(slot: number, trackIds: number[]) {
  if (slot < 1 || slot > FAVORITE_SLOTS || trackIds.length === 0) return;
  const pl = playlistByName(favoritePlaylistName(slot));
  if (!pl) return;
  const unique = [...new Set(trackIds)];
  const next = [...pl.track_ids];
  for (const tid of unique) {
    const idx = next.indexOf(tid);
    if (idx >= 0) next.splice(idx, 1);
    else next.push(tid);
  }
  pl.track_ids = rawIds(next);
  await saveState();
  syncTrackFavoriteFields();
}

/* ── Main Library Store State ───────────────────────────────────────── */
let recentlyAddedCount = 0;
let analyzingIds = $state.raw<number[]>([]);

type TrackRow = Record<string, any>;

const TRACK_VIEW_FIELDS: Record<string, (row: TrackRow) => unknown> = Object.assign(Object.create(null), {
  cover: (row: TrackRow) => artwork.get(row.id, row.raw?.has_artwork),
  rowCover: (row: TrackRow) => artwork.getRow(row.id, row.raw?.has_artwork),
  status: (row: TrackRow) => fileStateGlyph(fileStateOf(row.id)),
  bands: (row: TrackRow) => {
    rowPreviewVersion;
    if (row.bandsPreview) return rowBandsFromBandBytes(row.bandsPreview);
    return row.preview ? rowBandsFor(row.preview) : rowBandsFromWire(row.bandsWire);
  },
});

interface RowEntry {
  state: TrackRow;
  touched: Set<string> | null;
  exposed: Set<string> | null;
}

const rowEntries = new WeakMap<TrackRow, RowEntry>();

function createRowState(track: TrackRow): TrackRow {
  const created = $derived.by(() => {
    const row = $state(track);
    return row;
  });
  return untrack(() => created);
}

function rowEntryOf(track: TrackRow): RowEntry {
  let entry = rowEntries.get(track);
  if (!entry) {
    entry = { state: createRowState(track), touched: null, exposed: null };
    rowEntries.set(track, entry);
  }
  return entry;
}

function rowStateOf(track: TrackRow): TrackRow {
  return rowEntryOf(track).state;
}

function exposeContainer(entry: RowEntry, key: string | symbol, value: unknown) {
  if (typeof key === 'string' && value !== null && typeof value === 'object' && isPlainContainer(value)) (entry.exposed ??= new Set()).add(key);
}

function touchedEntryOf(track: TrackRow, key: string | symbol, value?: unknown): RowEntry {
  const entry = rowEntryOf(track);
  if (typeof key === 'string') {
    (entry.touched ??= new Set()).add(key);
    exposeContainer(entry, key, value);
  }
  return entry;
}

const rowPlainObject = Symbol('rowPlainObject');

/** The plain object behind a library row (its own fields as of the last `bumpLibraryTracks`), or the row itself. */
export function plainRowOf<T extends object>(row: T): T {
  return (row as Record<symbol, T>)[rowPlainObject] ?? row;
}

const trackRowHandler: ProxyHandler<TrackRow> = {
  get(track, key, receiver) {
    if (key === 'id') return track.id;
    if (key === rowPlainObject) return track;
    const view = typeof key === 'string' ? TRACK_VIEW_FIELDS[key] : undefined;
    if (view) return view(receiver);
    const entry = rowEntryOf(track);
    const value = Reflect.get(entry.state, key, receiver);
    exposeContainer(entry, key, value);
    return value;
  },
  has(track, key) {
    return key in TRACK_VIEW_FIELDS || Reflect.has(rowStateOf(track), key);
  },
  set(track, key, value) {
    return Reflect.set(touchedEntryOf(track, key, value).state, key, value);
  },
  defineProperty(track, key, descriptor) {
    return Reflect.defineProperty(touchedEntryOf(track, key, descriptor.value).state, key, descriptor);
  },
  deleteProperty(track, key) {
    return Reflect.deleteProperty(touchedEntryOf(track, key).state, key);
  },
  ownKeys(track) {
    return Reflect.ownKeys(rowStateOf(track));
  },
  getOwnPropertyDescriptor(track, key) {
    const entry = rowEntryOf(track);
    const descriptor = Reflect.getOwnPropertyDescriptor(entry.state, key);
    if (descriptor) exposeContainer(entry, key, descriptor.value);
    return descriptor;
  },
};

/**
 * A row is its plain object behind a handler that creates the `$state` proxy on the first read or write of anything but
 * `id`, so rows nobody has touched cost one `Proxy`. The proxy is created inside its own derived: a state created while
 * an effect runs would not be tracked by that effect's first reads.
 */
function reactiveTrack<T extends object>(track: T): T {
  return new Proxy(track, trackRowHandler as ProxyHandler<T>);
}

/**
 * The track list itself is raw state: replacing it is one signal, and swapping it after a change copies an array of
 * pointers instead of reading 10k elements through a reactive array proxy. Each track is its own reactive proxy, so a
 * row still follows its fields.
 */
let trackList = $state.raw<any[]>([]);

const trackListLength = $derived(trackList.length);

/**
 * The plain objects behind `library.tracks`, same order. Reading text through the row proxies costs ~1 s at 30k
 * tracks; these are read instead. A write through a row does not reach its plain object (a `$state` proxy keeps its
 * own copy), so `bumpLibraryTracks(ids)` copies the changed rows over.
 */
let plainTracks: Record<string, any>[] = [];

/** The plain objects behind `library.tracks`, by position. */
export function libraryPlainRows(): readonly Record<string, any>[] {
  return plainTracks;
}

function setLibraryTracks(tracks: Record<string, any>[], rows = tracks.map(reactiveTrack)) {
  plainTracks = tracks;
  trackList = rows;
}

function appendLibraryTracks(tracks: Record<string, any>[], rows: Record<string, any>[]) {
  plainTracks = plainTracks.concat(tracks);
  const previous = trackList;
  trackList = trackList.concat(rows);
  recordTrackListBump(previous, trackList, [], true);
}

function isPlainContainer(value: object): boolean {
  return Array.isArray(value) || Object.getPrototypeOf(value) === Object.prototype;
}

function ownStringKeys(value: object): string[] {
  const keys: string[] = [];
  for (const key of Reflect.ownKeys(value)) if (typeof key === 'string') keys.push(key);
  return keys;
}

function plainCopy(value: unknown): unknown {
  if (value === null || typeof value !== 'object') return value;
  if (Array.isArray(value)) {
    const copy: unknown[] = new Array(value.length);
    for (let i = 0; i < value.length; i++) if (i in value) copy[i] = plainCopy(value[i]);
    return copy;
  }
  if (Object.getPrototypeOf(value) === Object.prototype) {
    const copy: Record<string, unknown> = {};
    for (const key of ownStringKeys(value)) copy[key] = plainCopy((value as Record<string, unknown>)[key]);
    return copy;
  }
  return $state.snapshot(value);
}

function plainValue(value: unknown): unknown {
  return value !== null && typeof value === 'object' && isPlainContainer(value) ? plainCopy(value) : value;
}

const SEARCH_TEXT_FIELDS = new Set(['title', 'artist', 'album', 'label', 'genre', 'key', 'bpm', 'time', 'filePath', 'index', 'raw']);

function copyRowFieldsToPlainTracks(ids: number[]): number[] {
  const searchTextChanged: number[] = [];
  untrack(() => {
    for (const id of ids) {
      const i = trackIndexById(id);
      const row = trackList[i];
      const plain = plainTracks[i];
      if (!row || plain?.id !== id || row === plain) continue;
      const entry = rowEntries.get(plain);
      if (!entry) continue;
      const { state, touched, exposed } = entry;
      entry.touched = null;
      let changed = false;
      if (touched) {
        for (const key of touched) {
          if (!Reflect.has(state, key)) continue;
          plain[key] = plainValue(key in TRACK_VIEW_FIELDS ? row[key] : state[key]);
          changed ||= SEARCH_TEXT_FIELDS.has(key);
        }
      }
      if (exposed) {
        for (const key of exposed) {
          if (touched?.has(key) || !Reflect.has(state, key)) continue;
          plain[key] = plainValue(state[key]);
          changed ||= SEARCH_TEXT_FIELDS.has(key);
        }
      }
      if (changed) searchTextChanged.push(id);
    }
  });
  return searchTextChanged;
}

export const library = $state({
  /** Replaced, never mutated in place; `bumpLibraryTracks(ids)` after changing a track's fields. */
  get tracks(): any[] {
    return trackList;
  },
  set tracks(tracks: any[]) {
    setLibraryTracks(tracks);
  },
  playlists: [] as PlaylistInput[],
  volumes: [] as string[],
  
  // Active states
  activeVolume: null as string | null,
  usbTracks: [] as any[],
  usbPlaylists: [] as any[],
  usbLastSync: null as UsbLastSync | null,

  // App settings/status
  appVersion: '0.0.0',
  libraryPath: '',
  tracksLoaded: false,
  bootSnapshot: null as BootSnapshot | null,
  analyzing: false,
  /** Track ids currently in an analysis batch (drives row + waveform placeholder animation). Replaced, never mutated. */
  get analyzingTrackIds(): number[] {
    return analyzingIds;
  },
  set analyzingTrackIds(ids: number[]) {
    analyzingIds = ids;
  },
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
    return trackListLength;
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
          { kind: 'leaf', icon: 'list',     label: 'All Tracks',      count: trackListLength },
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

let haystackList: string[] | null = null;
let haystackLetters = new Int32Array(0);
let haystackDigits = new Int16Array(0);
let haystackCursor = 0;
let warmupTimer: ReturnType<typeof setTimeout> | undefined;
let warmupGeneration = 0;

const WARMUP_CHUNK = 100;
const WARMUP_SLICE_MS = 8;
const WARMUP_IDLE_RESERVE_MS = 2;
const WARMUP_DELAY_MS = 250;

function resetSearchIndex() {
  haystackList = null;
  haystackLetters = new Int32Array(0);
  haystackDigits = new Int16Array(0);
  haystackCursor = 0;
  warmupGeneration++;
}

const LETTER_BITS = new Int32Array(128);
const DIGIT_BITS = new Int16Array(128);
for (let code = 97; code <= 122; code++) LETTER_BITS[code] = 1 << (code - 97);
for (let code = 48; code <= 57; code++) DIGIT_BITS[code] = 1 << (code - 48);

function markCharacters(index: number, haystack: string) {
  let letters = 0;
  let digits = 0;
  for (let k = 0; k < haystack.length; k++) {
    const code = haystack.charCodeAt(k);
    if (code < 128) {
      letters |= LETTER_BITS[code];
      digits |= DIGIT_BITS[code];
    }
  }
  haystackLetters[index] = letters;
  haystackDigits[index] = digits;
}

function fillSearchIndex(limit: number) {
  haystackList ??= [];
  if (haystackLetters.length < plainTracks.length) {
    const letters = new Int32Array(plainTracks.length);
    const digits = new Int16Array(plainTracks.length);
    letters.set(haystackLetters);
    digits.set(haystackDigits);
    haystackLetters = letters;
    haystackDigits = digits;
  }
  const end = Math.min(plainTracks.length, haystackCursor + limit);
  for (; haystackCursor < end; haystackCursor++) {
    const haystack = trackHaystack(plainTracks[haystackCursor]);
    haystackList.push(haystack);
    markCharacters(haystackCursor, haystack);
  }
}

function refreshHaystacks(ids: number[]) {
  for (const id of ids) {
    const index = trackIndexById(id);
    if (index < 0 || index >= haystackCursor) continue;
    const plain = plainTracks[index];
    if (plain?.id !== id) continue;
    const haystack = trackHaystack(plain);
    haystackList![index] = haystack;
    markCharacters(index, haystack);
  }
}

function haystackListFor(): string[] {
  fillSearchIndex(plainTracks.length);
  return haystackList!;
}

function whenIdle(run: (deadline?: IdleDeadline) => void) {
  if (typeof requestIdleCallback === 'function') requestIdleCallback(run, { timeout: 2000 });
  else setTimeout(run, 0);
}

/** Builds the search text in idle-time chunks so the first keystroke finds it ready. */
function scheduleSearchIndexWarmup() {
  clearTimeout(warmupTimer);
  warmupTimer = setTimeout(() => {
    const generation = warmupGeneration;
    const step = (deadline?: IdleDeadline) => {
      if (generation !== warmupGeneration) return;
      if (haystackCursor < plainTracks.length) {
        const sliceEnd = performance.now() + WARMUP_SLICE_MS;
        do fillSearchIndex(WARMUP_CHUNK);
        while (
          haystackCursor < plainTracks.length &&
          deadline &&
          !deadline.didTimeout &&
          deadline.timeRemaining() > WARMUP_IDLE_RESERVE_MS &&
          performance.now() < sliceEnd
        );
        whenIdle(step);
      } else {
        haystacksFor(library.tracks);
      }
    };
    whenIdle(step);
  }, WARMUP_DELAY_MS);
}

const text = (value: unknown) => (value == null ? '' : String(value));

const unlessSame = (value: unknown, shown: string) => {
  const s = text(value);
  return s === shown ? '' : ' ' + s;
};

function trackHaystack(track: Record<string, unknown>): string {
  const raw = (track.raw ?? {}) as Record<string, unknown>;
  const title = text(track.title);
  const artist = text(track.artist);
  const album = text(track.album);
  const genre = text(track.genre);
  const key = text(track.key);
  const filePath = text(track.filePath);
  return (
    title + ' ' + artist + ' ' + album + ' ' + text(track.label) + ' ' + genre + ' ' + key + ' ' +
    text(track.bpm) + ' ' + text(track.time) + ' ' + filePath + ' ' + text(track.index) +
    unlessSame(raw.title, title) + unlessSame(raw.artist, artist) + unlessSame(raw.album, album) +
    unlessSame(raw.genre, genre) + unlessSame(raw.key, key) + unlessSame(raw.source_path, filePath)
  ).toLowerCase();
}

/**
 * Search text per track, cached per source array. Track text only changes
 * together with `bumpLibraryTracks()` / `loadState()`, which swap the array.
 */
function haystacksFor<T extends Record<string, unknown>>(tracks: T[]): string[] {
  let haystacks = haystackCache.get(tracks);
  if (!haystacks) {
    const list = haystackListFor();
    const members = tracks === library.tracks ? null : membersOf(tracks)?.members;
    haystacks =
      tracks === library.tracks
        ? list
        : members
          ? Array.from(members, (position) => list[position])
          : tracks.map((t) => {
              const index = trackIndexById(t.id as number);
              return index >= 0 && plainTracks[index].id === t.id ? list[index] : trackHaystack(t);
            });
    haystackCache.set(tracks, haystacks);
  }
  return haystacks;
}

interface FilterEntry {
  list: object;
  terms: string[];
  indices: number[];
  result: unknown[];
}

const FILTER_HISTORY = 16;
const filterHistory = new WeakMap<object, Map<string, FilterEntry>>();
const filterSources = new WeakMap<object, { source: object; positions: number[] }>();
let libraryFilters: { tracks: object; history: Map<string, FilterEntry> } | null = null;

/**
 * Where the rows of a `filterTracks` result of `library.tracks` sit in it (ascending), or `'all'` for `library.tracks`
 * itself; `null` for any other list, and for a result of a list that was replaced since.
 */
export function libraryPositionsOf(rows: readonly unknown[]): readonly number[] | 'all' | null {
  const current = untrack(() => trackList);
  if (rows === current) return 'all';
  const found = filterSources.get(rows);
  return found && found.source === current ? found.positions : null;
}

function libraryMembership(rows: readonly unknown[]): { list: object; positions: ArrayLike<number> | 'all' } | null {
  if (rows === untrack(() => trackList) || listVersions.has(rows)) return { list: rows, positions: 'all' };
  const filtered = filterSources.get(rows);
  if (filtered) return { list: filtered.source, positions: filtered.positions };
  const playlist = memberRows.get(rows);
  return playlist ? { list: playlist.list, positions: playlist.members } : null;
}

/**
 * Whether two lists of library rows (`library.tracks`, a `filterTracks` result of it, or the rows of a playlist) hold the
 * same rows in the same order, answered from where the rows sit in `library.tracks` when both were taken before and after
 * only bumps; `false` when that cannot be told (the rows may still be the same).
 */
export function sameLibraryMembers(earlier: readonly unknown[], rows: readonly unknown[]): boolean {
  if (earlier === rows) return true;
  const a = libraryMembership(earlier);
  const b = libraryMembership(rows);
  if (!a || !b || libraryPositionsChangedSince(a.list) === null || libraryPositionsChangedSince(b.list) === null) return false;
  if (a.positions === 'all' || b.positions === 'all') return a.positions === b.positions;
  if (a.positions === b.positions) return true;
  if (a.positions.length !== b.positions.length) return false;
  for (let i = 0; i < a.positions.length; i++) if (a.positions[i] !== b.positions[i]) return false;
  return true;
}

function narrows(older: string[], terms: string[]): boolean {
  return older.every((old) => terms.some((term) => term.includes(old)));
}

function singleCharacterBits(terms: string[]): { letters: number; digits: number } | null {
  let letters = 0;
  let digits = 0;
  for (const term of terms) {
    if (term.length !== 1) return null;
    const code = term.charCodeAt(0);
    if (code >= 97 && code <= 122) letters |= 1 << (code - 97);
    else if (code >= 48 && code <= 57) digits |= 1 << (code - 48);
    else return null;
  }
  return { letters, digits };
}

/**
 * `masked`: `haystacks` is the search text of `plainTracks` (`'all'`) or of the rows at these positions of it, whose letters and
 * digits were noted in `haystackLetters` and `haystackDigits`.
 */
function matchingIndices(haystacks: string[], terms: string[], candidates: number[] | null, masked: Int32Array | 'all' | null): number[] {
  const indices: number[] = [];
  const first = terms[0];
  const bits = masked ? singleCharacterBits(terms) : null;
  if (bits) {
    const letters = haystackLetters;
    const digits = haystackDigits;
    const wantLetters = bits.letters;
    const wantDigits = bits.digits;
    const positions = masked === 'all' ? null : (masked as Int32Array);
    if (candidates) {
      for (let c = 0; c < candidates.length; c++) {
        const i = candidates[c];
        const p = positions ? positions[i] : i;
        if ((letters[p] & wantLetters) === wantLetters && (digits[p] & wantDigits) === wantDigits) indices.push(i);
      }
    } else {
      for (let i = 0; i < haystacks.length; i++) {
        const p = positions ? positions[i] : i;
        if ((letters[p] & wantLetters) === wantLetters && (digits[p] & wantDigits) === wantDigits) indices.push(i);
      }
    }
    return indices;
  }
  if (terms.length === 1) {
    if (candidates) {
      for (let c = 0; c < candidates.length; c++) {
        const i = candidates[c];
        if (haystacks[i].includes(first)) indices.push(i);
      }
    } else {
      for (let i = 0; i < haystacks.length; i++) if (haystacks[i].includes(first)) indices.push(i);
    }
    return indices;
  }
  const matches = (i: number) => {
    const haystack = haystacks[i];
    for (let t = 0; t < terms.length; t++) if (!haystack.includes(terms[t])) return false;
    return true;
  };
  if (candidates) {
    for (let c = 0; c < candidates.length; c++) if (matches(candidates[c])) indices.push(candidates[c]);
  } else {
    for (let i = 0; i < haystacks.length; i++) if (matches(i)) indices.push(i);
  }
  return indices;
}

/** Match tracks against a whitespace-separated query (all terms must match). */
export function filterTracks<T extends Record<string, unknown>>(
  tracks: T[],
  query: string
): T[] {
  const trimmed = query.trim().toLowerCase();
  if (!trimmed) return tracks;
  const isLibrary = tracks === untrack(() => trackList);
  let history = filterHistory.get(tracks);
  if (!history) {
    history = isLibrary && libraryFilters && libraryPositionsChangedSince(libraryFilters.tracks) !== null ? new Map(libraryFilters.history) : new Map();
    filterHistory.set(tracks, history);
  }
  const known = history.get(trimmed);
  if (known && known.list === tracks) return known.result as T[];
  const terms = trimmed.split(/\s+/).filter(Boolean);
  const haystacks = haystacksFor(tracks);
  const member = isLibrary ? null : membersOf(tracks);
  if (isLibrary && known) {
    const changed = libraryPositionsChangedSince(known.list);
    if (changed) {
      const indices = withChangedMatches(known.indices, changed, haystacks, terms);
      if (indices === known.indices) return remember(history, trimmed, terms, indices, tracks, true, null, known.result);
      return remember(history, trimmed, terms, indices, tracks, true);
    }
  }
  let candidates: number[] | null = null;
  let changedSinceCandidates: number[] = [];
  for (const entry of history.values()) {
    if (!narrows(entry.terms, terms) || (candidates && entry.indices.length >= candidates.length)) continue;
    if (entry.list === tracks) {
      candidates = entry.indices;
      changedSinceCandidates = [];
      continue;
    }
    const changed = isLibrary ? libraryPositionsChangedSince(entry.list) : null;
    if (changed) {
      candidates = entry.indices;
      changedSinceCandidates = changed;
    }
  }
  const masked = isLibrary ? 'all' : (member?.members ?? null);
  let indices = matchingIndices(haystacks, terms, candidates, masked);
  if (candidates && changedSinceCandidates.length > 0) {
    const outside = [...new Set(changedSinceCandidates)].sort((x, y) => x - y).filter((i) => !isAmong(candidates, i));
    if (outside.length > 0) indices = mergeAscending(indices, matchingIndices(haystacks, terms, outside, masked));
  }
  return remember(history, trimmed, terms, indices, tracks, isLibrary, member);
}

function mergeAscending(first: number[], second: number[]): number[] {
  if (second.length === 0) return first;
  const merged: number[] = [];
  let a = 0;
  let b = 0;
  while (a < first.length && b < second.length) merged.push(first[a] < second[b] ? first[a++] : second[b++]);
  while (a < first.length) merged.push(first[a++]);
  while (b < second.length) merged.push(second[b++]);
  return merged;
}

function remember<T>(history: Map<string, FilterEntry>, query: string, terms: string[], indices: number[], tracks: T[], isLibrary: boolean, member: MemberRows | null = null, sameRows: readonly unknown[] | null = null): T[] {
  const result = indices.length === tracks.length ? tracks : sameRows ? (sameRows.slice() as T[]) : indices.map((i) => tracks[i]);
  history.set(query, { list: tracks, terms, indices, result });
  if (history.size > FILTER_HISTORY) history.delete(history.keys().next().value!);
  if (isLibrary) {
    libraryFilters = { tracks, history };
    if (result !== tracks) filterSources.set(result, { source: tracks, positions: indices });
  } else if (member && result !== tracks) {
    memberRows.set(result, { ...member, members: Int32Array.from(indices, (i) => member.members[i]) });
  }
  return result;
}

function withChangedMatches(earlier: number[], changed: number[], haystacks: string[], terms: string[]): number[] {
  const moved = [...new Set(changed)].sort((a, b) => a - b);
  const matching = moved.filter((i) => terms.every((term) => haystacks[i].includes(term)));
  const nowMatching = new Set(matching);
  if (moved.every((i) => isAmong(earlier, i) === nowMatching.has(i))) return earlier;
  const dropped = new Set(moved);
  const kept = earlier.filter((i) => !dropped.has(i));
  if (matching.length === 0) return kept;
  const merged: number[] = [];
  let a = 0;
  let b = 0;
  while (a < kept.length && b < matching.length) merged.push(kept[a] < matching[b] ? kept[a++] : matching[b++]);
  while (a < kept.length) merged.push(kept[a++]);
  while (b < matching.length) merged.push(matching[b++]);
  return merged;
}

function isAmong(ascending: readonly number[], value: number): boolean {
  let low = 0;
  let high = ascending.length;
  while (low < high) {
    const mid = (low + high) >>> 1;
    if (ascending[mid] < value) low = mid + 1;
    else high = mid;
  }
  return ascending[low] === value;
}

let indexCache: { source: object; length: number; byId: Map<number, number> | null; slots: Int32Array | null } | null = null;

/** Tracks never analysed. Changes with the track list (every field change bumps it); counted on the plain objects. */
export function unanalyzedTrackCount(): number {
  const tracks = library.tracks;
  let count = 0;
  for (let i = 0; i < tracks.length; i++) if (!plainTracks[i].analyzed) count++;
  return count;
}

/** Whether any track was never analysed; read on the plain objects like `unanalyzedTrackCount`, stops at the first. */
export function hasUnanalyzedTracks(): boolean {
  const tracks = library.tracks;
  for (let i = 0; i < tracks.length; i++) if (!plainTracks[i].analyzed) return true;
  return false;
}

function positionsByIdTable(tracks: readonly Record<string, any>[]): Int32Array | null {
  let highest = 0;
  for (let i = 0; i < tracks.length; i++) {
    const id = tracks[i].id;
    if (!Number.isInteger(id) || id < 0) return null;
    if (id > highest) highest = id;
  }
  if (highest > tracks.length * 8 + 4096) return null;
  const slots = new Int32Array(highest + 1);
  for (let i = 0; i < tracks.length; i++) slots[tracks[i].id] = i + 1;
  return slots;
}

function trackIndexTables() {
  if (!indexCache || indexCache.source !== plainTracks || indexCache.length !== plainTracks.length) {
    const slots = positionsByIdTable(plainTracks);
    indexCache = {
      source: plainTracks,
      length: plainTracks.length,
      byId: slots ? null : new Map(plainTracks.map((t, i) => [t.id as number, i])),
      slots,
    };
  }
  return indexCache;
}

function trackIndexById(id: number): number {
  const { slots, byId } = trackIndexTables();
  if (slots) return (id >>> 0) === id && id < slots.length ? slots[id] - 1 : -1;
  return byId!.get(id) ?? -1;
}

/** Covers stored for imported tracks whose rows have not landed yet (`landImportedTracks`). */
const coversBeforeRows = new Set<number>();

/** Imported tracks come back without artwork; their covers are stored afterwards. */
export function markCoversStored(ids: number[]) {
  const changed: number[] = [];
  for (const id of ids) {
    const track = libraryTrackById(id);
    if (!track) {
      coversBeforeRows.add(id);
      continue;
    }
    if (!track.raw || track.raw.has_artwork) continue;
    track.raw.has_artwork = true;
    changed.push(id);
  }
  if (changed.length > 0) bumpLibraryTracks(changed);
}

export function libraryTrackById(id: number) {
  const tracks = library.tracks;
  const track = tracks[trackIndexById(id)];
  return track?.id === id ? track : tracks.find((t) => t.id === id);
}

/** Tracks shown for a sidebar source (label, playlist name, or USB path). */
export function tracksForSidebarSource(source: string | null | undefined) {
  const active = source ?? 'All Tracks';
  if (!active || active === 'All Tracks') {
    return library.tracks;
  }
  if (active === 'Recently Added') {
    return library.tracks.filter((_, i) => plainTracks[i]?.raw?.tempo > 0);
  }
  if (library.volumes.includes(active)) {
    return library.usbTracks;
  }
  const playlist = library.playlists.find((p) => p.name === active && !p.is_folder);
  if (playlist) {
    const ids = playlist.track_ids;
    return isRawIds(ids) ? playlistRowsOf(ids) : ids.map(libraryTrackById).filter(Boolean);
  }
  return library.tracks;
}

interface MemberRows {
  list: object;
  members: Int32Array;
  unique: boolean;
}

const memberRows = new WeakMap<object, MemberRows>();
const playlistViews = new WeakMap<number[], { list: object; rows: any[] }>();

function membersOf(rows: readonly unknown[]): MemberRows | null {
  const found = memberRows.get(rows);
  return found && libraryPositionsChangedSince(found.list) !== null ? found : null;
}

/**
 * Where the rows of a playlist, or of a `filterTracks` result of one, sit in `library.tracks` (in the order of the rows,
 * each position once); `null` for any other list and once the library list was replaced (bumps keep positions).
 */
export function libraryMembersOf(rows: readonly unknown[]): Int32Array | null {
  const found = membersOf(rows);
  return found?.unique ? found.members : null;
}

function buildPlaylistRows(ids: readonly number[], tracks: any[]): { rows: any[]; found: MemberRows | null } {
  if (tracks.length !== plainTracks.length) return { rows: ids.map(libraryTrackById).filter(Boolean), found: null };
  const rows: any[] = new Array(ids.length);
  const members = new Int32Array(ids.length);
  const seen = new Uint8Array(tracks.length);
  let count = 0;
  let unique = true;
  const { slots, byId } = trackIndexTables();
  for (let i = 0; i < ids.length; i++) {
    const id = ids[i];
    const position = slots ? ((id >>> 0) === id && id < slots.length ? slots[id] - 1 : -1) : (byId!.get(id) ?? -1);
    if (position < 0) continue;
    rows[count] = tracks[position];
    members[count++] = position;
    if (seen[position] === 1) unique = false;
    seen[position] = 1;
  }
  rows.length = count;
  return { rows, found: { list: tracks, members: members.subarray(0, count), unique } };
}

function playlistRowsOf(ids: number[]): any[] {
  const tracks = library.tracks;
  const view = playlistViews.get(ids);
  if (view && view.list === tracks) return view.rows;
  const stored = view ? memberRows.get(view.rows) : undefined;
  const changed = view && stored ? libraryPositionsChangedSince(view.list) : null;
  if (view && stored && changed) {
    const moved = new Set(changed);
    const rows = stored.members.some((position) => moved.has(position)) ? view.rows.slice() : view.rows;
    memberRows.set(rows, { ...stored, list: tracks });
    playlistViews.set(ids, { list: tracks, rows });
    return rows;
  }
  const built = buildPlaylistRows(ids, tracks);
  if (built.found) memberRows.set(built.rows, built.found);
  playlistViews.set(ids, { list: tracks, rows: built.rows });
  return built.rows;
}

/** Human-readable title for a sidebar source id. */
export function sidebarSourceLabel(source: string) {
  if (!source || source === 'All Tracks') return 'All Tracks';
  if (library.volumes.includes(source)) {
    return source.replace(/\\/g, '/').split('/').pop() || source;
  }
  return source;
}

let analyzingIdSet: { ids: number[]; set: Set<number> } | null = null;

export function isTrackAnalyzing(trackId: number): boolean {
  const ids = library.analyzingTrackIds;
  if (analyzingIdSet?.ids !== ids) analyzingIdSet = { ids, set: new Set(ids) };
  return analyzingIdSet.set.has(trackId);
}

function isKnownTrackId(id: number): boolean {
  return plainTracks[trackIndexById(id)]?.id === id || plainTracks.some((t) => t.id === id);
}

function knownTrackIds(ids: number[]): number[] {
  return [...new Set(ids)].filter((id) => id > 0 && isKnownTrackId(id));
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
  const track = libraryTrackById(trackId);
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
    track.bandsWire = undefined;
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
        bumpLibraryTracks([id]);
        return;
      }
      if (applyWaveformAndCuesFromAnalysis(track, res)) bumpLibraryTracks([id]);
      performance.mark('ff:detail-ready');
    } catch (err) {
      console.warn('ensureTrackPeaksLoaded failed:', err);
      track.analyzed = false;
      track.analysisLoaded = false;
      bumpLibraryTracks([id]);
    } finally {
      peakLoadInflight.delete(id);
    }
  })();
  peakLoadInflight.set(id, job);
  return job;
}

const ANALYSIS_APPLY_INTERVAL_MS = 250;
const queuedAnalysisRefresh = new Set<number>();
const queuedAnalysisFinish = new Set<number>();
let analysisApplyTimer: ReturnType<typeof setTimeout> | undefined;
let analysisApplying = false;
let lastAnalysisApplyAt = -Infinity;

async function fetchAnalysisForDisplay(id: number, attempt = 0): Promise<any> {
  try {
    return await getAnalysisData(id, attempt > 0);
  } catch (err) {
    if (attempt >= 4) throw err;
    await new Promise((r) => setTimeout(r, 120 * (attempt + 1)));
    return fetchAnalysisForDisplay(id, attempt + 1);
  }
}

async function refreshTrackAnalysis(id: number) {
  const track = libraryTrackById(id);
  if (!track) return;
  try {
    const res = await fetchAnalysisForDisplay(id);
    if (isValidAnalysisPayload(res)) {
      applyWaveformAndCuesFromAnalysis(track, res);
      return;
    }
  } catch (err) {
    console.warn('Failed to refresh analysis display:', err);
  }
  track.analyzed = false;
  track.analysisLoaded = false;
}

/** Analysis events arrive per track; the rows take them in batches, at most one library update per interval. */
function queueAnalysisRefresh(trackId: number, finished: boolean) {
  invalidateAnalysisCache(trackId);
  queuedAnalysisRefresh.add(trackId);
  if (finished) queuedAnalysisFinish.add(trackId);
  scheduleAnalysisApply();
}

function scheduleAnalysisApply() {
  if (analysisApplyTimer !== undefined || analysisApplying) return;
  const wait = Math.max(0, lastAnalysisApplyAt + ANALYSIS_APPLY_INTERVAL_MS - performance.now());
  analysisApplyTimer = setTimeout(() => {
    analysisApplyTimer = undefined;
    void applyQueuedAnalysis();
  }, wait);
}

async function applyQueuedAnalysis() {
  analysisApplying = true;
  const ids = [...queuedAnalysisRefresh];
  const finished = new Set(queuedAnalysisFinish);
  queuedAnalysisRefresh.clear();
  queuedAnalysisFinish.clear();
  try {
    if (finished.size > 0) {
      library.analyzingTrackIds = library.analyzingTrackIds.filter((id) => !finished.has(id));
    }
    await Promise.all(ids.map(refreshTrackAnalysis));
    if (ids.length > 0) bumpLibraryTracks(ids);
  } finally {
    lastAnalysisApplyAt = performance.now();
    analysisApplying = false;
    if (queuedAnalysisRefresh.size > 0 || queuedAnalysisFinish.size > 0) scheduleAnalysisApply();
  }
}

function finishAnalyzingTrack(trackId: number) {
  queueAnalysisRefresh(trackId, true);
}

/** Puts tracks into the state a first analysis starts from: no waveform, and no BPM or key unless hand-set. */
function markTracksPendingAnalysis(ids: number[]) {
  const restarted: number[] = [];
  const searchTextWrittenToPlain: number[] = [];
  for (const id of ids) {
    const row = libraryTrackById(id);
    if (!row) continue;
    const plain = plainRowOf(row);
    const writtenToPlain = !rowEntries.has(plain);
    const track = writtenToPlain ? plain : row;
    if (track.analysisPending) continue;
    track.preview = undefined;
    track.bandsPreview = undefined;
    track.bandsWire = undefined;
    track.analysisLoaded = false;
    track.analysisPending = true;
    let searchTextWritten = false;
    if (!track.bpmLocked) {
      track.bpm = '—';
      track.bpmExact = 0;
      searchTextWritten = true;
    }
    if (!track.keyLocked) {
      track.key = '—';
      searchTextWritten = true;
    }
    if (writtenToPlain && searchTextWritten) searchTextWrittenToPlain.push(id);
    restarted.push(id);
  }
  if (restarted.length === 0) return;
  bumpLibraryTracks(restarted, searchTextWrittenToPlain);
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
  queueAnalysisRefresh(payload.track_id, false);
}

let analysisRunsEnded = 0;

function handleAnalysisCompleteEvent() {
  analysisRunsEnded++;
  library.analyzing = false;
  library.analyzingTrackIds = [];
  library.analysisProgress = { current: 0, total: 0, message: '' };
  if (!library.syncing) {
    library.statusMessage = 'Ready';
  }
}

const WIRE_BANDS_CACHE_LIMIT = 512;
const wireBandsCache = new Map<string | number[], ReturnType<typeof rowBandsFromBandBytes>>();

function rowBandsFromWire(wire: string | number[] | undefined) {
  if (!wire) return null;
  const hit = wireBandsCache.get(wire);
  if (hit !== undefined) return hit;
  const bytes = decodeBandPreview(wire);
  const bands = bytes ? rowBandsFromBandBytes(bytes) : null;
  wireBandsCache.set(wire, bands);
  if (wireBandsCache.size > WIRE_BANDS_CACHE_LIMIT) wireBandsCache.delete(wireBandsCache.keys().next().value!);
  return bands;
}

/* ── Mapper ─────────────────────────────────────────────────────────── */
const bpmTexts = new Map<number, string>();
const timeTexts = new Map<number, string>();

function bpmText(tempo: number): string {
  let text = bpmTexts.get(tempo);
  if (text === undefined) bpmTexts.set(tempo, (text = tempo > 0 ? (tempo / 100).toFixed(1) : '—'));
  return text;
}

function timeText(durationSecs: number): string {
  if (!(durationSecs > 0)) return '—';
  const whole = Math.floor(durationSecs);
  let text = timeTexts.get(whole);
  if (text === undefined) timeTexts.set(whole, (text = `${Math.floor(whole / 60)}:${String(whole % 60).padStart(2, '0')}`));
  return text;
}

function mapTrackInfoToLocal(t: TrackInfo, idx: number, fav: number | undefined) {
  const position = idx + 1;
  const indexStr = position < 10 ? '0' + position : String(position);
  const bpmStr = bpmText(t.tempo);
  const keyStr = t.key || '—';
  const timeStr = timeText(t.duration_secs);

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
    fav,
    genre: t.genre || '—',
    year: '—',
    rating: t.rating ?? 0,
    color: t.color ?? null,
    filePath: t.source_path,
    comment: '',
    analyzed,
    analysisPending: t.analysis_pending ?? false,
    raw: t,
  };

  applyWirePreview(local, t);
  return local;
}

function applyWirePreview(local: Record<string, any>, wire: { waveform_bands?: string | number[] | null; waveform_preview?: string | number[] | null }) {
  if (wire.waveform_bands) {
    local.bandsWire = wire.waveform_bands;
    return;
  }
  const preview = decodeTrackPreview(wire.waveform_preview);
  if (preview && isValidWaveformPreview(preview)) local.preview = Uint8Array.from(preview);
}

let rowPreviewVersion = $state(0);
const rowPreviewAsked = new WeakSet<object>();
let rowPreviewQueue: Record<string, any>[] = [];

function hasRowPreview(track: Record<string, any>) {
  return !!(track.bandsWire || track.preview || track.bandsPreview);
}

/**
 * The list answer carries no row previews; rows on screen ask for theirs here, batched into one
 * `get_row_previews` call per task. Each track object asks once, so a reload (new objects) asks again.
 */
export function requestRowPreviews(tracks: readonly (Record<string, any> | undefined)[]) {
  const queued = rowPreviewQueue.length;
  for (const track of tracks) {
    if (!track || !track.analyzed || hasRowPreview(track) || rowPreviewAsked.has(track)) continue;
    rowPreviewAsked.add(track);
    rowPreviewQueue.push(track);
  }
  if (queued === 0 && rowPreviewQueue.length > 0) queueMicrotask(flushRowPreviews);
}

async function flushRowPreviews() {
  const batch = rowPreviewQueue;
  rowPreviewQueue = [];
  if (batch.length === 0) return;
  let answers;
  try {
    answers = await getRowPreviews(batch.map((t) => t.id));
  } catch (err) {
    for (const track of batch) rowPreviewAsked.delete(track);
    console.warn('row previews unavailable:', err);
    return;
  }
  const byId = new Map(answers.map((a) => [a.id, a]));
  let changed = false;
  for (const track of batch) {
    const answer = byId.get(track.id);
    if (!answer || hasRowPreview(track)) continue;
    applyWirePreview(track, answer);
    changed ||= hasRowPreview(track);
  }
  if (changed) rowPreviewVersion++;
}

const TRACK_MAP_CHUNK = 2500;

function nextTask(): Promise<void> {
  const channel = new MessageChannel();
  return new Promise((resolve) => {
    channel.port1.onmessage = () => {
      channel.port1.close();
      resolve();
    };
    channel.port2.postMessage(null);
  });
}

async function mapTracksInTaskSizedChunks(tracks: TrackInfo[], favorites: Map<number, number>, firstIndex = 0) {
  const plain: Record<string, any>[] = new Array(tracks.length);
  const rows: Record<string, any>[] = new Array(tracks.length);
  for (let start = 0; start < tracks.length; start += TRACK_MAP_CHUNK) {
    if (start > 0) await nextTask();
    const end = Math.min(tracks.length, start + TRACK_MAP_CHUNK);
    for (let i = start; i < end; i++) {
      plain[i] = mapTrackInfoToLocal(tracks[i], firstIndex + i, favorites.get(tracks[i].id));
      rows[i] = reactiveTrack(plain[i]);
    }
  }
  return { plain, rows };
}

/* ── Actions ────────────────────────────────────────────────────────── */

/** Loads full state from library backend / mock fallback. */
export async function loadState() {
  try {
    artwork.clear();
    invalidateAnalysisCache();
    const state = await tauriLoadState();
    recentlyAddedCount = state.tracks.filter((t) => t.tempo > 0).length;
    const { plain, rows } = await mapTracksInTaskSizedChunks(state.tracks, lowestFavoriteSlots(state.playlists));
    resetSearchIndex();
    setLibraryTracks(plain, rows);
    performance.mark('ff:state-rows');
    for (const playlist of state.playlists) rawIds(playlist.track_ids);
    library.playlists = state.playlists;
    scheduleSearchIndexWarmup();
    for (const listener of trackListLoadedListeners) listener();

    await ensureFavoriteSlots();
    library.tauriError = lastTauriError;
  } catch (err) {
    console.error('loadState failed:', err);
    library.tauriError = lastTauriError;
    throw err;
  } finally {
    library.tracksLoaded = true;
    library.bootSnapshot = null;
  }
}

async function applyChangedTrackLabels() {
  let changes;
  try {
    changes = await refreshTrackLabels();
  } catch (err) {
    console.warn('track labels not refreshed:', err);
    return;
  }
  const ids: number[] = [];
  for (const change of changes) {
    const track = libraryTrackById(change.id);
    if (!track) continue;
    track.rating = change.rating ?? 0;
    track.color = change.color ?? null;
    ids.push(change.id);
  }
  if (ids.length > 0) bumpLibraryTracks(ids);
}

/**
 * Appends freshly imported tracks: the backend gives them the next ids, so `load_state` would
 * list them last with the same fields, and nothing else in the state changes on import.
 */
async function landImportedTracks(added: TrackInfo[]) {
  if (added.length === 0) return;
  const offset = plainTracks.length;
  const favorites = lowestFavoriteSlots(library.playlists);
  const { plain, rows } = await mapTracksInTaskSizedChunks(added, favorites, offset);
  recentlyAddedCount += added.filter((t) => t.tempo > 0).length;
  appendLibraryTracks(plain, rows);
  const coveredEarly = added.filter((t) => coversBeforeRows.delete(t.id)).map((t) => t.id);
  if (coveredEarly.length > 0) markCoversStored(coveredEarly);
  warmupGeneration++;
  scheduleSearchIndexWarmup();
  for (const listener of trackListLoadedListeners) listener();
}

let saveInFlight: Promise<void> | null = null;
let savePending: Promise<void> | null = null;

async function writePlaylists() {
  try {
    await tauriSaveState(library.playlists);
  } catch (err) {
    console.error('saveState failed:', err);
  }
}

function startSave(): Promise<void> {
  const run: Promise<void> = writePlaylists().finally(() => {
    if (saveInFlight === run) saveInFlight = null;
  });
  saveInFlight = run;
  return run;
}

/**
 * Saves playlists to backend. Tracks are already auto-persisted.
 * One save runs at a time; calls made meanwhile share one follow-up save that sends the newest playlists.
 */
export function saveState(): Promise<void> {
  if (!saveInFlight) return startSave();
  if (!savePending) {
    savePending = saveInFlight.then(() => {
      savePending = null;
      return startSave();
    });
  }
  return savePending;
}

export function flushPendingSave() {
  if (!savePending) return;
  void writePlaylists();
}

if (typeof addEventListener === 'function') addEventListener('pagehide', flushPendingSave);

let libraryListenersInstalled = false;

/** Initialize library store, triggers on boot */
export const LAUNCH_ANALYSIS_DELAY_MS = 5000;

let launchAnalysisTimer: ReturnType<typeof setTimeout> | undefined;

/** Resumes unfinished analysis a few seconds after launch so startup stays fast. Runs once. */
export function scheduleLaunchAnalysis() {
  if (launchAnalysisTimer !== undefined) return;
  launchAnalysisTimer = setTimeout(async () => {
    const runsEndedBefore = analysisRunsEnded;
    const queued = await tauriResumeAnalysis();
    if (queued === 0 || analysisRunsEnded !== runsEndedBefore) return;
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
    if (!library.tracksLoaded) library.bootSnapshot = readBootSnapshot(path);

    await loadState();
    whenIdle(() => void applyChangedTrackLabels());
    await refreshVolumes();

    if (!libraryListenersInstalled) {
      libraryListenersInstalled = true;
      listenToAnalysisProgress(handleAnalysisProgressEvent);
      listenToAnalysisPartial(handleAnalysisPartialEvent);
      listenToAnalysisComplete(handleAnalysisCompleteEvent);
      listenToImportCovers(markCoversStored);
      watchUiBusy(window, () => library.analyzing, signalAnalysisUiBusy);
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

  await landImportedTracks(newTracks);

  await offerAnalysis(newTracks.map((t) => t.id), options?.analyze);
}

async function offerAnalysis(ids: number[], analyzeNow = false) {
  if (ids.length === 0) return;
  await loadAppSettings();
  if (analyzeNow || appSettings.analyzeAfterImport) {
    await analyzeTrackIds(ids);
    return;
  }
  library.pendingAnalyzeTrackIds = ids;
  library.showAnalysisPrompt = true;
}

/** Close the post-import analyze prompt without running analysis. */
export function dismissAnalysisPrompt() {
  library.showAnalysisPrompt = false;
  library.pendingAnalyzeTrackIds = [];
}

/** Run analysis for tracks pending from the last import, then close the prompt. */
export async function confirmAnalysisPrompt(always = false) {
  const ids = [...library.pendingAnalyzeTrackIds];
  dismissAnalysisPrompt();
  if (always) await setAnalyzeAfterImport(true).catch((err) => console.error('setAnalyzeAfterImport failed:', err));
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
    await landImportedTracks(outcome.added);
    library.tauriError = lastTauriError;
    await offerAnalysis(outcome.added.map((t) => t.id));
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
    const gone = new Set(ids);
    library.playlists.forEach(pl => {
      pl.track_ids = rawIds(pl.track_ids.filter(tid => !gone.has(tid)));
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
  pl.track_ids = rawIds(pl.track_ids.filter((id) => !drop.has(id)));
  await saveState();
  if (isFavoritePlaylistName(pl.name)) syncTrackFavoriteFields();
}

/** Remove every track from a playlist (favorites / curate lists). */
export async function clearPlaylistTracks(playlistId: number) {
  const pl = library.playlists.find((p) => p.id === playlistId);
  if (!pl || pl.track_ids.length === 0) return;
  pl.track_ids = rawIds([]);
  await saveState();
  if (isFavoritePlaylistName(pl.name)) syncTrackFavoriteFields();
}

/** Empty every favorite slot (the slots themselves stay). */
export async function clearAllFavorites() {
  let changed = false;
  for (const pl of library.playlists) {
    if (isFavoritePlaylistName(pl.name) && pl.track_ids.length > 0) {
      pl.track_ids = rawIds([]);
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
      library.usbLastSync = null;
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
    library.usbLastSync = null;
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
      library.usbLastSync = res.last_sync ?? null;
    } else {
      library.usbTracks = [];
      library.usbPlaylists = [];
      library.usbLastSync = null;
    }
  } catch (err) {
    console.error('refreshUsbState failed:', err);
    if (library.activeVolume !== volume) return;
    library.statusMessage = 'Failed to read USB';
    library.usbTracks = [];
    library.usbPlaylists = [];
    library.usbLastSync = null;
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
    track_ids: rawIds([...new Set(trackIds)]),
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
  library.playlists.push({ id: newId, name, track_ids: rawIds([]), parent_id: parentId, is_folder: true });
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
    track_ids: rawIds([...source.track_ids]),
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
  const members = new Set(pl.track_ids);
  const fresh = [...new Set(trackIds)].filter(tid => !members.has(tid));
  if (fresh.length === 0) return 0;
  pl.track_ids = rawIds([...pl.track_ids, ...fresh]);
  await saveState();
  if (isFavoritePlaylistName(pl.name)) syncTrackFavoriteFields();
  return fresh.length;
}

export async function removeTrackFromPlaylist(playlistId: number, trackId: number) {
  const pl = library.playlists.find(p => p.id === playlistId);
  if (!pl) return;
  pl.track_ids = rawIds(pl.track_ids.filter(tid => tid !== trackId));
  await saveState();
  if (isFavoritePlaylistName(pl.name)) syncTrackFavoriteFields();
}

export async function reorderPlaylistTracks(playlistId: number, orderedIds: number[]) {
  const pl = library.playlists.find(p => p.id === playlistId);
  if (!pl) return;
  pl.track_ids = rawIds([...orderedIds]);
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
  const moving = new Set(unique);
  if (removeFromSource && sourcePlaylistId != null && sourcePlaylistId !== toPlaylistId) {
    const source = library.playlists.find(p => p.id === sourcePlaylistId);
    if (source) {
      source.track_ids = rawIds(source.track_ids.filter(id => !moving.has(id)));
    }
  }

  const withoutDupes = target.track_ids.filter(id => !moving.has(id));
  const idx =
    insertIndex != null && insertIndex >= 0
      ? Math.min(insertIndex, withoutDupes.length)
      : withoutDupes.length;
  target.track_ids = rawIds([
    ...withoutDupes.slice(0, idx),
    ...unique,
    ...withoutDupes.slice(idx),
  ]);
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
    library.usbLastSync = null;
    library.activeVolume = null;

    await loadState();
    library.statusMessage = 'Library reset — empty';
  } catch (err) {
    library.statusMessage = 'Library reset failed';
    console.error('resetLibraryDatabase failed:', err);
    throw err;
  }
}


