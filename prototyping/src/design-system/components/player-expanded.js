import './player-expanded.css';
import { el } from '../utils/dom.js';

const NS = 'http://www.w3.org/2000/svg';

function createPlayIcon() {
  const svg = document.createElementNS(NS, 'svg');
  svg.setAttribute('viewBox', '0 0 24 24');
  svg.setAttribute('width', '10');
  svg.setAttribute('height', '10');
  svg.setAttribute('fill', 'currentColor');
  svg.innerHTML = '<polygon points="6,4 6,20 18,12" />';
  return svg;
}

function createChevronDownIcon() {
  const svg = document.createElementNS(NS, 'svg');
  svg.setAttribute('viewBox', '0 0 24 24');
  svg.setAttribute('width', '14');
  svg.setAttribute('height', '14');
  svg.setAttribute('fill', 'none');
  svg.setAttribute('stroke', 'currentColor');
  svg.setAttribute('stroke-width', '2');
  svg.setAttribute('stroke-linecap', 'round');
  svg.setAttribute('stroke-linejoin', 'round');
  svg.innerHTML = '<polyline points="6 9 12 15 18 9" />';
  return svg;
}

/**
 * @param {object} options
 * @param {string} options.title
 * @param {string} options.artist
 * @param {string} options.bpm
 * @param {string} options.key
 * @param {string} options.time
 * @param {Array<{ name: string, position: number }>} [options.cues]
 * @param {function} [options.onSave]
 * @param {function} [options.onDiscard]
 * @param {function} [options.onClose]
 */
export function createPlayerExpanded({
  title,
  artist,
  bpm,
  key,
  time,
  cues = [],
  onSave,
  onDiscard,
  onClose
} = {}) {
  // 1. Head Row
  const playIcon = createPlayIcon();
  const playBtn = el('button', { class: 'ff-player-expanded__play-btn' }, [playIcon]);

  const titleEl = el('span', { class: 'ff-player-expanded__title' }, [title]);
  const artistEl = el('span', { class: 'ff-player-expanded__artist' }, [artist]);

  const bpmPill = el('div', { class: 'ff-player-expanded__meta-pill' }, [bpm]);
  const keyPill = el('div', { class: 'ff-player-expanded__meta-pill' }, [key]);
  const timePill = el('div', { class: 'ff-player-expanded__meta-pill' }, [time]);
  const pills = el('div', { class: 'ff-player-expanded__meta-pills' }, [bpmPill, keyPill, timePill]);

  const leftHead = el('div', { class: 'ff-player-expanded__head-left' }, [
    playBtn,
    titleEl,
    artistEl,
    pills
  ]);

  const saveBtn = el('button', {
    class: 'ff-player-expanded__action-btn ff-player-expanded__action-btn--ghost',
    onClick: onSave
  }, ['Save']);

  const discardBtn = el('button', {
    class: 'ff-player-expanded__action-btn ff-player-expanded__action-btn--ghost',
    onClick: onDiscard
  }, ['Discard']);

  const chevron = createChevronDownIcon();
  chevron.classList.add('ff-player-expanded__chevron');
  const closeBtn = el('button', {
    class: 'ff-player-expanded__close-btn',
    onClick: onClose
  }, [chevron]);

  const rightHead = el('div', { class: 'ff-player-expanded__head-right' }, [
    saveBtn,
    discardBtn,
    closeBtn
  ]);

  const headRow = el('div', { class: 'ff-player-expanded__head' }, [leftHead, rightHead]);

  // 2. Cuestrip
  const defaultCues = cues.length > 0 ? cues : [
    { name: 'A', position: 10 },
    { name: 'B', position: 35 },
    { name: 'C', position: 60 },
    { name: 'D', position: 85 },
  ];

  const cueMarkers = defaultCues.map(cue => {
    const marker = el('div', { class: 'ff-player-expanded__cue-marker' });
    const label = el('span', { class: 'ff-player-expanded__cue-label' }, [cue.name]);
    return el('div', {
      class: 'ff-player-expanded__cue',
      style: `left: ${cue.position}%`
    }, [label, marker]);
  });

  const cuestrip = el('div', { class: 'ff-player-expanded__cuestrip' }, cueMarkers);

  // 3. Waveform Region
  const playhead = el('div', { class: 'ff-player-expanded__waveform-playhead', style: 'left: 30%' });
  const waveform = el('div', { class: 'ff-player-expanded__waveform' }, [playhead]);

  // 4. Beat Grid
  const beatGridChildren = [];
  // Fake beats: generate taller and smaller ticks
  for (let i = 0; i <= 64; i++) {
    const leftPct = (i / 64) * 100;
    const isBar = i % 4 === 0;
    const barNum = isBar ? (i / 4) + 1 : null;

    const tick = el('div', {
      class: `ff-player-expanded__beat-tick ${isBar ? 'ff-player-expanded__beat-tick--bar' : ''}`,
      style: `left: ${leftPct}%`
    });

    let numLabel = null;
    if (isBar) {
      numLabel = el('span', {
        class: 'ff-player-expanded__bar-num',
        style: `left: ${leftPct}%`
      }, [String(barNum)]);
    }

    beatGridChildren.push(tick);
    if (numLabel) {
      beatGridChildren.push(numLabel);
    }
  }
  const beatGrid = el('div', { class: 'ff-player-expanded__beat-grid' }, beatGridChildren);

  // 5. Controls Row
  const timeDisplay = el('span', { class: 'ff-player-expanded__time-display' }, [time]);

  const bpmNudge = el('span', { class: 'ff-player-expanded__nudge-placeholder' }, ['BPM −/+']);
  const keyNudge = el('span', { class: 'ff-player-expanded__nudge-placeholder' }, ['Key −/+']);
  const halfBtn = el('button', { class: 'ff-player-expanded__control-btn' }, ['½×']);
  const doubleBtn = el('button', { class: 'ff-player-expanded__control-btn' }, ['2×']);
  const setBeatBtn = el('button', { class: 'ff-player-expanded__control-btn' }, ['Set first beat']);

  const centerControls = el('div', { class: 'ff-player-expanded__center-controls' }, [
    bpmNudge,
    keyNudge,
    halfBtn,
    doubleBtn,
    setBeatBtn
  ]);

  const addCueBtn = el('button', { class: 'ff-player-expanded__add-cue-btn' }, ['Add cue']);

  const controlsRow = el('div', { class: 'ff-player-expanded__controls' }, [
    timeDisplay,
    centerControls,
    addCueBtn
  ]);

  // Main wrapper
  const element = el('div', { class: 'ff-player-expanded' }, [
    headRow,
    cuestrip,
    waveform,
    beatGrid,
    controlsRow
  ]);

  return { element };
}
