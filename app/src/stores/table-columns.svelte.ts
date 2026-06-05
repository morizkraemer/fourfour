/**
 * table-columns.svelte.ts — Column layouts per list context.
 * Persisted to localStorage under fourfour.tableColumns.v3
 *
 * Layout kinds:
 *   library  — All Tracks, Recently Added
 *   playlist — named playlists
 *   usb      — mounted USB volumes
 */

import {
  minWidthForColumn,
  measureColumnContentWidth,
} from '../../../prototyping/viewport/design-system/utils/column-layout.js';
import { library, playlistByName } from './library.svelte.ts';

const STORAGE_KEY = 'fourfour.tableColumns.v3';
const LEGACY_STORAGE_KEYS = ['fourfour.tableColumns.v2', 'fourfour.tableColumns.v1'];

export type LayoutKind = 'library' | 'playlist' | 'usb';

export const LAYOUT_KINDS: LayoutKind[] = ['library', 'playlist', 'usb'];

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

function emptyLayouts(): Record<LayoutKind, TableLayout> {
  return {
    library: defaultLayout(),
    playlist: defaultLayout(),
    usb: defaultLayout(),
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
          tableLayouts[kind] = {
            columns: mergeSavedColumns(entry.columns),
            sortKey: entry.sortKey ?? null,
            sortDir: entry.sortDir === 'desc' ? 'desc' : 'asc',
          };
          syncSortIndicators(kind);
        }
      }
      persist();
      return;
    }

    // Migrate v1/v2 single global layout → seed all kinds from the same saved prefs.
    if (Array.isArray(saved.columns)) {
      const migrated = {
        columns: mergeSavedColumns(saved.columns),
        sortKey: saved.sortKey ?? null,
        sortDir: saved.sortDir === 'desc' ? 'desc' : 'asc',
      } as TableLayout;
      for (const kind of LAYOUT_KINDS) {
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

function mergeSavedColumns(saved: TableColumn[]): TableColumn[] {
  const byKey = new Map(saved.map(c => [c.key, c]));
  const ordered: TableColumn[] = [];
  for (const s of saved) {
    const def = DEFAULT_COLUMNS.find(d => d.key === s.key)
      ?? extraColumnDef(s.key);
    if (def) {
      ordered.push(normalizeColumn({
        ...def,
        ...s,
        hidden: s.hidden ?? false,
      }));
    }
  }
  for (const def of DEFAULT_COLUMNS) {
    if (!byKey.has(def.key)) ordered.push({ ...def });
  }
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
  };
  return {
    key: meta.key,
    label: meta.label.toUpperCase(),
    width: widths[key] ?? 100,
    hidden: true,
  };
}

function persist() {
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

export function fitColumnToContent(
  kind: LayoutKind,
  key: string,
  rootEl: HTMLElement | null,
) {
  const width = measureColumnContentWidth(key, rootEl, visibleColumns(kind));
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
  persist();
}

export function clearSort(kind: LayoutKind) {
  tableLayouts[kind] = {
    ...tableLayouts[kind],
    sortKey: null,
    sortDir: 'asc',
  };
  syncSortIndicators(kind);
  persist();
}

/** Sort track rows for display (does not mutate library order). */
export function sortTracks<T extends Record<string, unknown>>(
  tracks: T[],
  kind: LayoutKind,
): T[] {
  const key = tableLayouts[kind].sortKey;
  if (!key) return tracks;
  const dir = tableLayouts[kind].sortDir === 'asc' ? 1 : -1;
  return [...tracks].sort((a, b) => {
    if (key === 'fav') return compareFav(a, b, dir);
    const av = sortValue(a, key);
    const bv = sortValue(b, key);
    if (av < bv) return -1 * dir;
    if (av > bv) return 1 * dir;
    return 0;
  });
}

function favSlot(track: Record<string, unknown>): number | null {
  const n = track.fav;
  return typeof n === 'number' && n > 0 ? n : null;
}

function compareFav(
  a: Record<string, unknown>,
  b: Record<string, unknown>,
  dir: number,
): number {
  const af = favSlot(a);
  const bf = favSlot(b);
  if (af === null && bf === null) return 0;
  if (af === null) return 1;
  if (bf === null) return -1;
  if (af < bf) return -1 * dir;
  if (af > bf) return 1 * dir;
  return 0;
}

function sortValue(track: Record<string, unknown>, key: string): string | number {
  const v = track[key];
  if (key === 'bpm') {
    const n = parseFloat(String(v).replace('—', ''));
    return Number.isFinite(n) ? n : -1;
  }
  if (key === 'index') {
    return parseInt(String(v), 10) || 0;
  }
  return String(v ?? '').toLowerCase();
}
