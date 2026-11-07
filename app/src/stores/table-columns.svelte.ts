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
import { library, libraryChangesSince, libraryMembersOf, libraryPlainRows, libraryPositionsOf, libraryRowsChangedSince, onTrackListLoaded, plainRowOf, playlistByName } from './library.svelte.ts';
import { extendValueClasses, moveChangedValues, orderFromClasses, sortedOrder, sortedSubset, valueClasses, valueClassesSteps, withChangedValues, type SortedOrder, type SortValue, type ValueClasses } from '../lib/sortOrder.ts';

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
  const members = key === 'fav' ? null : libraryMembersOf(tracks);
  const byRank = members && sortByColumnRank(tracks, members, key, dir);
  if (byRank) return byRank;
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

interface ColumnSort {
  tracks: object;
  values: SortValue[];
  classes: ValueClasses | null | undefined;
  orders: Map<1 | -1, HeldOrder>;
}

interface HeldOrder {
  sorted: SortedOrder;
  tracks: object;
}

const columnSorts = new Map<string, ColumnSort>();

const UNIQUE_VALUE_KEYS = new Set(['title', 'index']);

function readLibraryValues(
  rows: readonly Record<string, unknown>[],
  plains: readonly Record<string, any>[],
  key: string,
  positions: readonly number[] | null,
  into: SortValue[],
  from = 0,
  to = rows.length,
  memo: Map<unknown, SortValue> | null = UNIQUE_VALUE_KEYS.has(key) ? null : new Map(),
) {
  const aligned = plains.length === rows.length;
  const read = (p: number) => {
    const plain = aligned ? plains[p] : plainRowOf(rows[p]);
    const raw = Object.hasOwn(plain, key) ? plain[key] : rows[p][key];
    if (!memo || (typeof raw !== 'string' && typeof raw !== 'number')) {
      into[p] = sortValueOfRaw(raw, key);
      return;
    }
    let value = memo.get(raw);
    if (value === undefined) {
      value = sortValueOfRaw(raw, key);
      memo.set(raw, value);
    }
    into[p] = value;
  };
  untrack(() => {
    if (positions) for (let i = 0; i < positions.length; i++) read(positions[i]);
    else for (let p = from; p < to; p++) read(p);
  });
}

function dropUnusableColumnSorts() {
  for (const [key, entry] of columnSorts) if (libraryChangesSince(entry.tracks) === null) columnSorts.delete(key);
}

function columnSortOf(rows: readonly Record<string, unknown>[], key: string): ColumnSort {
  let entry = columnSorts.get(key);
  if (entry && entry.tracks !== rows) {
    const changes = libraryChangesSince(entry.tracks);
    if (changes) {
      const known = entry.values.length;
      const plains = libraryPlainRows();
      const changed = changes.grown ? changes.positions.filter((position) => position < known) : changes.positions;
      if (changed.length > 0) readLibraryValues(rows, plains, key, changed, entry.values);
      if (changes.grown) {
        readLibraryValues(rows, plains, key, null, entry.values, known, rows.length);
        if (changed.length > 0) entry.classes = undefined;
        else if (entry.classes) entry.classes = extendValueClasses(entry.classes, entry.values, known);
      } else if (changed.length > 0) {
        entry.classes = undefined;
      }
      entry.tracks = rows;
    } else {
      dropUnusableColumnSorts();
      entry = undefined;
    }
  }
  if (!entry) {
    const values = new Array<SortValue>(rows.length);
    readLibraryValues(rows, libraryPlainRows(), key, null, values);
    entry = { tracks: rows, values, classes: undefined, orders: new Map() };
    columnSorts.set(key, entry);
  }
  return entry;
}

function orderOf(entry: ColumnSort, direction: 1 | -1): SortedOrder {
  const held = entry.orders.get(direction);
  if (held) {
    if (held.tracks === entry.tracks) return held.sorted;
    const changes = libraryChangesSince(held.tracks);
    const grown = entry.values.length > held.sorted.order.length;
    const inPlace = changes && !grown && (changes.positions.length === 0 || moveChangedValues(held.sorted, entry.values, changes.positions, direction));
    const moved = changes && (inPlace ? held.sorted : withChangedValues(held.sorted, entry.values, changes.positions, direction));
    if (moved) {
      held.sorted = moved;
      held.tracks = entry.tracks;
      return moved;
    }
    entry.orders.delete(direction);
  }
  if (entry.classes === undefined) entry.classes = valueClasses(entry.values);
  const sorted = entry.classes ? orderFromClasses(entry.classes, direction) : sortedOrder(entry.values, direction);
  entry.orders.set(direction, { sorted, tracks: entry.tracks });
  return sorted;
}

function sortLibraryRows<T extends Record<string, unknown>>(tracks: T[], positions: readonly number[] | 'all', key: string, dir: 1 | -1): T[] {
  const rows = untrack(() => library.tracks) as T[];
  const sorted = orderOf(columnSortOf(rows, key), dir);
  const order = positions === 'all' ? sorted.order : sortedSubset(positions, sorted);
  const out = new Array<T>(order.length);
  for (let i = 0; i < order.length; i++) out[i] = rows[order[i]];
  return out;
}

let slotOfRank = new Int32Array(0);

function sortByColumnRank<T extends Record<string, unknown>>(tracks: T[], members: Int32Array, key: string, dir: 1 | -1): T[] | null {
  const rows = untrack(() => library.tracks) as T[];
  const known = columnSorts.get(key);
  const reusable = known !== undefined && libraryChangesSince(known.tracks) !== null;
  if (!reusable && members.length * 8 < rows.length) return null;
  const entry = columnSortOf(rows, key);
  const sorted = orderOf(entry, dir);
  const count = members.length;
  if (slotOfRank.length < rows.length) slotOfRank = new Int32Array(rows.length);
  const ranks = new Int32Array(count);
  for (let i = 0; i < count; i++) {
    const rank = sorted.rank[members[i]];
    ranks[i] = rank;
    slotOfRank[rank] = i;
  }
  ranks.sort();
  const slots = new Int32Array(count);
  const values = entry.values;
  let from = 0;
  while (from < count) {
    const value = values[sorted.order[ranks[from]]];
    let to = from + 1;
    while (to < count && values[sorted.order[ranks[to]]] === value) to++;
    for (let k = from; k < to; k++) slots[k] = slotOfRank[ranks[k]];
    if (to - from > 1) slots.subarray(from, to).sort();
    from = to;
  }
  const out = new Array<T>(count);
  for (let k = 0; k < count; k++) out[k] = tracks[slots[k]];
  return out;
}

function* refreshColumn(key: string): Generator<void, ColumnSort> {
  const rows = untrack(() => library.tracks) as Record<string, unknown>[];
  const directions = [...(columnSorts.get(key)?.orders.keys() ?? [])];
  const entry = columnSortOf(rows, key);
  yield;
  for (const direction of directions) {
    orderOf(entry, direction);
    yield;
  }
  return entry;
}

const WARMUP_DELAY_MS = 1000;
const WARMUP_SLICE_MS = 6;
const WARMUP_FALLBACK_GAP_MS = 12;
const WARMUP_READ_CHUNK = 2048;
const WARMUP_SORT_CHUNK = 4096;
const NOT_WARMED_COLUMNS = new Set(['status', 'fav', 'wave', 'cover', 'index']);

interface WarmupChunks {
  read: number;
  sort: number;
}

function* warmColumn(key: string, rows: readonly Record<string, unknown>[], plains: readonly Record<string, any>[], chunks: WarmupChunks): Generator<void, ColumnSort> {
  const values = new Array<SortValue>(rows.length);
  const memo = UNIQUE_VALUE_KEYS.has(key) ? null : new Map<unknown, SortValue>();
  for (let from = 0; from < rows.length; from += chunks.read) {
    readLibraryValues(rows, plains, key, null, values, from, Math.min(rows.length, from + chunks.read), memo);
    yield;
  }
  const classes = yield* valueClassesSteps(values, chunks.sort);
  const entry: ColumnSort = { tracks: rows, values, classes, orders: new Map() };
  if (classes) {
    entry.orders.set(1, { sorted: orderFromClasses(classes, 1), tracks: rows });
    yield;
    entry.orders.set(-1, { sorted: orderFromClasses(classes, -1), tracks: rows });
  }
  return entry;
}

/**
 * Sort orders of the visible library columns, built a few milliseconds at a time (`step`) so the first click on a
 * header finds them ready. Each result is kept only if nothing built that column meanwhile and the list was at most
 * bumped since; `step` returns true when all columns are done.
 */
export function startLibrarySortWarmup(keys: readonly string[], chunks: WarmupChunks = { read: WARMUP_READ_CHUNK, sort: WARMUP_SORT_CHUNK }): { step(budgetMs: number): boolean } {
  const rows = untrack(() => library.tracks) as Record<string, unknown>[];
  const plains = libraryPlainRows();
  dropUnusableColumnSorts();
  const aligned = rows.length === plains.length;
  const queue = aligned ? keys.filter((key) => !columnSorts.has(key)) : [];
  const stale = aligned ? keys.filter((key) => columnSorts.has(key) && columnSorts.get(key)!.tracks !== rows) : [];
  let current: { key: string; job: Generator<void, ColumnSort> } | null = null;
  return {
    step(budgetMs) {
      const end = performance.now() + budgetMs;
      do {
        if (!current) {
          const staleKey = stale.shift();
          if (staleKey !== undefined) {
            current = { key: staleKey, job: refreshColumn(staleKey) };
            continue;
          }
          const key = queue.shift();
          if (key === undefined) return true;
          current = { key, job: warmColumn(key, rows, plains, chunks) };
        }
        const next = current.job.next();
        if (!next.done) continue;
        if (!columnSorts.has(current.key) && libraryChangesSince(next.value.tracks) !== null) columnSorts.set(current.key, next.value);
        current = null;
      } while (performance.now() < end);
      return false;
    },
  };
}

let warmupTimer: ReturnType<typeof setTimeout> | undefined;
let warmupRun = 0;

function warmableLibraryColumns(): string[] {
  return untrack(() => visibleColumns('library')).map((column) => column.key).filter((key) => !NOT_WARMED_COLUMNS.has(key));
}

function scheduleLibrarySortWarmup() {
  clearTimeout(warmupTimer);
  const run = ++warmupRun;
  warmupTimer = setTimeout(() => {
    const warmup = startLibrarySortWarmup(warmableLibraryColumns());
    const next = () => {
      if (run !== warmupRun) return;
      if (typeof requestIdleCallback === 'function') requestIdleCallback(slice, { timeout: 4000 });
      else setTimeout(slice, WARMUP_FALLBACK_GAP_MS);
    };
    const slice = () => {
      if (run !== warmupRun) return;
      if (!warmup.step(WARMUP_SLICE_MS)) next();
    };
    next();
  }, WARMUP_DELAY_MS);
}

onTrackListLoaded(scheduleLibrarySortWarmup);

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
