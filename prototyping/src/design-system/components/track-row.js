import './track-row.css';
import { el } from '../utils/dom.js';
import { createTagBadge } from './tag-badge.js';
import { createColorSwatch } from './color-swatch.js';
import { createWaveform } from './waveform.js';
import { keyColor } from '../utils/key-color.js';

/*
 * Track list row (§7). Cells are driven by a `columns` array so a column header
 * built from the same array stays aligned.
 *   track: { index, title, artist, label, bpm, key, time, tag, fav, cover, peaks }
 *     tag: null | { type:'color', color } | { type:'digit', value, color }
 *   state: 'rest' | 'hover' | 'selected' | 'analyzing' | 'drag'
 *
 * Special cell types (dispatched by col.key): 'fav' (toggle box), 'wave'
 * (mini-waveform preview), 'cover' (artwork thumb), 'key' (camelot-colored).
 * Everything else renders as text. TRACK_COLUMNS is the default text layout;
 * the table module passes its own Pencil-matching column set.
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

  if (col.key === 'fav') {
    // track.fav is the favorite slot number (1..8). Filled box + centered digit
    // when assigned (Pencil "rNfav"); empty transparent box otherwise.
    const digit = track.fav;
    const on = digit != null && digit !== false && digit !== '';
    const box = el('div', { class: `ff-trow__fav${on ? ' ff-trow__fav--on' : ''}` },
      on && digit !== true ? [el('span', { class: 'ff-trow__fav-digit', text: String(digit) })] : []);
    return el('div', { class: 'ff-trow__cell ff-trow__cell--fav', style }, [box]);
  }

  if (col.key === 'wave') {
    const w = col.width ?? 120;
    const { element } = createWaveform({
      peaks: track.peaks ?? null,
      width: w,
      height: 14,
      variant: 'mini',
      seed: (track.index ?? 1) * 2654435761,
    });
    return el('div', { class: 'ff-trow__cell ff-trow__cell--wave', style }, [element]);
  }

  if (col.key === 'cover') {
    const thumb = el('div', { class: 'ff-trow__cover' });
    if (track.cover) thumb.style.backgroundImage = `url(${track.cover})`;
    return el('div', { class: 'ff-trow__cell ff-trow__cell--cover', style }, [thumb]);
  }

  if (col.key === 'key') {
    const k = String(track.key ?? '—');
    const span = el('span', { class: 'ff-trow__text', text: k });
    span.style.color = k === '—' ? 'var(--ff-muted)' : keyColor(k);
    return el('div', { class: 'ff-trow__cell ff-trow__cell--key', style }, [span]);
  }

  return el('div', { class: `ff-trow__cell ff-trow__cell--${col.key}`, style }, [
    el('span', { class: 'ff-trow__text', text: String(track[col.key] ?? '') }),
  ]);
}
