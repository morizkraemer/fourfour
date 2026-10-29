import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  initTableColumns,
  moveColumn,
  resetColumns,
  ALL_COLUMN_KEYS,
  tableLayouts,
  toggleColumnVisibility,
  visibleColumns,
} from './table-columns.svelte.ts';

const keys = (kind: 'library' | 'organize') => visibleColumns(kind).map((c) => c.key);

let storage: Record<string, string>;

beforeEach(() => {
  storage = {};
  vi.stubGlobal('localStorage', {
    getItem: (k: string) => storage[k] ?? null,
    setItem: (k: string, v: string) => {
      storage[k] = v;
    },
  });
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('moveColumn', () => {
  it('moves a shown extra column before another', () => {
    toggleColumnVisibility('library', 'album');
    expect(keys('library').at(-1)).toBe('album');
    moveColumn('library', 'album', 'title', true);
    expect(keys('library').indexOf('album')).toBe(keys('library').indexOf('title') - 1);
  });

  it('moves the cover column and persists the order', () => {
    resetColumns('library');
    moveColumn('library', 'cover', 'artist', false);
    const order = keys('library');
    expect(order.indexOf('cover')).toBe(order.indexOf('artist') + 1);
    expect(JSON.parse(storage['fourfour.tableColumns.v3']).layouts.library.columns.map((c: { key: string }) => c.key)).toContain('cover');
    moveColumn('library', 'artist', 'cover', true);
    expect(keys('library').indexOf('artist')).toBe(keys('library').indexOf('cover') - 1);
  });

  it('moves a column after the last one', () => {
    moveColumn('organize', 'album', 'key', false);
    expect(keys('organize').at(-1)).toBe('album');
  });

  it('moves another column next to album', () => {
    moveColumn('organize', 'album', 'cover', true);
    moveColumn('organize', 'bpm', 'album', true);
    expect(keys('organize').slice(0, 4).join()).toBe('index,bpm,album,cover');
  });

  it('persists the order and restores it on init', () => {
    moveColumn('organize', 'album', 'index', true);
    const expected = keys('organize');
    expect(expected[0]).toBe('album');
    expect(JSON.parse(storage['fourfour.tableColumns.v3']).layouts.organize.columns[0].key).toBe('album');

    tableLayouts.organize = { ...tableLayouts.organize, columns: [] };
    initTableColumns();
    expect(keys('organize')).toEqual(expected);
  });
});

describe('imported rating and colour columns', () => {
  it('are offered in the column menu but hidden by default', () => {
    expect(ALL_COLUMN_KEYS.map((c) => c.key)).toEqual(expect.arrayContaining(['rating', 'color']));
    resetColumns('library');
    expect(keys('library')).not.toContain('rating');
    expect(keys('library')).not.toContain('color');
  });

  it('show up once toggled on', () => {
    resetColumns('library');
    toggleColumnVisibility('library', 'rating');
    toggleColumnVisibility('library', 'color');
    expect(keys('library').slice(-2)).toEqual(['rating', 'color']);
  });
});

describe('file status column', () => {
  it('is shown first by default and slots into an older saved layout before fav', async () => {
    resetColumns('library');
    expect(keys('library')[0]).toBe('status');

    vi.resetModules();
    storage['fourfour.tableColumns.v3'] = JSON.stringify({
      layouts: { library: { columns: [{ key: 'fav' }, { key: 'title' }, { key: 'time' }], sortKey: null, sortDir: 'asc' } },
    });
    const fresh = await import('./table-columns.svelte.ts');
    fresh.initTableColumns();
    expect(fresh.visibleColumns('library').map((c) => c.key).slice(0, 2)).toEqual(['status', 'fav']);
  });
});
