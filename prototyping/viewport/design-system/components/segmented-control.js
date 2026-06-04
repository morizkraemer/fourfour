import './segmented-control.css';
import { el } from '../utils/dom.js';

/**
 * Segmented control — a row of mutually-exclusive toggle buttons.
 *   items: string[]          – button labels
 *   activeIndex: number      – initially selected index (default 0)
 *   onChange: (index) => void – called when the selection changes
 */
export function createSegmentedControl({
  items = [],
  activeIndex = 0,
  onChange,
} = {}) {
  const buttons = items.map((label, i) => {
    const classes = ['ff-segmented__item'];
    if (i === activeIndex) classes.push('ff-segmented__item--on');

    return el('button', {
      class: classes.join(' '),
      type: 'button',
      text: label,
      onClick: () => setActive(i),
    });
  });

  function setActive(index) {
    buttons.forEach((btn, i) => {
      btn.classList.toggle('ff-segmented__item--on', i === index);
    });
    if (onChange) onChange(index);
  }

  const element = el('div', { class: 'ff-segmented' }, buttons);

  return { element };
}
