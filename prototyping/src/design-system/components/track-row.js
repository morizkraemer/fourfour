import './track-row.css';
import { el } from '../utils/dom.js';
import { createTagBadge } from './tag-badge.js';
import { createColorSwatch } from './color-swatch.js';

/*
 * Track list row (§7). Cells are driven by a `columns` array so a column header
 * built from the same array stays aligned.
 *   track: { index, title, artist, label, bpm, time, tag }
 *     tag: null | { type:'color', color } | { type:'digit', value, color }
 *   state: 'rest' | 'hover' | 'selected' | 'analyzing' | 'drag'
 *
 * TRACK_COLUMNS is the default layout — import it to feed createColumnHeader too.
 */
export const TRACK_COLUMNS = [
  { key: 'index', label: '#', width: 26 },
  { key: 'title', label: 'TITLE', flex: true },
  { key: 'artist', label: 'ARTIST', width: 200 },
  { key: 'label', label: 'LABEL', width: 160 },
  { key: 'bpm', label: 'BPM', width: 52 },
  { key: 'time', label: 'TIME', width: 48 },
  { key: 'tag', label: '', width: 16, align: 'right' },
];

export function createTrackRow({ track = {}, state = 'rest', columns = TRACK_COLUMNS } = {}) {
  const cells = columns.map((col) => renderCell(col, track));
  return { element: el('div', { class: `ff-trow ff-trow--${state}` }, cells) };
}

function renderCell(col, track) {
  const style = col.flex ? { flex: '1 1 0' } : { width: `${col.width}px`, flex: 'none' };
  if (col.align === 'right') style.justifyContent = 'flex-end';

  if (col.key === 'tag') {
    const tag = track.tag;
    let child = null;
    if (tag && tag.type === 'digit') {
      child = createTagBadge({ value: tag.value, variant: 'color', color: tag.color, size: 'sm' }).element;
    } else if (tag) {
      child = createColorSwatch({ color: tag.color }).element;
    }
    return el('div', { class: 'ff-trow__cell ff-trow__cell--tag', style }, child ? [child] : []);
  }

  return el('div', { class: `ff-trow__cell ff-trow__cell--${col.key}`, style }, [
    el('span', { class: 'ff-trow__text', text: String(track[col.key] ?? '') }),
  ]);
}
