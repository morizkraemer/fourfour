import './module-table.css';
import { el } from '../design-system/utils/dom.js';
import { createColumnHeader, createTrackRow } from '../design-system/index.js';

export const meta = { title: 'Module · Table (column-header + track rows)', layer: 'module' };

/*
 * The library table module: a column header + a scroll body of track rows,
 * driven by one Pencil-matching column set so header and rows stay aligned.
 * Exports buildTable() so the Browse composite can drop it in as-is.
 */
export const TABLE_COLUMNS = [
  { key: 'fav', label: 'FAV', width: 20 },
  { key: 'index', label: '#', width: 24 },
  { key: 'wave', label: 'PREVIEW', width: 120 },
  { key: 'cover', label: '', width: 18 },
  { key: 'title', label: 'TITLE', flex: true },
  { key: 'artist', label: 'ARTIST', width: 180 },
  { key: 'label', label: 'LABEL', width: 130 },
  { key: 'bpm', label: 'BPM', width: 50, sort: 'desc' },
  { key: 'key', label: 'KEY', width: 36 },
  { key: 'time', label: 'TIME', width: 48 },
];

const TRACKS = [
  { title: 'Hyperion', artist: 'Solomun', label: 'Diynamic', bpm: '124', key: '9A', time: '7:42' },
  { title: 'Voidwalker', artist: 'Tale Of Us', label: 'Afterlife', bpm: '122', key: '7A', time: '8:14', fav: 1, state: 'selected' },
  { title: 'Drift', artist: 'Bicep', label: 'Ninja Tune', bpm: '128', key: '5B', time: '6:55' },
  { title: 'Aphelion', artist: 'Maceo Plex', label: 'Lone Romantic', bpm: '121', key: '6A', time: '7:08' },
  { title: 'Resonate', artist: 'Mind Against', label: 'Life and Death', bpm: '120', key: '8A', time: '7:34' },
  { title: 'Glass Ladder', artist: 'Adam Port', label: 'Keinemusik', bpm: '118', key: '4A', time: '6:32', fav: 3 },
  { title: 'Suspended', artist: 'Stephan Bodzin', label: 'Herzblut', bpm: '124', key: '11A', time: '8:02' },
  { title: 'Fractured Light', artist: 'Recondite', label: 'Plangent', bpm: '121', key: '12B', time: '7:18' },
  { title: 'Distant Star', artist: 'Kiasmos', label: 'Erased Tapes', bpm: '119', key: '9B', time: '6:48' },
  { title: 'Halflight', artist: 'Kollektiv Turmstrasse', label: 'Diynamic', bpm: '122', key: '6A', time: '7:55' },
  { title: 'Erosion', artist: 'Innellea', label: 'Afterlife', bpm: '—', key: '—', time: '—', state: 'analyzing' },
  { title: 'Cinder', artist: 'Anfisa Letyago', label: 'NTFO', bpm: '130', key: '9A', time: '6:14', fav: 2 },
];

export function buildTable() {
  const header = createColumnHeader({ columns: TABLE_COLUMNS }).element;

  const rows = TRACKS.map((t, i) => {
    const { state = 'rest', ...track } = t;
    return createTrackRow({ track: { index: String(i + 1).padStart(2, '0'), ...track }, state, columns: TABLE_COLUMNS }).element;
  });

  const body = el('div', { class: 'ff-table__body' }, rows);
  return el('div', { class: 'ff-table' }, [header, body]);
}

export default function render() {
  return el('div', { class: 'ff-table-demo' }, [buildTable()]);
}
