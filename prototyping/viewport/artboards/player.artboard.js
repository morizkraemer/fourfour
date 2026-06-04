import { el } from '../design-system/utils/dom.js';
import {
  createPlayerCompact,
  createPlayerExpanded
} from '../design-system/index.js';

export const meta = { title: 'Player UI (compact · expanded waveform)', layer: 'composite' };

const cap = (t) =>
  el('div', {
    style: {
      fontFamily: 'var(--ff-font-mono)',
      fontSize: 'var(--ff-type-label)',
      letterSpacing: '0.06em',
      fontWeight: '500',
      color: 'var(--ff-faint)',
      marginTop: 'var(--ff-space-4)',
      marginBottom: 'var(--ff-space-2)',
    },
    text: t,
  });

export default function render() {
  // Player Compact
  const compactIdle = createPlayerCompact({
    title: 'One More Time',
    artist: 'Daft Punk',
    currentTime: '01:24',
    currentTimeMs: '.4',
    totalTime: '05:20',
    progress: 0.26,
    playing: false,
    bpm: '128.00',
    key: '8A',
    onPlayToggle: () => alert('Play toggle compact')
  }).element;

  const compactPlaying = createPlayerCompact({
    title: 'Strobe',
    artist: 'deadmau5',
    currentTime: '04:12',
    currentTimeMs: '.8',
    totalTime: '10:37',
    progress: 0.39,
    playing: true,
    bpm: '128.00',
    key: '3A',
    onPlayToggle: () => alert('Play toggle compact')
  }).element;

  // Player Expanded
  const expanded = createPlayerExpanded({
    title: 'One More Time',
    artist: 'Daft Punk',
    bpm: '128.00',
    key: '8A',
    time: '01:24.320 / 05:20',
    cues: [
      { name: 'A', position: 10 },
      { name: 'B', position: 28 },
      { name: 'C', position: 52 },
      { name: 'D', position: 75 }
    ],
    onSave: () => alert('Save expanded'),
    onDiscard: () => alert('Discard expanded'),
    onClose: () => alert('Close expanded')
  }).element;

  return el('div', {
    style: {
      display: 'flex',
      flexDirection: 'column',
      gap: 'var(--ff-space-4)',
      width: '740px',
      padding: 'var(--ff-space-7)',
      background: 'var(--ff-surface)',
      border: '1px solid var(--ff-border)',
      borderRadius: 'var(--ff-radius-md)',
    }
  }, [
    cap('COMPACT BOTTOM PLAYER (IDLE & PLAYING)'),
    el('div', { style: { display: 'flex', flexDirection: 'column', gap: '12px' } }, [
      compactIdle,
      compactPlaying
    ]),
    cap('EXPANDED WAVEFORM & CUE EDITOR'),
    expanded
  ]);
}
