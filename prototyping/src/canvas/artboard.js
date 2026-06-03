import './artboard.css';
import { el } from '../design-system/utils/dom.js';

const pinIconHtml = `
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <path d="M12 17v5" />
    <path d="M9 10.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V6a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1z" />
  </svg>
`;

/**
 * Wraps an artboard's content element in a titled card with a live-dims
 * header and action controls (Pin/Rename).
 */
export function createArtboard(title, contentEl, { layer = 'primitive' } = {}) {
  const dims = el('span', { class: 'ff-card__dims', text: '' });
  const titleEl = el('span', { class: 'ff-card__title', text: title });

  const renameBtn = el('button', { class: 'ff-card__rename', text: '✎', type: 'button' });
  renameBtn.setAttribute('aria-label', `Rename "${title}"`);
  renameBtn.hidden = true;

  const pinBtn = el('button', { class: 'ff-card__pin', type: 'button' });
  pinBtn.setAttribute('aria-label', `Pin "${title}"`);
  pinBtn.setAttribute('aria-pressed', 'false');
  pinBtn.innerHTML = pinIconHtml;

  const titleRow = el('div', { class: 'ff-card__title-row' }, [
    titleEl,
    renameBtn,
    pinBtn,
  ]);

  const header = el('div', { class: 'ff-card__header' }, [
    titleRow,
    dims,
  ]);
  const body = el('div', { class: 'ff-card__body' }, [contentEl]);
  const card = el('div', { class: 'ff-card', dataset: { layer } }, [header, body]);

  const update = () => {
    dims.textContent = `${Math.round(body.offsetWidth)} × ${Math.round(body.offsetHeight)}`;
  };
  requestAnimationFrame(update);
  new ResizeObserver(update).observe(body);

  card.handle = titleRow;
  card.titleEl = titleEl;
  card.renameBtn = renameBtn;
  card.pinBtn = pinBtn;
  card.bodyEl = body;
  return card;
}
