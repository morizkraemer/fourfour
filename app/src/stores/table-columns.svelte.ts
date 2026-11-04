/**
 * table-columns.svelte.ts — Column layouts per list context.
 * Persisted to localStorage under fourfour.tableColumns.v3
 *
 * Layout kinds:
 *   library  — All Tracks, Recently Added
 *   playlist — named playlists
 *   usb      — mounted USB volumes
 *   organize — right-panel organize table (independent persistent layout)
 */

import {
  minWidthForColumn,
  measureColumnContentWidth,
} from '../../../prototyping/viewport/design-system/utils/column-layout.js';
import { labelColorRank } from '../../../prototyping/viewport/design-system/utils/label-color.js';
import { untrack } from 'svelte';
import { library, libraryPlainRows, libraryPositionsChangedSince, libraryPositionsOf, libraryRowsChangedSince, plainRowOf, playlistByName } from './library.svelte.ts';
import { sortedOrder, sortedSubset, withChangedValues, type SortedOrder, type SortValue } from '../lib/sortOrder.ts';

const STORAGE_KEY = 'fourfour.tableColumns.v3';
const LEGACY_STORAGE_KEYS = ['fourfour.tableColumns.v2', 'fourfour.tableColumns.v1'];

export type LayoutKind = 'library' | 'playlist' | 'usb' | 'organize';

export const LAYOUT_KINDS: LayoutKind[] = ['library', 'playlist', 'usb', 'organize'];

export type TableColumn = {
  key: string;
  label: string;
  width?: number;
  minWidth?: number;
  flex?: boolean;
  align?: 'right';
  sort?: 'asc' | 'desc';
  hidden?: boolean;
};

export type TableLayout = {
  columns: TableColumn[];
  sortKey: string | null;
  sortDir: 'asc' | 'desc';
};

const DEFAULT_COLUMNS: TableColumn[] = [
  { key: 'status', label: '', width: 20 },
  { key: 'fav', label: 'FAV', width: 20 },
  { key: 'index', label: '#', width: 28 },
  { key: 'wave', label: 'PREVIEW', width: 120 },
  { key: 'cover', label: '', width: 18 },
  { key: 'title', label: 'TITLE', width: 180 },
  { key: 'artist', label: 'ARTIST', width: 180 },
  { key: 'label', label: 'LABEL', width: 130 },
  { key: 'bpm', label: 'BPM', width: 52, align: 'right' },
  { key: 'key', label: 'KEY', width: 36, align: 'right' },
  { key: 'time', label: 'TIME', width: 48, align: 'right' },
];

/** Full catalog of columns users can show/hide (includes defaults + extras). */
export const ALL_COLUMN_KEYS: { key: string; label: string }[] = [
  { key: 'status', label: 'File status' },
  { key: 'fav', label: 'Favorites' },
  { key: 'index', label: '#' },
  { key: 'wave', label: 'Preview' },
  { key: 'cover', label: 'Cover' },
  { key: 'title', label: 'Title' },
  { key: 'artist', label: 'Artist' },
  { key: 'album', label: 'Album' },
  { key: 'label', label: 'Label' },
  { key: 'genre', label: 'Genre' },
  { key: 'year', label: 'Year' },
  { key: 'rating', label: 'Rating' },
  { key: 'color', label: 'Colour' },
  { key: 'bpm', label: 'BPM' },
  { key: 'key', label: 'Key' },
  { key: 'time', label: 'Time' },
];

function defaultLayout(): TableLayout {
  return {
    columns: DEFAULT_COLUMNS.map(c => normalizeColumn({ ...c })),
    sortKey: null,
    sortDir: 'asc',
  };
}

const ORGANIZE_DEFAULT_COLUMNS: TableColumn[] = [
  { key: 'index', label: '#', width: 28 },
  { key: 'cover', label: '', width: 18 },
  { key: 'title', label: 'TITLE', width: 180 },
  { key: 'album', label: 'ALBUM', width: 160 },
  { key: 'bpm', label: 'BPM', width: 52, align: 'right' },
  { key: 'key', label: 'KEY', width: 36, align: 'right' },
];

function organizeLayout(): TableLayout {
  return {
    columns: ORGANIZE_DEFAULT_COLUMNS.map(c => normalizeColumn({ ...c })),
    sortKey: null,
    sortDir: 'asc',
  };
}

function emptyLayouts(): Record<LayoutKind, TableLayout> {
  return {
    library: defaultLayout(),
    playlist: defaultLayout(),
    usb: defaultLayout(),
    organize: organizeLayout(),
  };
}

export const tableLayouts = $state(emptyLayouts());

/** Resolve which persisted layout profile applies to a list panel. */
export function resolveLayoutKind(
  sourceId: string | null | undefined,
): LayoutKind {
  const source = sourceId ?? 'All Tracks';
  if (library.volumes.includes(source)) return 'usb';
  if (playlistByName(source)) return 'playlist';
  return 'library';
}

export function getLayout(kind: LayoutKind): TableLayout {
  return tableLayouts[kind];
}

let initialized = false;

export function initTableColumns() {
  if (initialized) return;
  initialized = true;
  try {
    let raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) {
      for (const legacyKey of LEGACY_STORAGE_KEYS) {
        raw = localStorage.getItem(legacyKey);
        if (raw) break;
      }
    }
    if (!raw) return;

    const saved = JSON.parse(raw);
    if (saved.layouts && typeof saved.layouts === 'object') {
      for (const kind of LAYOUT_KINDS) {
        const entry = saved.layouts[kind];
        if (entry && Array.isArray(entry.columns)) {
          const fallback = kind === 'organize' ? ORGANIZE_DEFAULT_COLUMNS : DEFAULT_COLUMNS;
          tableLayouts[kind] = {
            columns: mergeSavedColumns(entry.columns, fallback),
            sortKey: entry.sortKey ?? null,
            sortDir: entry.sortDir === 'desc' ? 'desc' : 'asc',
          };
          syncSortIndicators(kind);
        }
      }
      persist();
      return;
    }

    // Migrate v1/v2 single global layout → seed library/playlist/usb from saved prefs.
    // 'organize' always starts fresh from its own default.
    if (Array.isArray(saved.columns)) {
      const migrated = {
        columns: mergeSavedColumns(saved.columns),
        sortKey: saved.sortKey ?? null,
        sortDir: saved.sortDir === 'desc' ? 'desc' : 'asc',
      } as TableLayout;
      for (const kind of LAYOUT_KINDS) {
        if (kind === 'organize') continue;
        tableLayouts[kind] = {
          columns: migrated.columns.map(c => ({ ...c })),
          sortKey: migrated.sortKey,
          sortDir: migrated.sortDir,
        };
        syncSortIndicators(kind);
      }
      persist();
    }
  } catch {
    /* ignore corrupt prefs */
  }
}

function syncSortIndicators(kind: LayoutKind) {
  const layout = tableLayouts[kind];
  for (const col of layout.columns) {
    if (col.key === layout.sortKey) {
      col.sort = layout.sortDir;
    } else {
      delete col.sort;
    }
  }
  layout.columns = [...layout.columns];
  tableLayouts[kind] = { ...layout };
}

function defaultWidthForKey(key: string): number {
  const def = DEFAULT_COLUMNS.find(d => d.key === key) ?? extraColumnDef(key);
  return def?.width ?? 100;
}

/** Flex columns only set a resize floor — convert to fixed width so drag actually changes size. */
function normalizeColumn(col: TableColumn): TableColumn {
  const floor = minWidthForColumn(col.key);
  const { flex: _flex, minWidth, ...rest } = col;
  const width = Math.max(
    floor,
    col.width ?? (col.flex ? minWidth : undefined) ?? defaultWidthForKey(col.key),
  );
  return { ...rest, width };
}

function mergeSavedColumns(
  saved: TableColumn[],
  fallbackColumns: TableColumn[] = DEFAULT_COLUMNS,
): TableColumn[] {
  const byKey = new Map(saved.map(c => [c.key, c]));
  const ordered: TableColumn[] = [];
  for (const s of saved) {
    const def = fallbackColumns.find(d => d.key === s.key)
      ?? DEFAULT_COLUMNS.find(d => d.key === s.key)
      ?? extraColumnDef(s.key);
    if (def) {
      ordered.push(normalizeColumn({
        ...def,
        ...s,
        hidden: s.hidden ?? false,
      }));
    }
  }
  fallbackColumns.forEach((def, i) => {
    if (byKey.has(def.key)) return;
    const nextKey = fallbackColumns.slice(i + 1).find(d => ordered.some(c => c.key === d.key))?.key;
    const at = nextKey ? ordered.findIndex(c => c.key === nextKey) : ordered.length;
    ordered.splice(at, 0, { ...def });
  });
  for (const extra of ALL_COLUMN_KEYS) {
    if (!ordered.some(c => c.key === extra.key) && byKey.has(extra.key)) {
      ordered.push(normalizeColumn({ ...extraColumnDef(extra.key)!, ...byKey.get(extra.key)! }));
    }
  }
  return ordered;
}

function extraColumnDef(key: string): TableColumn | null {
  const meta = ALL_COLUMN_KEYS.find(c => c.key === key);
  if (!meta) return null;
  const widths: Record<string, number> = {
    album: 160,
    genre: 100,
    year: 48,
    rating: 60,
    color: 36,
  };
  return {
    key: meta.key,
    label: meta.label.toUpperCase(),
    width: widths[key] ?? 100,
    hidden: true,
  };
}

let persistPending = false;
let persistOnPageHide = false;

function persistIfPending() {
  if (persistPending) persist();
}

/** A sort click writes the layout once the browser is idle, so the click's frame does not wait for the JSON and storage write. */
function persistSoon() {
  if (persistPending) return;
  persistPending = true;
  if (typeof requestIdleCallback === 'function') requestIdleCallback(persistIfPending, { timeout: 1000 });
  else setTimeout(persistIfPending, 200);
  if (!persistOnPageHide && typeof addEventListener === 'function') {
    persistOnPageHide = true;
    addEventListener('pagehide', persistIfPending);
  }
}

function persist() {
  persistPending = false;
  try {
    localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({ layouts: tableLayouts }),
    );
  } catch {
    /* quota / private mode */
  }
}

export function visibleColumns(kind: LayoutKind): TableColumn[] {
  return tableLayouts[kind].columns.filter(c => !c.hidden);
}

export function moveColumn(
  kind: LayoutKind,
  dragKey: string,
  targetKey: string,
  insertBefore = true,
) {
  const cols = tableLayouts[kind].columns;
  const from = cols.findIndex(c => c.key === dragKey);
  let to = cols.findIndex(c => c.key === targetKey);
  if (from < 0 || to < 0 || from === to) return;
  if (!insertBefore) to += 1;
  const [item] = cols.splice(from, 1);
  if (from < to) to -= 1;
  cols.splice(to, 0, item);
  tableLayouts[kind] = { ...tableLayouts[kind], columns: [...cols] };
  persist();
}

export function setColumnHidden(kind: LayoutKind, key: string, hidden: boolean) {
  const cols = tableLayouts[kind].columns;
  const col = cols.find(c => c.key === key);
  if (!col) {
    const def = extraColumnDef(key);
    if (!def) return;
    cols.push({ ...def, hidden });
  } else {
    col.hidden = hidden;
  }
  tableLayouts[kind] = { ...tableLayouts[kind], columns: [...cols] };
  persist();
}

export function toggleColumnVisibility(kind: LayoutKind, key: string) {
  const cols = tableLayouts[kind].columns;
  let col = cols.find(c => c.key === key);
  if (!col) {
    const def = extraColumnDef(key);
    if (!def) return;
    col = { ...def, hidden: false };
    cols.push(col);
  } else {
    col.hidden = !col.hidden;
  }
  tableLayouts[kind] = { ...tableLayouts[kind], columns: [...cols] };
  persist();
}

export function setColumnWidth(
  kind: LayoutKind,
  key: string,
  width: number,
  save = true,
) {
  const cols = tableLayouts[kind].columns;
  const col = cols.find(c => c.key === key);
  if (!col) return;
  const w = Math.round(Math.max(minWidthForColumn(key), width));
  delete col.flex;
  delete col.minWidth;
  col.width = w;
  tableLayouts[kind] = { ...tableLayouts[kind], columns: [...cols] };
  if (save) persist();
}

const TEXT_FALLBACK_COLUMNS = new Set(['key', 'album', 'genre', 'year']);

/** Widest cell text over every track, not just the rows the virtualised table has rendered. */
function measureAllRowsTextWidth(
  key: string,
  rootEl: HTMLElement | null,
  tracks: Record<string, unknown>[],
): number {
  const sample = rootEl?.querySelector(`.ff-trow__cell[data-col-key="${key}"] .ff-trow__text`);
  const ctx = document.createElement('canvas').getContext('2d');
  if (!sample || !ctx) return 0;
  const style = getComputedStyle(sample);
  ctx.font = `${style.fontWeight} ${style.fontSize} ${style.fontFamily}`;
  const fallback = TEXT_FALLBACK_COLUMNS.has(key) ? '—' : '';
  let max = 0;
  for (const track of tracks) {
    const text = String(track[key] ?? fallback).trim();
    if (text) max = Math.max(max, ctx.measureText(text).width);
  }
  return Math.ceil(max) + 10;
}

export function fitColumnToContent(
  kind: LayoutKind,
  key: string,
  rootEl: HTMLElement | null,
  tracks: Record<string, unknown>[] = [],
) {
  const width = Math.max(
    measureColumnContentWidth(key, rootEl, visibleColumns(kind)),
    measureAllRowsTextWidth(key, rootEl, tracks),
  );
  setColumnWidth(kind, key, width);
}

export function resetColumns(kind: LayoutKind) {
  tableLayouts[kind] = defaultLayout();
  persist();
}

export function setSort(kind: LayoutKind, key: string) {
  const layout = tableLayouts[kind];
  if (layout.sortKey === key) {
    layout.sortDir = layout.sortDir === 'asc' ? 'desc' : 'asc';
  } else {
    layout.sortKey = key;
    layout.sortDir = 'asc';
  }
  syncSortIndicators(kind);
  persistSoon();
}

export function clearSort(kind: LayoutKind) {
  tableLayouts[kind] = {
    ...tableLayouts[kind],
    sortKey: null,
    sortDir: 'asc',
  };
  syncSortIndicators(kind);
  persistSoon();
}

/** Sort track rows for display (does not mutate library order). Tracks without a value stay last in both directions. */
export function sortTracks<T extends Record<string, unknown>>(
  tracks: T[],
  kind: LayoutKind,
): T[] {
  const key = tableLayouts[kind].sortKey;
  if (!key) return tracks;
  const dir = tableLayouts[kind].sortDir === 'asc' ? 1 : -1;
  const positions = key === 'fav' ? null : libraryPositionsOf(tracks);
  if (positions) return sortLibraryRows(tracks, positions, key, dir);
  const keyed =
    key === 'fav'
      ? tracks.map((track) => ({ track, value: favSlot(track) }))
      : sortKeyed(tracks, sortValueReader(key));
  keyed.sort((a, b) => {
    if (a.value === null && b.value === null) return 0;
    if (a.value === null) return 1;
    if (b.value === null) return -1;
    if (a.value < b.value) return -1 * dir;
    if (a.value > b.value) return 1 * dir;
    return 0;
  });
  return keyed.map((entry) => entry.track);
}

let sortValuesVersion: { tracks: unknown; usbTracks: unknown } | null = null;
const sortValuesByKey = new Map<string, WeakMap<object, SortValue>>();

/**
 * Sort keys per track, read without tracking and kept until the track list is replaced: library tracks change their
 * fields only together with `bumpLibraryTracks()` (and USB tracks are replaced whole), which swaps the list this
 * cache is checked against; after a bump only the changed rows are read again. A tracked read of the key of every track made each sort depend on 10k track signals.
 * `fav` stays a tracked read: favorite slots change in place (`syncTrackFavoriteFields`).
 */
function sortValueReader(key: string): (track: Record<string, unknown>) => SortValue {
  const tracks = library.tracks;
  const usbTracks = library.usbTracks;
  if (!sortValuesVersion || sortValuesVersion.tracks !== tracks || sortValuesVersion.usbTracks !== usbTracks) {
    const changed = sortValuesVersion?.usbTracks === usbTracks ? libraryRowsChangedSince(sortValuesVersion.tracks as object) : null;
    if (changed) {
      for (const values of sortValuesByKey.values()) for (const row of changed) values.delete(row);
    } else {
      sortValuesByKey.clear();
    }
    sortValuesVersion = { tracks, usbTracks };
  }
  let values = sortValuesByKey.get(key);
  if (!values) {
    values = new WeakMap();
    sortValuesByKey.set(key, values);
  }
  const cache = values;
  return (track) => {
    let value = cache.get(track);
    if (value === undefined) {
      value = sortValue(track, key);
      cache.set(track, value);
    }
    return value;
  };
}

interface LibrarySorts {
  tracks: object;
  values: Map<string, SortValue[]>;
  orders: Map<string, SortedOrder>;
  from: { values: Map<string, SortValue[]>; orders: Map<string, SortedOrder>; changed: number[] } | null;
}

let librarySorts: LibrarySorts | null = null;

function libraryValues(rows: readonly Record<string, unknown>[], key: string, positions: readonly number[] | null, into: SortValue[]) {
  const plains = libraryPlainRows();
  const aligned = plains.length === rows.length;
  const read = (p: number) => {
    const plain = aligned ? plains[p] : plainRowOf(rows[p]);
    into[p] = sortValueOfRaw(Object.hasOwn(plain, key) ? plain[key] : rows[p][key], key);
  };
  untrack(() => {
    if (positions) for (let i = 0; i < positions.length; i++) read(positions[i]);
    else for (let p = 0; p < rows.length; p++) read(p);
  });
}

function librarySort(key: string, dir: 1 | -1): SortedOrder {
  const rows = untrack(() => library.tracks);
  if (librarySorts?.tracks !== rows) {
    const previous = librarySorts;
    const changed = previous ? libraryPositionsChangedSince(previous.tracks) : null;
    librarySorts = {
      tracks: rows,
      values: new Map(),
      orders: new Map(),
      from: previous && changed ? { values: previous.values, orders: previous.orders, changed } : null,
    };
  }
  const cache = librarySorts!;
  let values = cache.values.get(key);
  if (!values) {
    values = cache.from?.values.get(key);
    if (values) libraryValues(rows, key, cache.from!.changed, values);
    else {
      values = new Array<SortValue>(rows.length);
      libraryValues(rows, key, null, values);
    }
    cache.values.set(key, values);
  }
  const orderKey = `${key}|${dir}`;
  let sorted = cache.orders.get(orderKey);
  if (!sorted) {
    const earlier = cache.from?.values.get(key) === values ? cache.from.orders.get(orderKey) : undefined;
    sorted = (earlier && withChangedValues(earlier, values, cache.from!.changed, dir)) || sortedOrder(values, dir);
    cache.orders.set(orderKey, sorted);
  }
  return sorted;
}

function sortLibraryRows<T extends Record<string, unknown>>(tracks: T[], positions: readonly number[] | 'all', key: string, dir: 1 | -1): T[] {
  const rows = untrack(() => library.tracks) as T[];
  const sorted = librarySort(key, dir);
  const order = positions === 'all' ? sorted.order : sortedSubset(positions, sorted);
  const out = new Array<T>(order.length);
  for (let i = 0; i < order.length; i++) out[i] = rows[order[i]];
  return out;
}

function sortKeyed<T extends Record<string, unknown>>(tracks: T[], valueOf: (track: T) => SortValue) {
  return untrack(() => tracks.map((track) => ({ track, value: valueOf(track) })));
}

function favSlot(track: Record<string, unknown>): number | null {
  const n = track.fav;
  return typeof n === 'number' && n > 0 ? n : null;
}

function isBlank(text: string): boolean {
  return text === '' || text === '—' || text === 'undefined';
}

export function camelotRank(key: string): number | null {
  const match = /^(\d{1,2})([ABab])$/.exec(key.trim());
  if (!match) return null;
  return Number(match[1]) * 2 + (match[2].toUpperCase() === 'B' ? 1 : 0);
}

export function durationSeconds(time: string): number | null {
  const parts = time.trim().split(':');
  if (parts.length < 2 || parts.length > 3) return null;
  const numbers = parts.map(Number);
  if (numbers.some((n) => !Number.isFinite(n))) return null;
  return numbers.reduce((total, n) => total * 60 + n, 0);
}

function sortValue(track: Record<string, unknown>, key: string): string | number | null {
  const plain = plainRowOf(track);
  return sortValueOfRaw(Object.hasOwn(plain, key) ? plain[key] : track[key], key);
}

function sortValueOfRaw(raw: unknown, key: string): string | number | null {
  const text = String(raw ?? '').trim();
  if (isBlank(text)) return null;
  if (key === 'bpm') {
    const n = parseFloat(text);
    return Number.isFinite(n) ? n : null;
  }
  if (key === 'key') return camelotRank(text);
  if (key === 'time') return durationSeconds(text);
  if (key === 'rating') {
    const n = parseInt(text, 10);
    return n > 0 ? n : null;
  }
  if (key === 'color') return labelColorRank(text);
  if (key === 'index' || key === 'year') {
    const n = parseInt(text, 10);
    return Number.isFinite(n) ? n : null;
  }
  return text.toLowerCase();
}
