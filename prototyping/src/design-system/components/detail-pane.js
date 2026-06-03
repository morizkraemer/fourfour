import './detail-pane.css';
import { el } from '../utils/dom.js';

const NS = 'http://www.w3.org/2000/svg';

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

/**
 * @param {object} options
 * @param {'single'|'multi'|'empty'|'missing'} options.mode
 * @param {string} [options.title]
 * @param {string} [options.artist]
 * @param {object} [options.meta]             – key-value pairs for track meta info
 * @param {Array<{ name: string, position: string }>} [options.cues]
 * @param {number} [options.multiCount]       – selected tracks count (for multi mode)
 */
export function createDetailPane({
  mode = 'empty',
  title = '',
  artist = '',
  meta = {},
  cues = [],
  multiCount = 0
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

    // Art Placeholder
    const discIcon = createDiscIcon(80);
    discIcon.classList.add('ff-detail-pane__art-icon');
    const art = el('div', {
      class: `ff-detail-pane__art ${isMissing ? 'ff-detail-pane__art--missing' : ''}`
    }, [discIcon]);

    containerChildren.push(art);

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
    const defaultMetaKeys = ['BPM', 'Key', 'Time', 'Genre', 'Label', 'Added'];
    defaultMetaKeys.forEach(key => {
      let val = isMissing ? '—' : (meta[key] || '—');
      dlItems.push(el('dt', { class: 'ff-detail-pane__dt' }, [key]));
      dlItems.push(el('dd', { class: 'ff-detail-pane__dd' }, [val]));
    });

    const metaList = el('dl', { class: 'ff-detail-pane__meta-list' }, dlItems);
    containerChildren.push(metaList);

    // Cues (if single mode and cues provided)
    if (!isMissing && cues && cues.length > 0) {
      const cueHeader = el('div', { class: 'ff-detail-pane__section-title' }, ['Cue Points']);
      const cueRows = cues.map(cue => {
        const pip = el('div', { class: 'ff-detail-pane__cue-pip' });
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
          ...cueRows
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
    const defaultMetaKeys = ['Total Size', 'Total Duration', 'BPM Range', 'Keys'];
    defaultMetaKeys.forEach(key => {
      let val = meta[key] || '—';
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
