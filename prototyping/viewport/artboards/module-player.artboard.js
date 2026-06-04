import './module-player.css';
import { el } from '../design-system/utils/dom.js';
import { createPlayerCompact } from '../design-system/index.js';

export const meta = { title: 'Module · Global Player (compact, full width)', layer: 'module' };

/*
 * The bottom player bar spans the full window width (Pencil "Global Player").
 * Wraps the compact player primitive at module width. Exports buildPlayer().
 */
export function buildPlayer() {
  const { element } = createPlayerCompact({
    title: 'Voidwalker',
    artist: 'Tale Of Us',
    currentTime: '2:54',
    currentTimeMs: '.2',
    totalTime: '8:14',
    progress: 0.35,
    playing: true,
    bpm: '122.00',
    key: '7A',
    // hot-cue markers (0..1) — same colors as the detail-pane cue dots
    cues: [
      { position: 0.0,  color: '#ec4899' }, // Intro 0:00
      { position: 0.19, color: '#38bdf8' }, // Drop  1:32
      { position: 0.58, color: '#4ade80' }, // Break 4:48
    ],
  });
  return el('div', { class: 'ff-player-module' }, [element]);
}

export default function render() {
  return el('div', { class: 'ff-player-demo' }, [buildPlayer()]);
}
