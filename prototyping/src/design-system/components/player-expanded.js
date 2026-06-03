import './player-expanded.css';
import { el } from '../utils/dom.js';
import { createButton } from './button.js';
import { createNudge } from './nudge.js';
import { createSegmentedControl } from './segmented-control.js';

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

/**
 * @param {object} options
 * @param {string} options.title
 * @param {string} options.artist
 * @param {string|number} options.bpm
 * @param {string} options.key
 * @param {string} options.time                – time string like "2:14 / 5:42"
 * @param {Array<{ color: string, position: number }>} [options.cues]
 * @param {function} [options.onSave]
 * @param {function} [options.onBpmChange]
 * @param {function} [options.onKeyChange]
 * @param {function} [options.onHalf]
 * @param {function} [options.onDouble]
 * @param {function} [options.onSetFirstBeat]
 * @param {function} [options.onAddCue]
 */
export function createPlayerExpanded({
  title,
  artist,
  bpm,
  key,
  time,
  cues = [],
  onSave,
  onBpmChange,
  onKeyChange,
  onHalf,
  onDouble,
  onSetFirstBeat,
  onAddCue
} = {}) {
  // 1. Head Row
  const playIcon = createPlayIcon();
  const playBtn = el('button', { class: 'ff-player-expanded__play-btn' }, [playIcon]);

  const titleGroup = el('div', { class: 'ff-player-expanded__title-group' }, [
    el('span', { class: 'ff-player-expanded__meta-title' }, [title]),
    el('span', { class: 'ff-player-expanded__meta-artist' }, [artist])
  ]);

  const bpmPill = el('div', { class: 'ff-player-expanded__meta-pill' }, [
    el('span', { class: 'ff-player-expanded__meta-pill-text' }, [String(bpm)])
  ]);
  const keyPill = el('div', { class: 'ff-player-expanded__meta-pill' }, [
    el('span', { class: 'ff-player-expanded__meta-pill-text' }, [key])
  ]);
  const timePill = el('div', { class: 'ff-player-expanded__meta-pill' }, [
    el('span', { class: 'ff-player-expanded__meta-pill-text' }, [time.split('/')[1]?.trim() || time])
  ]);

  const pills = el('div', { class: 'ff-player-expanded__meta-pills' }, [bpmPill, keyPill, timePill]);

  const pxhLeft = el('div', { class: 'ff-player-expanded__head-left' }, [
    playBtn,
    titleGroup,
    pills
  ]);

  const { element: seg } = createSegmentedControl({
    items: ['Color', 'Mono'],
    activeIndex: 0
  });

  const { element: saveBtn } = createButton({
    label: 'Save edits',
    variant: 'ghost',
    onClick: onSave
  });

  const pxhRight = el('div', { class: 'ff-player-expanded__head-right' }, [
    seg,
    saveBtn
  ]);

  const headRow = el('div', { class: 'ff-player-expanded__head' }, [pxhLeft, pxhRight]);

  // 2. Cuestrip (height: 18px)
  const defaultCues = cues.length > 0 ? cues : [
    { color: '#4ade80', position: 10 },
    { color: '#38bdf8', position: 35 },
    { color: '#a78bfa', position: 60 },
    { color: '#f59e0b', position: 85 }
  ];

  const cueElements = defaultCues.map(cue => {
    return el('div', {
      class: 'ff-player-expanded__cue-line',
      style: `left: ${cue.position}%; background-color: ${cue.color}`
    });
  });

  const cuestrip = el('div', { class: 'ff-player-expanded__cuestrip' }, cueElements);

  // 3. Waveform (height: 120px)
  const playhead = el('div', { class: 'ff-player-expanded__waveform-playhead', style: 'left: 30%' });
  const waveform = el('div', { class: 'ff-player-expanded__waveform' }, [playhead]);

  // 4. Beat Grid (height: 14px)
  const beatGridChildren = [];
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

  // 5. Controls Row (ctrl)
  const timeDisplay = el('span', { class: 'ff-player-expanded__time-display' }, [time]);

  // BPM nudge (using design system nudge primitive, compact 17px variant)
  const { element: bpmN } = createNudge({
    value: bpm,
    label: 'BPM',
    onChange: onBpmChange,
    variant: 'compact'
  });

  // Key toggle button (using design system button primitive, compact 17px variant)
  const { element: keyN } = createButton({
    label: key,
    variant: 'default',
    size: 'compact',
    onClick: (e) => {
      if (e) e.stopPropagation();
      if (typeof onKeyChange === 'function') onKeyChange();
    }
  });
  keyN.classList.add('ff-player-expanded__key-btn');

  const { element: halfBtn } = createButton({
    label: '½×',
    variant: 'ghost',
    size: 'compact',
    onClick: onHalf
  });

  const { element: doubleBtn } = createButton({
    label: '2×',
    variant: 'ghost',
    size: 'compact',
    onClick: onDouble
  });

  const { element: setBeatBtn } = createButton({
    label: 'Set first beat',
    variant: 'ghost',
    size: 'compact',
    onClick: onSetFirstBeat
  });

  const centerControls = el('div', { class: 'ff-player-expanded__center-controls' }, [
    bpmN,
    keyN,
    halfBtn,
    doubleBtn,
    setBeatBtn
  ]);

  const { element: addCueBtn } = createButton({
    label: 'Add cue',
    variant: 'default',
    size: 'compact',
    onClick: onAddCue
  });

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
