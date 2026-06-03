import './detail-pane.css';
import { el } from '../utils/dom.js';

const NS = 'http://www.w3.org/2000/svg';

function lookupKey(obj, key) {
  if (!obj) return undefined;
  if (obj[key] !== undefined) return obj[key];
  if (obj[key.toLowerCase()] !== undefined) return obj[key.toLowerCase()];
  const found = Object.keys(obj).find(k => k.toLowerCase() === key.toLowerCase());
  return found ? obj[found] : undefined;
}

function createMusicIcon() {
  const svg = document.createElementNS(NS, 'svg');
  svg.setAttribute('viewBox', '0 0 24 24');
  svg.setAttribute('width', '146');
  svg.setAttribute('height', '146');
  svg.setAttribute('fill', 'none');
  svg.setAttribute('stroke', 'currentColor');
  svg.setAttribute('stroke-width', '1.5');
  svg.setAttribute('stroke-linecap', 'round');
  svg.setAttribute('stroke-linejoin', 'round');
  // Lucide music icon
  svg.innerHTML = '<path d="M9 18V5l12-2v13" /><circle cx="6" cy="18" r="3" /><circle cx="18" cy="16" r="3" />';
  return svg;
}

function createDiscIcon(size) {
  const svg = document.createElementNS(NS, 'svg');
  svg.setAttribute('viewBox', '0 0 24 24');
  svg.setAttribute('width', String(size));
  svg.setAttribute('height', String(size));
  svg.setAttribute('fill', 'none');
  svg.setAttribute('stroke', 'currentColor');
  svg.setAttribute('stroke-width', '2');
  svg.setAttribute('stroke-linecap', 'round');
  svg.setAttribute('stroke-linejoin', 'round');
  svg.innerHTML = '<circle cx="12" cy="12" r="10" /><circle cx="12" cy="12" r="2" />';
  return svg;
}

// Cue color palette matching the track-row and cuestrip CDJ colors
const CUE_COLORS = [
  '#ec4899', // pink
  '#38bdf8', // blue
  '#4ade80', // green
  '#f59e0b', // orange
  '#a78bfa', // purple
  '#facc15', // yellow
  '#94a3b8'  // grey
];

/**
 * @param {object} options
 * @param {'single'|'multi'|'empty'|'missing'} options.mode
 * @param {string} [options.title]
 * @param {string} [options.artist]
 * @param {object} [options.meta]             – key-value pairs for track meta info
 * @param {Array<{ name: string, position: string, color?: string }>} [options.cues]
 * @param {number} [options.multiCount]       – selected tracks count (for multi mode)
 * @param {string} [options.filePath]         – file path of the track
 */
export function createDetailPane({
  mode = 'empty',
  title = '',
  artist = '',
  meta = {},
  cues = [],
  multiCount = 0,
  filePath = ''
} = {}) {
  const containerChildren = [];

  if (mode === 'empty') {
    const discIcon = createDiscIcon(40);
    discIcon.classList.add('ff-detail-pane__empty-icon');

    containerChildren.push(
      el('div', { class: 'ff-detail-pane__empty-state' }, [
        discIcon,
        el('p', { class: 'ff-detail-pane__empty-label' }, ['No track selected']),
        el('p', { class: 'ff-detail-pane__empty-sub' }, ['Select a track to view details'])
      ])
    );
  } else if (mode === 'single' || mode === 'missing') {
    const isMissing = mode === 'missing';

    // Cover Art Placeholder (height: 292px)
    const musicIcon = createMusicIcon();
    musicIcon.classList.add('ff-detail-pane__art-icon');
    const cover = el('div', {
      class: `ff-detail-pane__cover ${isMissing ? 'ff-detail-pane__cover--missing' : ''}`
    }, [musicIcon]);

    containerChildren.push(cover);

    // Title & Artist
    containerChildren.push(
      el('div', { class: 'ff-detail-pane__header' }, [
        el('h2', { class: 'ff-detail-pane__title' }, [title || 'Untitled Track']),
        el('p', { class: 'ff-detail-pane__artist' }, [artist || 'Unknown Artist'])
      ])
    );

    // Alert if missing
    if (isMissing) {
      containerChildren.push(
        el('div', { class: 'ff-detail-pane__warn-alert' }, ['File not found on disk'])
      );
    }

    // Meta list
    const dlItems = [];
    const defaultMetaKeys = ['ALBUM', 'LABEL', 'BPM · KEY', 'DURATION', 'ADDED'];
    defaultMetaKeys.forEach(key => {
      let val = '—';
      if (!isMissing) {
        if (key === 'BPM · KEY') {
          const bpmVal = lookupKey(meta, 'BPM') || '—';
          const keyVal = lookupKey(meta, 'KEY') || '—';
          val = (bpmVal !== '—' || keyVal !== '—') ? `${bpmVal} · ${keyVal}` : '—';
        } else {
          val = lookupKey(meta, key) || '—';
        }
      }
      dlItems.push(el('dt', { class: 'ff-detail-pane__dt' }, [key]));
      dlItems.push(el('dd', { class: 'ff-detail-pane__dd' }, [val]));
    });

    const metaList = el('dl', { class: 'ff-detail-pane__meta-list' }, dlItems);
    containerChildren.push(metaList);

    // Cues
    if (!isMissing && cues && cues.length > 0) {
      const cueHeader = el('div', { class: 'ff-detail-pane__section-title' }, ['CUE POINTS']);
      const cueRows = cues.map((cue, idx) => {
        const color = cue.color || CUE_COLORS[idx % CUE_COLORS.length];
        const pip = el('div', {
          class: 'ff-detail-pane__cue-pip',
          style: `background-color: ${color}`
        });
        const nameEl = el('span', { class: 'ff-detail-pane__cue-name' }, [cue.name]);
        const posEl = el('span', { class: 'ff-detail-pane__cue-pos' }, [cue.position]);

        return el('div', { class: 'ff-detail-pane__cue-item' }, [
          pip,
          nameEl,
          posEl
        ]);
      });

      containerChildren.push(
        el('div', { class: 'ff-detail-pane__cues-section' }, [
          cueHeader,
          el('div', { class: 'ff-detail-pane__cues-list' }, cueRows)
        ])
      );
    }

    // File Path section
    const displayPath = filePath || (isMissing ? '/Volumes/Music/Tale Of Us/Endless Run/02 Voidwalker.flac' : '');
    if (displayPath) {
      containerChildren.push(
        el('div', { class: 'ff-detail-pane__file-path-section' }, [
          el('div', { class: 'ff-detail-pane__section-title' }, ['FILE PATH']),
          el('div', { class: 'ff-detail-pane__file-path-val' }, [displayPath])
        ])
      );
    }

    // Actions if missing
    if (isMissing) {
      const locateBtn = el('button', { class: 'ff-detail-pane__action-link' }, ['Locate file']);
      const reanalyzeBtn = el('button', { class: 'ff-detail-pane__action-link' }, ['Reanalyze']);
      const removeBtn = el('button', { class: 'ff-detail-pane__action-link ff-detail-pane__action-link--destructive' }, ['Remove']);

      containerChildren.push(
        el('div', { class: 'ff-detail-pane__actions' }, [
          locateBtn,
          reanalyzeBtn,
          removeBtn
        ])
      );
    }

  } else if (mode === 'multi') {
    const countDisplay = el('div', { class: 'ff-detail-pane__count-display' }, [String(multiCount)]);
    const countLabel = el('div', { class: 'ff-detail-pane__count-label' }, ['tracks selected']);

    // Meta list with totals
    const dlItems = [];
    const defaultMetaKeys = ['TOTAL SIZE', 'TOTAL DURATION', 'BPM RANGE', 'KEYS'];
    defaultMetaKeys.forEach(key => {
      let val = lookupKey(meta, key) || '—';
      dlItems.push(el('dt', { class: 'ff-detail-pane__dt' }, [key]));
      dlItems.push(el('dd', { class: 'ff-detail-pane__dd' }, [val]));
    });

    const metaList = el('dl', { class: 'ff-detail-pane__meta-list' }, dlItems);

    containerChildren.push(
      countDisplay,
      countLabel,
      metaList
    );
  }

  const element = el('div', { class: 'ff-detail-pane' }, containerChildren);

  return { element };
}
