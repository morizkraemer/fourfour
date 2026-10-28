import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  initTableColumns,
  moveColumn,
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
