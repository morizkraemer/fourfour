import './toast.css';
import { el } from '../utils/dom.js';

const NS = 'http://www.w3.org/2000/svg';

function createSvgIcon(type) {
  const svg = document.createElementNS(NS, 'svg');
  svg.setAttribute('viewBox', '0 0 24 24');
  svg.setAttribute('width', '14');
  svg.setAttribute('height', '14');
  svg.setAttribute('fill', 'none');
  svg.setAttribute('stroke', 'currentColor');
  svg.setAttribute('stroke-width', '2');
  svg.setAttribute('stroke-linecap', 'round');
  svg.setAttribute('stroke-linejoin', 'round');

  if (type === 'success') {
    svg.innerHTML = '<path d="M22 11.08V12a10 10 0 1 1-5.93-9.14" /><polyline points="22 4 12 14.01 9 11.01" />';
  } else if (type === 'error') {
    svg.innerHTML = '<circle cx="12" cy="12" r="10" /><line x1="15" y1="9" x2="9" y2="15" /><line x1="9" y1="9" x2="15" y2="15" />';
  } else {
    // default/disc
    svg.innerHTML = '<circle cx="12" cy="12" r="10" /><circle cx="12" cy="12" r="2" />';
  }
  return svg;
}

/**
 * @param {object} options
 * @param {string} options.message
 * @param {string} [options.sub]
 * @param {'default'|'success'|'error'} [options.variant='default']
 * @param {boolean} [options.showUndo=false]
 * @param {function} [options.onUndo]
 */
export function createToast({ message, sub, variant = 'default', showUndo = false, onUndo } = {}) {
  const icon = createSvgIcon(variant);
  icon.classList.add('ff-toast__icon');

  const textSection = el('div', { class: 'ff-toast__text-section' }, [
    el('div', { class: 'ff-toast__message' }, [message]),
    sub ? el('div', { class: 'ff-toast__sub' }, [sub]) : null,
  ]);

  const children = [icon, textSection];

  if (showUndo) {
    const undoButton = el('span', {
      class: 'ff-toast__undo',
      onClick: (e) => {
        e.stopPropagation();
        if (typeof onUndo === 'function') onUndo();
      }
    }, ['Undo']);
    children.push(undoButton);
  }

  const element = el('div', {
    class: `ff-toast ff-toast--${variant}`
  }, children);

  return { element };
}
