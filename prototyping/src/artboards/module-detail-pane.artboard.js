import './module-detail-pane.css';
import { el } from '../design-system/utils/dom.js';
import { createDetailPane } from '../design-system/index.js';

export const meta = { title: 'Module · Detail Pane (278w)', layer: 'module' };

/*
 * Right-side detail pane (Pencil "Detail Sidebar"): the resting state of the
 * morphing right panel — selected track artwork, metadata, cues, file path.
 * Exports buildDetailPane() for the Browse composite.
 */
export function buildDetailPane() {
  const pane = createDetailPane({
    mode: 'single',
    title: 'Voidwalker',
    artist: 'Tale Of Us',
    meta: {
      ALBUM: 'Endless Run',
      LABEL: 'Afterlife',
      BPM: '122',
      KEY: '7A',
      DURATION: '8:14',
      ADDED: '2 days ago',
    },
    cues: [
      { name: 'Intro', position: '0:00' },
      { name: 'Drop', position: '1:32' },
      { name: 'Break', position: '4:48' },
    ],
    filePath: '/Volumes/Music/Tale Of Us/Endless Run/02 Voidwalker.flac',
  }).element;

  return el('div', { class: 'ff-detail-module' }, [pane]);
}

export default function render() {
  return buildDetailPane();
}
