import './progress-bar.css';
import { el } from '../utils/dom.js';

/**
 * @param {object} options
 * @param {number}  [options.value=0]        – 0-100 progress value
 * @param {'determinate'|'indeterminate'|'thin'} [options.variant='determinate']
 */
export function createProgressBar({ value = 0, variant = 'determinate' } = {}) {
  const isThin = variant === 'thin';
  const isIndeterminate = variant === 'indeterminate';

  const fillStyle = !isIndeterminate ? `width:${Math.min(100, Math.max(0, value))}%` : undefined;

  const fill = el('div', { class: 'ff-progress-bar__fill', style: fillStyle });

  const modifiers = [
    isThin ? 'ff-progress-bar--thin' : '',
    isIndeterminate ? 'ff-progress-bar--indeterminate' : 'ff-progress-bar--determinate',
  ]
    .filter(Boolean)
    .join(' ');

  const track = el(
    'div',
    {
      class: `ff-progress-bar ${modifiers}`,
      role: 'progressbar',
      'aria-valuenow': isIndeterminate ? undefined : value,
      'aria-valuemin': 0,
      'aria-valuemax': 100,
    },
    [fill],
  );

  return { element: track };
}
