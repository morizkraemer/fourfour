/**
 * Shared flex styles for column-header + track-row cells.
 * Keeps header/body aligned and enforces per-column minimum widths.
 */

/** Minimum width (px) when resizing — icon-tight, text truncates with ellipsis below this. */
export const COLUMN_MIN_WIDTH = {
  fav: 20,
  index: 24,
  wave: 36,
  cover: 18,
  title: 32,
  artist: 32,
  label: 32,
  album: 32,
  genre: 32,
  year: 28,
  rating: 52,
  color: 24,
  bpm: 36,
  key: 28,
  time: 40,
};

const DEFAULT_MIN = 20;

const NON_TEXT_COLUMNS = new Set(['fav', 'index', 'wave', 'cover', 'rating', 'color']);

export function minWidthForColumn(key, fallback = DEFAULT_MIN) {
  return COLUMN_MIN_WIDTH[key] ?? fallback;
}

/**
 * @param {{ key?: string, flex?: boolean, width?: number, minWidth?: number }} col
 * @param {{ width?: number, minWidth?: number }} [override]
 * @returns {number} the fixed width, or the minimum width of a flex column
 */
export function columnWidthPx(col, override = {}) {
  if (col.flex) {
    const floor = minWidthForColumn(col.key);
    return Math.max(floor, override.minWidth ?? override.width ?? col.minWidth ?? floor);
  }
  const w = override.width ?? col.width ?? minWidthForColumn(col.key);
  return Math.max(w, minWidthForColumn(col.key, DEFAULT_MIN));
}

/**
 * @param {{ key?: string, flex?: boolean, width?: number, minWidth?: number, align?: string }} col
 * @param {{ width?: number, minWidth?: number }} [override]
 * @returns {string}
 */
export function columnCellStyle(col, override = {}) {
  const align = col.align === 'right' ? 'justify-content:flex-end;' : '';
  const width = columnWidthPx(col, override);
  if (col.flex) return `flex:1 1 0;min-width:${width}px;max-width:none;${align}`;
  return `flex:0 0 ${width}px;width:${width}px;max-width:${width}px;min-width:${width}px;${align}`;
}

/**
 * Track-row cell geometry for a whole table: one `--ff-col-<key>` per column, set on an ancestor of the rows.
 * `track-row.css` sizes each `.ff-trow__cell--<key>` from it, so a resize touches one element, not every cell.
 * @param {Array<{ key?: string, flex?: boolean, width?: number, minWidth?: number }>} columns
 * @returns {string}
 */
export function columnVars(columns) {
  let vars = '';
  for (const col of columns) if (col.key) vars += `--ff-col-${col.key}:${columnWidthPx(col)}px;`;
  return vars;
}

/**
 * @param {{ flex?: boolean, align?: string }} col
 * @returns {string} classes a track-row cell needs besides its key class
 */
export function columnCellClass(col) {
  return (col.flex ? ' ff-trow__cell--flex' : '') + (col.align === 'right' ? ' ff-trow__cell--end' : '');
}

/**
 * Measure the widest rendered text (or fixed chrome) for a column in a table root.
 * @param {string} key
 * @param {HTMLElement | null} rootEl
 * @param {Array<{ key?: string }>} columns
 */
export function measureColumnContentWidth(key, rootEl, columns) {
  const min = minWidthForColumn(key);
  if (!rootEl || !key) return min;

  const colIndex = columns.findIndex(c => c.key === key);
  if (colIndex < 0) return min;

  if (NON_TEXT_COLUMNS.has(key)) {
    const header = rootEl.querySelector(`.ff-colh__cell[data-col-key="${key}"]`);
    return Math.max(min, header ? Math.ceil(header.getBoundingClientRect().width) : min);
  }

  const canvas = document.createElement('canvas');
  const ctx = canvas.getContext('2d');
  if (!ctx) return min;

  let max = 0;

  function measureTextEl(el) {
    const text = el.textContent?.trim() ?? '';
    if (!text) return 0;
    const style = getComputedStyle(el);
    ctx.font = `${style.fontWeight} ${style.fontSize} ${style.fontFamily}`;
    return ctx.measureText(text).width;
  }

  const header = rootEl.querySelector(`.ff-colh__cell[data-col-key="${key}"]`);
  if (header) {
    const label = header.querySelector('.ff-colh__label');
    if (label) max = Math.max(max, measureTextEl(label));
    if (header.querySelector('.ff-colh__arrow')) max += 14;
  }

  for (const row of rootEl.querySelectorAll('.ff-trow')) {
    const cell = row.querySelectorAll('.ff-trow__cell')[colIndex];
    if (!cell) continue;
    const textEl = cell.querySelector('.ff-trow__text');
    if (textEl) max = Math.max(max, measureTextEl(textEl));
  }

  return Math.max(min, Math.ceil(max) + 10);
}
