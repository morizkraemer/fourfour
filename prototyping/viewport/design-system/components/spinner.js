import './spinner.css';
import { el } from '../utils/dom.js';

/**
 * @param {object} options
 * @param {'default'|'small'} [options.size='default']
 */
export function createSpinner({ size = 'default' } = {}) {
  const classes = ['ff-spinner', size === 'small' ? 'ff-spinner--small' : '']
    .filter(Boolean)
    .join(' ');

  const element = el('div', { class: classes, role: 'status', 'aria-label': 'Loading' });

  return { element };
}
