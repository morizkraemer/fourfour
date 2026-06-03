import './player-compact.css';
import { el } from '../utils/dom.js';

const NS = 'http://www.w3.org/2000/svg';

function createArtIcon() {
  const svg = document.createElementNS(NS, 'svg');
  svg.setAttribute('viewBox', '0 0 24 24');
  svg.setAttribute('width', '22');
  svg.setAttribute('height', '22');
  svg.setAttribute('fill', 'none');
  svg.setAttribute('stroke', 'currentColor');
  svg.setAttribute('stroke-width', '2');
  svg.setAttribute('stroke-linecap', 'round');
  svg.setAttribute('stroke-linejoin', 'round');
  svg.innerHTML = '<circle cx="12" cy="12" r="10" /><circle cx="12" cy="12" r="2" />';
  return svg;
}

function createPlayIcon() {
  const svg = document.createElementNS(NS, 'svg');
  svg.setAttribute('viewBox', '0 0 24 24');
  svg.setAttribute('width', '10');
  svg.setAttribute('height', '10');
  svg.setAttribute('fill', 'currentColor');
  svg.innerHTML = '<polygon points="6,4 6,20 18,12" />';
  return svg;
}

/**
 * @param {object} options
 * @param {string} options.title
 * @param {string} options.artist
 * @param {string} options.currentTime
 * @param {string} options.totalTime
 * @param {number} options.progress             – 0 to 1 progress value
 * @param {boolean} [options.playing=false]
 * @param {function} [options.onPlayToggle]
 */
export function createPlayerCompact({
  title,
  artist,
  currentTime,
  totalTime,
  progress = 0,
  playing = false,
  onPlayToggle
} = {}) {
  const artIcon = createArtIcon();
  artIcon.classList.add('ff-player-compact__art-icon');
  const art = el('div', { class: 'ff-player-compact__art' }, [artIcon]);

  const playIcon = createPlayIcon();
  const playBtn = el('button', {
    class: `ff-player-compact__play-btn ${playing ? 'ff-player-compact__play-btn--playing' : ''}`,
    onClick: (e) => {
      e.stopPropagation();
      if (typeof onPlayToggle === 'function') onPlayToggle();
    }
  }, [playIcon]);

  // Waveform
  const playedOverlay = el('div', {
    class: 'ff-player-compact__waveform-played',
    style: `width: ${progress * 100}%`
  });

  const playhead = el('div', {
    class: 'ff-player-compact__waveform-playhead',
    style: `left: ${progress * 100}%`
  });

  const waveform = el('div', { class: 'ff-player-compact__waveform' }, [
    playedOverlay,
    playhead
  ]);

  const timesRow = el('div', { class: 'ff-player-compact__times' }, [
    el('span', {}, [currentTime]),
    el('span', {}, [totalTime])
  ]);

  const metaRow = el('div', { class: 'ff-player-compact__meta' }, [
    el('span', { class: 'ff-player-compact__title' }, [title]),
    el('span', { class: 'ff-player-compact__artist' }, [artist])
  ]);

  const contentCol = el('div', { class: 'ff-player-compact__content' }, [
    metaRow,
    waveform,
    timesRow
  ]);

  const element = el('div', { class: 'ff-player-compact' }, [
    art,
    playBtn,
    contentCol
  ]);

  return { element };
}
