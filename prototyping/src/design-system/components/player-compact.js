import './player-compact.css';
import { el } from '../utils/dom.js';

const NS = 'http://www.w3.org/2000/svg';

function createPlayIcon() {
  const svg = document.createElementNS(NS, 'svg');
  svg.setAttribute('viewBox', '0 0 24 24');
  svg.setAttribute('width', '11');
  svg.setAttribute('height', '11');
  svg.setAttribute('fill', 'currentColor');
  svg.innerHTML = '<polygon points="6,4 6,20 18,12" />';
  return svg;
}

function createPauseIcon() {
  const svg = document.createElementNS(NS, 'svg');
  svg.setAttribute('viewBox', '0 0 24 24');
  svg.setAttribute('width', '11');
  svg.setAttribute('height', '11');
  svg.setAttribute('fill', 'currentColor');
  svg.innerHTML = '<rect x="6" y="4" width="4" height="16" /><rect x="14" y="4" width="4" height="16" />';
  return svg;
}

function createChevronUpIcon() {
  const svg = document.createElementNS(NS, 'svg');
  svg.setAttribute('viewBox', '0 0 24 24');
  svg.setAttribute('width', '12');
  svg.setAttribute('height', '12');
  svg.setAttribute('fill', 'none');
  svg.setAttribute('stroke', 'currentColor');
  svg.setAttribute('stroke-width', '2');
  svg.setAttribute('stroke-linecap', 'round');
  svg.setAttribute('stroke-linejoin', 'round');
  svg.innerHTML = '<polyline points="18 15 12 9 6 15" />';
  return svg;
}

/**
 * @param {object} options
 * @param {string} options.title
 * @param {string} options.artist
 * @param {string} options.currentTime        – current time string like "2:54"
 * @param {string} [options.currentTimeMs=".2"]
 * @param {string} options.totalTime          – total duration string like "8:14"
 * @param {number} options.progress           – 0 to 1 progress value
 * @param {boolean} [options.playing=false]
 * @param {string|number} [options.bpm="124.00"]
 * @param {string} [options.key="8A"]
 * @param {function} [options.onPlayToggle]
 * @param {function} [options.onCue]
 * @param {function} [options.onBpmChange]
 * @param {function} [options.onExpand]
 */
export function createPlayerCompact({
  title,
  artist,
  currentTime,
  currentTimeMs = '.2',
  totalTime,
  progress = 0,
  playing = false,
  bpm = '124.00',
  key = '8A',
  onPlayToggle,
  onCue,
  onBpmChange,
  onExpand
} = {}) {
  // 1. Left Controls: cueBtn & playWrap (30x30 circles)
  const cueBtn = el('button', {
    class: 'ff-player-compact__cue-btn',
    onClick: (e) => {
      e.stopPropagation();
      if (typeof onCue === 'function') onCue();
    }
  }, [
    el('span', { class: 'ff-player-compact__cue-txt' }, ['CUE'])
  ]);

  const playIcon = playing ? createPauseIcon() : createPlayIcon();
  playIcon.classList.add('ff-player-compact__play-icon');

  const playBtn = el('button', {
    class: 'ff-player-compact__play-btn',
    onClick: (e) => {
      e.stopPropagation();
      if (typeof onPlayToggle === 'function') onPlayToggle();
    }
  }, [playIcon]);

  const leftControls = el('div', { class: 'ff-player-compact__left-controls' }, [
    cueBtn,
    playBtn
  ]);

  // 2. Content: Meta row + Waveform row
  // Meta title/artist inline
  const titleGroup = el('div', { class: 'ff-player-compact__title-group' }, [
    el('span', { class: 'ff-player-compact__meta-title' }, [title]),
    el('span', { class: 'ff-player-compact__meta-artist' }, [artist])
  ]);

  // BPM nudge (17px height)
  const bpmN = el('div', { class: 'ff-player-compact__bpm-nudge' }, [
    el('button', {
      class: 'ff-player-compact__nudge-btn',
      onClick: (e) => {
        e.stopPropagation();
        if (typeof onBpmChange === 'function') onBpmChange('decrement');
      }
    }, ['−']),
    el('span', { class: 'ff-player-compact__nudge-value' }, [String(bpm)]),
    el('button', {
      class: 'ff-player-compact__nudge-btn',
      onClick: (e) => {
        e.stopPropagation();
        if (typeof onBpmChange === 'function') onBpmChange('increment');
      }
    }, ['+'])
  ]);

  // Key display (17px height)
  const keyN = el('div', { class: 'ff-player-compact__key-display' }, [
    el('span', { class: 'ff-player-compact__key-value' }, [key])
  ]);

  // Times row
  const times = el('div', { class: 'ff-player-compact__times-group' }, [
    el('span', { class: 'ff-player-compact__time-cur' }, [
      currentTime,
      el('span', { class: 'ff-player-compact__time-cur-ms' }, [currentTimeMs])
    ]),
    el('span', { class: 'ff-player-compact__time-tot' }, [totalTime])
  ]);

  // Chevron
  const chevronIcon = createChevronUpIcon();
  chevronIcon.classList.add('ff-player-compact__expand-chevron');
  const expandBtn = el('button', {
    class: 'ff-player-compact__expand-btn',
    onClick: (e) => {
      e.stopPropagation();
      if (typeof onExpand === 'function') onExpand();
    }
  }, [chevronIcon]);

  const bpmKeyGroup = el('div', { class: 'ff-player-compact__bpm-key-group' }, [
    keyN,
    bpmN
  ]);

  const rightToggles = el('div', { class: 'ff-player-compact__right-toggles' }, [
    bpmKeyGroup,
    times,
    expandBtn
  ]);

  const metaRow = el('div', { class: 'ff-player-compact__meta-row' }, [
    titleGroup,
    rightToggles
  ]);

  // Waveform (height: 34px)
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

  const contentCol = el('div', { class: 'ff-player-compact__content' }, [
    metaRow,
    waveform
  ]);

  const element = el('div', { class: 'ff-player-compact' }, [
    leftControls,
    contentCol
  ]);

  return { element };
}
