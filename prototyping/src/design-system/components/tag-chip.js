import './tag-chip.css';
import { el } from '../utils/dom.js';

const NS = 'http://www.w3.org/2000/svg';

function createPinIcon() {
  const svg = document.createElementNS(NS, 'svg');
  svg.setAttribute('viewBox', '0 0 24 24');
  svg.setAttribute('width', '10');
  svg.setAttribute('height', '10');
  svg.setAttribute('fill', 'currentColor');
  svg.innerHTML = '<path d="M21 10c0 7-9 13-9 13s-9-6-9-13a9 9 0 0 1 18 0z" /><circle cx="12" cy="10" r="3" fill="var(--ff-elev)" />';
  return svg;
}

/**
 * @param {object} options
 * @param {string|number} options.digit
 * @param {string} options.name
 */
export function createTagChip({ digit, name } = {}) {
  const digitEl = el('span', { class: 'ff-tag-chip__digit' }, [String(digit)]);
  const nameEl = el('span', { class: 'ff-tag-chip__name' }, [name]);

  const element = el('div', { class: 'ff-tag-chip' }, [digitEl, nameEl]);

  return { element };
}

/**
 * @param {object} options
 * @param {string|number} options.value
 */
export function createDigitPill({ value } = {}) {
  const element = el('div', { class: 'ff-digit-pill' }, [String(value)]);

  return { element };
}

/**
 * @param {object} options
 * @param {string} options.name
 * @param {function} [options.onClose]
 */
export function createTargetChip({ name, onClose } = {}) {
  const pinIcon = createPinIcon();
  pinIcon.classList.add('ff-target-chip__pin');

  const nameEl = el('span', { class: 'ff-target-chip__name' }, [name]);

  const closeBtn = el('button', {
    class: 'ff-target-chip__close-btn',
    onClick: (e) => {
      e.stopPropagation();
      if (typeof onClose === 'function') onClose();
    }
  }, ['×']);

  const element = el('div', { class: 'ff-target-chip' }, [
    pinIcon,
    nameEl,
    closeBtn
  ]);

  return { element };
}
