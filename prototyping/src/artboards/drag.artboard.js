import { el } from '../design-system/utils/dom.js';
import {
  createDragGhost,
  createDropLine,
  createDropOverlay
} from '../design-system/index.js';

export const meta = { title: 'Drag & Drop Primitives (ghost · dropline · overlay)', layer: 'primitive' };

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

const row = (children, gap = 'var(--ff-space-5)') =>
  el('div', { style: { display: 'flex', alignItems: 'center', gap, flexWrap: 'wrap' } }, children);

export default function render() {
  // Drag Ghosts
  const singleGhost = createDragGhost({ label: 'Daft Punk - One More Time.mp3' }).element;
  const multiGhost = createDragGhost({ count: 5 }).element;

  const ghosts = row([
    singleGhost,
    multiGhost
  ]);

  // Drop Line
  const dropLine = createDropLine().element;

  // Drop Overlay
  const dropOverlay = createDropOverlay({
    title: 'Drop Tracks Here',
    sub: 'Add to Sunday Closing Playlist'
  }).element;

  return el('div', {
    style: {
      display: 'flex',
      flexDirection: 'column',
      gap: 'var(--ff-space-4)',
      width: '420px',
      padding: 'var(--ff-space-7)',
      background: 'var(--ff-surface)',
      border: '1px solid var(--ff-border)',
      borderRadius: 'var(--ff-radius-md)',
    }
  }, [
    cap('DRAG GHOSTS (FLOATING TRACKS)'),
    ghosts,
    cap('DROP LINE (ROW INSERTION INDICATOR)'),
    el('div', { style: { padding: '10px 0' } }, [dropLine]),
    cap('DROP OVERLAY PANEL'),
    dropOverlay
  ]);
}
