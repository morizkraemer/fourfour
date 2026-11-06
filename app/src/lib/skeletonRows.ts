import { columnCellStyle } from '../../../prototyping/viewport/design-system/utils/column-layout.js';
import type { TableColumn } from '../stores/table-columns.svelte.ts';

const EMPTY_CELL_KINDS = new Set(['tag', 'fav', 'wave', 'rating', 'color']);

function attribute(value: string): string {
  return value.replace(/[&"<>]/g, (c) => `&#${c.charCodeAt(0)};`);
}

function cellHtml(column: TableColumn): string {
  const key = attribute(column.key);
  const style = attribute(columnCellStyle(column));
  if (column.key === 'cover') {
    return `<div class="ff-trow__cell ff-trow__cell--cover" style="${style}" data-col-key="${key}"><div class="ff-trow__cover"></div></div>`;
  }
  const classes = EMPTY_CELL_KINDS.has(column.key) ? `ff-trow__cell ff-trow__cell--${key}` : `ff-trow__cell ff-trow__cell--${key} ff-trow__cell--text`;
  return `<div class="${classes}" style="${style}" data-col-key="${key}"></div>`;
}

export function skeletonTextWidth(row: number): number {
  return 55 + ((row * 17) % 40);
}

/** The placeholder rows of a table that has no tracks yet: the cells of a track row with no content, as one string. */
export function skeletonRowsHtml(columns: TableColumn[], rows: number): string {
  const cells = columns.map(cellHtml).join('');
  let html = '';
  for (let row = 0; row < rows; row++) {
    html += `<div class="ff-table__skeleton-row" style="--skeleton-text: ${skeletonTextWidth(row)}%;"><div class="ff-trow ff-trow--rest">${cells}</div></div>`;
  }
  return html;
}
