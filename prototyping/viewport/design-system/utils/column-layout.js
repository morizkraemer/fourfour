/**
 * Shared flex styles for column-header + track-row cells.
 * Keeps header/body aligned and enforces per-column minimum widths.
 */

/** Minimum width (px) when resizing or laying out a column. */
export const COLUMN_MIN_WIDTH = {
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

const DEFAULT_MIN = 24;

export function minWidthForColumn(key, fallback = DEFAULT_MIN) {
  return COLUMN_MIN_WIDTH[key] ?? fallback;
}

/**
 * @param {{ key?: string, flex?: boolean, width?: number, minWidth?: number, align?: string }} col
 * @returns {string}
 */
export function columnCellStyle(col) {
  const align = col.align === 'right' ? 'justify-content:flex-end;' : '';
  if (col.flex) {
    const min = col.minWidth ?? minWidthForColumn(col.key, 120);
    return `flex:1 1 0;min-width:${min}px;${align}`;
  }
  const w = col.width ?? minWidthForColumn(col.key, 80);
  const min = minWidthForColumn(col.key, DEFAULT_MIN);
  const width = Math.max(w, min);
  return `flex:0 0 ${width}px;width:${width}px;${align}`;
}
