/**
 * table-columns.svelte.ts — Column order, visibility, width, and sort state.
 * Persisted to localStorage under fourfour.tableColumns.v1
 */

const STORAGE_KEY = 'fourfour.tableColumns.v1';

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

const DEFAULT_COLUMNS: TableColumn[] = [
  { key: 'fav', label: 'FAV', width: 20 },
  { key: 'index', label: '#', width: 28 },
  { key: 'wave', label: 'PREVIEW', width: 120 },
  { key: 'cover', label: '', width: 18 },
  { key: 'title', label: 'TITLE', flex: true, minWidth: 160 },
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

export const tableColumns = $state({
  columns: DEFAULT_COLUMNS.map(c => ({ ...c })) as TableColumn[],
  sortKey: null as string | null,
  sortDir: 'asc' as 'asc' | 'desc',
});

export function initTableColumns() {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return;
    const saved = JSON.parse(raw);
    if (Array.isArray(saved.columns)) {
      tableColumns.columns = mergeSavedColumns(saved.columns);
    }
    if (saved.sortKey) tableColumns.sortKey = saved.sortKey;
    if (saved.sortDir === 'asc' || saved.sortDir === 'desc') {
      tableColumns.sortDir = saved.sortDir;
    }
    syncSortIndicators();
  } catch {
    /* ignore corrupt prefs */
  }
}

function syncSortIndicators() {
  for (const col of tableColumns.columns) {
    if (col.key === tableColumns.sortKey) {
      col.sort = tableColumns.sortDir;
    } else {
      delete col.sort;
    }
  }
  tableColumns.columns = [...tableColumns.columns];
}

function mergeSavedColumns(saved: TableColumn[]): TableColumn[] {
  const byKey = new Map(saved.map(c => [c.key, c]));
  const ordered: TableColumn[] = [];
  for (const s of saved) {
    const def = DEFAULT_COLUMNS.find(d => d.key === s.key)
      ?? extraColumnDef(s.key);
    if (def) {
      const merged = { ...def, ...s, hidden: s.hidden ?? false };
      if (merged.flex && merged.minWidth == null && def.minWidth != null) {
        merged.minWidth = def.minWidth;
      }
      ordered.push(merged);
    }
  }
  for (const def of DEFAULT_COLUMNS) {
    if (!byKey.has(def.key)) ordered.push({ ...def });
  }
  for (const extra of ALL_COLUMN_KEYS) {
    if (!ordered.some(c => c.key === extra.key) && byKey.has(extra.key)) {
      ordered.push({ ...extraColumnDef(extra.key)!, ...byKey.get(extra.key)! });
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
      JSON.stringify({
        columns: tableColumns.columns,
        sortKey: tableColumns.sortKey,
        sortDir: tableColumns.sortDir,
      })
    );
  } catch {
    /* quota / private mode */
  }
}

export function visibleColumns(): TableColumn[] {
  return tableColumns.columns.filter(c => !c.hidden);
}

export function moveColumn(dragKey: string, targetKey: string) {
  const cols = tableColumns.columns;
  const from = cols.findIndex(c => c.key === dragKey);
  const to = cols.findIndex(c => c.key === targetKey);
  if (from < 0 || to < 0 || from === to) return;
  const [item] = cols.splice(from, 1);
  cols.splice(to, 0, item);
  tableColumns.columns = [...cols];
  persist();
}

export function setColumnHidden(key: string, hidden: boolean) {
  const col = tableColumns.columns.find(c => c.key === key);
  if (!col) {
    const def = extraColumnDef(key);
    if (!def) return;
    tableColumns.columns.push({ ...def, hidden });
  } else {
    col.hidden = hidden;
  }
  tableColumns.columns = [...tableColumns.columns];
  persist();
}

export function toggleColumnVisibility(key: string) {
  let col = tableColumns.columns.find(c => c.key === key);
  if (!col) {
    const def = extraColumnDef(key);
    if (!def) return;
    col = { ...def, hidden: false };
    tableColumns.columns.push(col);
  } else {
    col.hidden = !col.hidden;
  }
  tableColumns.columns = [...tableColumns.columns];
  persist();
}

export function setColumnWidth(key: string, width: number) {
  const col = tableColumns.columns.find(c => c.key === key);
  if (!col) return;
  const min = minWidthForKey(key);
  const w = Math.round(Math.max(min, width));
  if (col.flex) {
    col.minWidth = w;
  } else {
    col.width = w;
  }
  tableColumns.columns = [...tableColumns.columns];
  persist();
}

function minWidthForKey(key: string): number {
  const mins: Record<string, number> = {
    fav: 20,
    index: 28,
    wave: 72,
    cover: 18,
    title: 96,
    artist: 72,
    label: 64,
    album: 72,
    genre: 64,
    year: 40,
    bpm: 44,
    key: 32,
    time: 44,
  };
  return mins[key] ?? 24;
}

export function resetColumns() {
  tableColumns.columns = DEFAULT_COLUMNS.map(c => ({ ...c }));
  tableColumns.sortKey = null;
  tableColumns.sortDir = 'asc';
  persist();
}

export function setSort(key: string) {
  if (tableColumns.sortKey === key) {
    tableColumns.sortDir = tableColumns.sortDir === 'asc' ? 'desc' : 'asc';
  } else {
    tableColumns.sortKey = key;
    tableColumns.sortDir = 'asc';
  }
  syncSortIndicators();
  persist();
}

export function clearSort() {
  tableColumns.sortKey = null;
  tableColumns.sortDir = 'asc';
  syncSortIndicators();
  persist();
}

/** Sort track rows for display (does not mutate library order). */
export function sortTracks<T extends Record<string, unknown>>(tracks: T[]): T[] {
  const key = tableColumns.sortKey;
  if (!key) return tracks;
  const dir = tableColumns.sortDir === 'asc' ? 1 : -1;
  return [...tracks].sort((a, b) => {
    const av = sortValue(a, key);
    const bv = sortValue(b, key);
    if (av < bv) return -1 * dir;
    if (av > bv) return 1 * dir;
    return 0;
  });
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
