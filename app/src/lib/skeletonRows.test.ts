import { describe, expect, it } from 'vitest';
import { columnCellStyle } from '../../../prototyping/viewport/design-system/utils/column-layout.js';
import { skeletonRowsHtml, skeletonTextWidth } from './skeletonRows.ts';

const COLUMNS = [
  { key: 'status', label: '', width: 20 },
  { key: 'fav', label: 'FAV', width: 20 },
  { key: 'index', label: '#', width: 28 },
  { key: 'wave', label: 'PREVIEW', width: 120 },
  { key: 'cover', label: '', width: 18 },
  { key: 'title', label: 'TITLE', flex: true },
  { key: 'rating', label: 'RATING', width: 60 },
  { key: 'color', label: '', width: 24 },
  { key: 'bpm', label: 'BPM', width: 52, align: 'right' as const },
  { key: 'tag', label: '', width: 16, align: 'right' as const },
];

describe('skeletonRowsHtml', () => {
  it('has one row per requested row with the text width cycling as before', () => {
    const rows = skeletonRowsHtml(COLUMNS, 24).match(/class="ff-table__skeleton-row" style="--skeleton-text: (\d+)%;"/g) ?? [];
    expect(rows).toHaveLength(24);
    expect(rows[0]).toContain('--skeleton-text: 55%');
    expect(rows[1]).toContain(`--skeleton-text: ${skeletonTextWidth(1)}%`);
    expect([0, 1, 2, 3, 23].map(skeletonTextWidth)).toEqual([55, 72, 89, 66, 86]);
  });

  it('gives every column its cell with the same classes and layout style as a track row', () => {
    const row = skeletonRowsHtml(COLUMNS, 1);
    const cells = [...row.matchAll(/<div class="(ff-trow__cell [^"]*)" style="([^"]*)" data-col-key="([^"]*)">/g)];
    expect(cells.map((c) => c[3])).toEqual(COLUMNS.map((c) => c.key));
    expect(cells.map((c) => c[2])).toEqual(COLUMNS.map((c) => columnCellStyle(c)));
    const classes = Object.fromEntries(cells.map((c) => [c[3], c[1]]));
    expect(classes.status).toBe('ff-trow__cell ff-trow__cell--status ff-trow__cell--text');
    expect(classes.index).toBe('ff-trow__cell ff-trow__cell--index ff-trow__cell--text');
    expect(classes.title).toBe('ff-trow__cell ff-trow__cell--title ff-trow__cell--text');
    expect(classes.bpm).toBe('ff-trow__cell ff-trow__cell--bpm ff-trow__cell--text');
    for (const key of ['fav', 'wave', 'rating', 'color', 'tag']) expect(classes[key]).toBe(`ff-trow__cell ff-trow__cell--${key}`);
    expect(classes.cover).toBe('ff-trow__cell ff-trow__cell--cover');
  });

  it('draws only the cover thumbnail inside a cell', () => {
    const row = skeletonRowsHtml(COLUMNS, 1);
    expect(row.match(/<div class="ff-trow__cover"><\/div>/g)).toHaveLength(1);
    expect(row).not.toContain('<span');
    expect(row).not.toContain('<canvas');
  });

  it('keeps a column key from breaking out of its attribute', () => {
    const html = skeletonRowsHtml([{ key: 'x" onclick="alert(1)', label: '', width: 40 }], 1);
    expect(html).not.toContain('" onclick="');
    expect(html).toContain('data-col-key="x&#34; onclick=&#34;alert(1)"');
  });

  it('has no rows for an empty count', () => {
    expect(skeletonRowsHtml(COLUMNS, 0)).toBe('');
  });
});
