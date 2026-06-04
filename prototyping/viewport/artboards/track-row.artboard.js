import { el } from '../design-system/utils/dom.js';
import { host, ColumnHeader, TrackRow, TRACK_COLUMNS } from '../design-system/svelte.js';

export const meta = { title: 'Track Row + Column Header', layer: 'primitive' };

// Column header shares TRACK_COLUMNS; mark BPM as the active sort column.
const headerColumns = TRACK_COLUMNS.map((c) => (c.key === 'bpm' ? { ...c, sort: 'desc' } : c));

const ROWS = [
  { state: 'rest', track: { index: '01', title: 'Hyperion', artist: 'Solomun', label: 'Diynamic', bpm: '124', time: '7:42', tag: { type: 'color', color: 1 } } },
  { state: 'hover', track: { index: '02', title: 'Voidwalker', artist: 'Tale Of Us', label: 'Afterlife', bpm: '122', time: '8:14', tag: { type: 'color', color: 5 } } },
  { state: 'selected', track: { index: '03', title: 'Drift', artist: 'Bicep', label: 'Ninja Tune', bpm: '128', time: '6:55', tag: { type: 'digit', value: 1, color: 4 } } },
  { state: 'analyzing', track: { index: '04', title: 'Aphelion', artist: 'Maceo Plex', label: 'Lone Romantic', bpm: '—', time: '—', tag: null } },
  { state: 'drag', track: { index: '05', title: 'Resonate', artist: 'Mind Against', label: 'Life and Death', bpm: '121', time: '7:08', tag: { type: 'color', color: 6 } } },
];

export default function render() {
  const children = [host(ColumnHeader, { columns: headerColumns })];
  for (const r of ROWS) children.push(host(TrackRow, r));

  return el(
    'div',
    {
      style: {
        width: '820px',
        display: 'flex',
        flexDirection: 'column',
        background: 'var(--ff-surface)',
        border: '1px solid var(--ff-border)',
        borderRadius: 'var(--ff-radius-md)',
        overflow: 'hidden',
      },
    },
    children
  );
}
