import { beforeEach, describe, expect, it } from 'vitest';
import { camelotRank, clearSort, durationSeconds, setSort, sortTracks, tableLayouts } from './table-columns.svelte.ts';

type Row = Record<string, unknown>;

const rows: Row[] = [
  { id: 1, title: 'Bravo', artist: 'zed', bpm: '128.0', key: '10A', time: '10:05', index: '01', year: '2019', fav: 2 },
  { id: 2, title: 'alpha', artist: 'Amy', bpm: '95.5', key: '2B', time: '3:30', index: '02', year: '2001' },
  { id: 3, title: 'Charlie', artist: '—', bpm: '—', key: '—', time: '—', index: '03', year: '—', fav: 1 },
  { id: 4, title: 'delta', artist: 'Bob', bpm: '174.0', key: '2A', time: '62:00', index: '04', year: '2020' },
  { id: 5, title: 'echo', artist: 'Cy', bpm: '100.0', key: '9B', time: '0:59', index: '05', year: '1999' },
];

const idsSortedBy = (key: string, dir: 'asc' | 'desc') => {
  clearSort('library');
  setSort('library', key);
  if (dir === 'desc') setSort('library', key);
  return sortTracks(rows, 'library').map((r) => r.id);
};

beforeEach(() => clearSort('library'));

describe('setSort', () => {
  it('first click sorts ascending, second descending, other column restarts ascending', () => {
    setSort('library', 'bpm');
    expect([tableLayouts.library.sortKey, tableLayouts.library.sortDir]).toEqual(['bpm', 'asc']);
    setSort('library', 'bpm');
    expect(tableLayouts.library.sortDir).toBe('desc');
    setSort('library', 'title');
    expect([tableLayouts.library.sortKey, tableLayouts.library.sortDir]).toEqual(['title', 'asc']);
  });

  it('marks only the active column with its direction', () => {
    setSort('library', 'key');
    const marked = tableLayouts.library.columns.filter((c) => c.sort);
    expect(marked.map((c) => [c.key, c.sort])).toEqual([['key', 'asc']]);
  });

  it('keeps each layout kind independent', () => {
    setSort('playlist', 'time');
    expect(tableLayouts.library.sortKey).toBeNull();
    clearSort('playlist');
  });
});

describe('sortTracks', () => {
  it('returns the input untouched without a sort key', () => {
    expect(sortTracks(rows, 'library')).toBe(rows);
  });

  it('sorts text case-insensitively', () => {
    expect(idsSortedBy('title', 'asc')).toEqual([2, 1, 3, 4, 5]);
    expect(idsSortedBy('title', 'desc')).toEqual([5, 4, 3, 1, 2]);
  });

  it('sorts BPM numerically with blanks last in both directions', () => {
    expect(idsSortedBy('bpm', 'asc')).toEqual([2, 5, 1, 4, 3]);
    expect(idsSortedBy('bpm', 'desc')).toEqual([4, 1, 5, 2, 3]);
  });

  it('sorts keys in Camelot order, A before B, numbers numerically', () => {
    expect(idsSortedBy('key', 'asc')).toEqual([4, 2, 5, 1, 3]);
    expect(idsSortedBy('key', 'desc')).toEqual([1, 5, 2, 4, 3]);
  });

  it('sorts time by duration, not text', () => {
    expect(idsSortedBy('time', 'asc')).toEqual([5, 2, 1, 4, 3]);
    expect(idsSortedBy('time', 'desc')).toEqual([4, 1, 2, 5, 3]);
  });

  it('sorts the number column and year numerically', () => {
    expect(idsSortedBy('index', 'desc')).toEqual([5, 4, 3, 2, 1]);
    expect(idsSortedBy('year', 'asc')).toEqual([5, 2, 1, 4, 3]);
  });

  it('puts the lowest favorite slot first and unmarked tracks last', () => {
    expect(idsSortedBy('fav', 'asc').slice(0, 2)).toEqual([3, 1]);
  });

  it('does not mutate the input order', () => {
    const before = rows.map((r) => r.id);
    idsSortedBy('bpm', 'desc');
    expect(rows.map((r) => r.id)).toEqual(before);
  });
});

describe('parsers', () => {
  it('ranks Camelot keys and rejects the rest', () => {
    expect(camelotRank('1A')).toBeLessThan(camelotRank('1B')!);
    expect(camelotRank('1B')).toBeLessThan(camelotRank('2A')!);
    expect(camelotRank('12B')).toBeGreaterThan(camelotRank('9B')!);
    expect(camelotRank('C#m')).toBeNull();
  });

  it('reads m:ss and h:mm:ss', () => {
    expect(durationSeconds('3:05')).toBe(185);
    expect(durationSeconds('1:02:03')).toBe(3723);
    expect(durationSeconds('—')).toBeNull();
  });
});
