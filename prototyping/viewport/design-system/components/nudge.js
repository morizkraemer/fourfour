import './nudge.css';
import { el } from '../utils/dom.js';

/**
 * @param {object} options
 * @param {string|number} options.value
 * @param {string} [options.label]           – aria-label / tooltip label
 * @param {function} [options.onChange]       – returns 'decrement' or 'increment'
 */
export function createNudge({ value, label = '', onChange, variant = 'default' } = {}) {
  const decBtn = el('button', {
    class: 'ff-nudge__btn ff-nudge__btn--dec',
    title: label ? `Decrease ${label}` : 'Decrease',
    'aria-label': 'Decrease',
    onClick: (e) => {
      e.stopPropagation();
      if (typeof onChange === 'function') onChange('decrement');
    }
  }, ['−']);

  const valEl = el('span', { class: 'ff-nudge__value' }, [String(value)]);

  const incBtn = el('button', {
    class: 'ff-nudge__btn ff-nudge__btn--inc',
    title: label ? `Increase ${label}` : 'Increase',
    'aria-label': 'Increase',
    onClick: (e) => {
      e.stopPropagation();
      if (typeof onChange === 'function') onChange('increment');
    }
  }, ['+']);

  const element = el('div', {
    class: `ff-nudge ${variant === 'compact' ? 'ff-nudge--compact' : ''} ${variant === 'large' ? 'ff-nudge--large' : ''}`,
    'aria-label': label
  }, [decBtn, valEl, incBtn]);

  return { element, valueElement: valEl };
}
