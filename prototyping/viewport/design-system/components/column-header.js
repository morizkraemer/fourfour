import './column-header.css';
import { el } from '../utils/dom.js';
import { createIcon } from './icon.js';

/*
 * Column header row for track lists (§7 / §22). Driven by the same `columns`
 * array as the track row, so widths stay aligned.
 *   columns: [{ label, width?, flex?, align?, sort? }]
 *     flex: true   → column grows to fill (the Title column)
 *     width: px    → fixed column
 *     align:'right'→ right-align the label
 *     sort:'asc'|'desc' → active sort column (emphasized + caret)
 */
export function createColumnHeader({ columns = [] } = {}) {
  const cells = columns.map((col) => {
    const active = !!col.sort;
    const children = [el('span', { class: 'ff-colh__label', text: col.label ?? '' })];
    if (active) {
      const { element } = createIcon({ name: 'chevron-down', size: 10, strokeWidth: 2.25 });
      element.classList.add('ff-colh__arrow');
      if (col.sort === 'asc') element.classList.add('ff-colh__arrow--asc');
      children.push(element);
    }
    const style = col.flex ? { flex: '1 1 0' } : { width: `${col.width}px`, flex: 'none' };
    if (col.align === 'right') style.justifyContent = 'flex-end';
    return el(
      'div',
      { class: `ff-colh__cell${active ? ' ff-colh__cell--active' : ''}`, style },
      children
    );
  });
  return { element: el('div', { class: 'ff-colh' }, cells) };
}
