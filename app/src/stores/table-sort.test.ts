import { beforeEach, describe, expect, it } from 'vitest';
import { labelColorRank } from '../../../prototyping/viewport/design-system/utils/label-color.js';
import { camelotRank, clearSort, durationSeconds, setSort, sortTracks, tableLayouts } from './table-columns.svelte.ts';

type Row = Record<string, unknown>;

const rows: Row[] = [
  { id: 1, title: 'Bravo', artist: 'zed', bpm: '128.0', key: '10A', time: '10:05', index: '01', year: '2019', fav: 2, rating: 3, color: 'Blue' },
  { id: 2, title: 'alpha', artist: 'Amy', bpm: '95.5', key: '2B', time: '3:30', index: '02', year: '2001', rating: 5, color: 'Pink' },
  { id: 3, title: 'Charlie', artist: '—', bpm: '—', key: '—', time: '—', index: '03', year: '—', fav: 1 },
  { id: 4, title: 'delta', artist: 'Bob', bpm: '174.0', key: '2A', time: '62:00', index: '04', year: '2020', rating: 0, color: null },
  { id: 5, title: 'echo', artist: 'Cy', bpm: '100.0', key: '9B', time: '0:59', index: '05', year: '1999', rating: 1, color: '#112233' },
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

  it('sorts rating and colour with unrated and uncoloured tracks last', () => {
    expect(idsSortedBy('rating', 'asc')).toEqual([5, 1, 2, 3, 4]);
    expect(idsSortedBy('rating', 'desc')).toEqual([2, 1, 5, 3, 4]);
    expect(idsSortedBy('color', 'asc')).toEqual([2, 1, 5, 3, 4]);
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

describe('sortTracks at 10k against the reference sort', () => {
  const blank = (text: string) => text === '' || text === '—' || text === 'undefined';
  function referenceValue(track: Row, key: string): string | number | null {
    if (key === 'fav') return typeof track.fav === 'number' && track.fav > 0 ? track.fav : null;
    const text = String(track[key] ?? '').trim();
    if (blank(text)) return null;
    if (key === 'bpm') return Number.isFinite(parseFloat(text)) ? parseFloat(text) : null;
    if (key === 'key') return camelotRank(text);
    if (key === 'time') return durationSeconds(text);
    if (key === 'rating') return parseInt(text, 10) > 0 ? parseInt(text, 10) : null;
    if (key === 'color') return labelColorRank(text);
    if (key === 'index' || key === 'year') return Number.isFinite(parseInt(text, 10)) ? parseInt(text, 10) : null;
    return text.toLowerCase();
  }
  function referenceSort(tracks: Row[], key: string, dir: 1 | -1) {
    const keyed = tracks.map((track) => ({ track, value: referenceValue(track, key) }));
    keyed.sort((a, b) => {
      if (a.value === null && b.value === null) return 0;
      if (a.value === null) return 1;
      if (b.value === null) return -1;
      if (a.value < b.value) return -dir;
      if (a.value > b.value) return dir;
      return 0;
    });
    return keyed.map((e) => e.track.id);
  }

  const WORDS = ['Ölbaum', 'ölbaum', 'Zebra', 'zebra', 'Æon', 'éclair', 'Eclair', 'Straße', 'STRASSE', '東京', '🎧 Mix', '  padded ', '—', '', '1999', 'a', 'A'];
  let seed = 7;
  const rand = (n: number) => (seed = (seed * 1103515245 + 12345) % 2 ** 31) % n;
  const big: Row[] = Array.from({ length: 10_000 }, (_, i) => ({
    id: i + 1,
    title: WORDS[rand(WORDS.length)] + (rand(3) === 0 ? '' : ` ${rand(40)}`),
    artist: rand(5) === 0 ? undefined : WORDS[rand(WORDS.length)],
    bpm: rand(6) === 0 ? '—' : (90 + rand(80) + rand(10) / 10).toFixed(1),
    key: rand(7) === 0 ? '—' : `${1 + rand(12)}${rand(2) ? 'A' : 'B'}`,
    time: rand(8) === 0 ? '—' : `${rand(12)}:${String(rand(60)).padStart(2, '0')}`,
    index: String(i + 1).padStart(2, '0'),
    year: rand(4) === 0 ? '—' : String(1980 + rand(45)),
    rating: rand(6),
    color: rand(3) === 0 ? null : ['Blue', 'Pink', 'Red', '#112233'][rand(4)],
    fav: rand(4) === 0 ? 1 + rand(9) : undefined,
  }));

  for (const key of ['title', 'artist', 'bpm', 'key', 'time', 'index', 'year', 'rating', 'color', 'fav']) {
    it(`orders ${key} exactly like the reference, both directions and again from the cache`, () => {
      clearSort('library');
      setSort('library', key);
      const asc = referenceSort(big, key, 1);
      expect(sortTracks(big, 'library').map((r) => r.id)).toEqual(asc);
      expect(sortTracks(big, 'library').map((r) => r.id)).toEqual(asc);
      setSort('library', key);
      expect(sortTracks(big, 'library').map((r) => r.id)).toEqual(referenceSort(big, key, -1));
    });
  }
});
