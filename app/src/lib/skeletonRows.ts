import { columnCellStyle } from '../../../prototyping/viewport/design-system/utils/column-layout.js';
import type { TableColumn } from '../stores/table-columns.svelte.ts';

const EMPTY_CELL_KINDS = new Set(['tag', 'fav', 'wave', 'rating', 'color']);

function attribute(value: string): string {
  return value.replace(/[&"<>]/g, (c) => `&#${c.charCodeAt(0)};`);
}

function cellHtml(column: TableColumn, title?: string): string {
  const key = attribute(column.key);
  const style = attribute(columnCellStyle(column));
  if (column.key === 'cover') {
    return `<div class="ff-trow__cell ff-trow__cell--cover" style="${style}" data-col-key="${key}"><div class="ff-trow__cover"></div></div>`;
  }
  const classes = EMPTY_CELL_KINDS.has(column.key) ? `ff-trow__cell ff-trow__cell--${key}` : `ff-trow__cell ff-trow__cell--${key} ff-trow__cell--text`;
  const content = title !== undefined && column.key === 'title' ? `<span class="ff-trow__text">${attribute(title)}</span>` : '';
  return `<div class="${classes}" style="${style}" data-col-key="${key}">${content}</div>`;
}

export function skeletonTextWidth(row: number): number {
  return 55 + ((row * 17) % 40);
}

/** The placeholder rows of a table that has no tracks yet: the cells of a track row with no content, as one string. */
export function skeletonRowsHtml(columns: TableColumn[], rows: number): string {
  const cells = columns.map((column) => cellHtml(column)).join('');
  let html = '';
  for (let row = 0; row < rows; row++) {
    html += `<div class="ff-table__skeleton-row" style="--skeleton-text: ${skeletonTextWidth(row)}%;"><div class="ff-trow ff-trow--rest">${cells}</div></div>`;
  }
  return html;
}

/** Rows for files still being imported: the file name in the title cell, every other cell empty. */
export function pendingRowsHtml(columns: TableColumn[], names: readonly string[]): string {
  let html = '';
  for (const name of names) {
    html += `<div class="ff-table__pending-row" data-pending-row><div class="ff-trow ff-trow--rest">${columns.map((c) => cellHtml(c, name)).join('')}</div></div>`;
  }
  return html;
}
