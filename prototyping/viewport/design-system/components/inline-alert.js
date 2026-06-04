import './inline-alert.css';
import { el } from '../utils/dom.js';

const NS = 'http://www.w3.org/2000/svg';

function createAlertIcon(variant) {
  const svg = document.createElementNS(NS, 'svg');
  svg.setAttribute('viewBox', '0 0 24 24');
  svg.setAttribute('width', '14');
  svg.setAttribute('height', '14');
  svg.setAttribute('fill', 'none');
  svg.setAttribute('stroke', 'currentColor');
  svg.setAttribute('stroke-width', '2');
  svg.setAttribute('stroke-linecap', 'round');
  svg.setAttribute('stroke-linejoin', 'round');

  if (variant === 'warn') {
    svg.innerHTML = '<path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3Z" /><line x1="12" y1="9" x2="12" y2="13" /><line x1="12" y1="17" x2="12.01" y2="17" />';
  } else {
    // info
    svg.innerHTML = '<circle cx="12" cy="12" r="10" /><line x1="12" y1="16" x2="12" y2="12" /><line x1="12" y1="8" x2="12.01" y2="8" />';
  }
  return svg;
}

/**
 * @param {object} options
 * @param {'info'|'warn'} [options.variant='info']
 * @param {string} options.html  – HTML string content for the alert body
 */
export function createInlineAlert({ variant = 'info', html } = {}) {
  const icon = createAlertIcon(variant);
  icon.classList.add('ff-inline-alert__icon');

  const body = el('div', { class: 'ff-inline-alert__body', html });

  const element = el('div', {
    class: `ff-inline-alert ff-inline-alert--${variant}`
  }, [icon, body]);

  return { element };
}
