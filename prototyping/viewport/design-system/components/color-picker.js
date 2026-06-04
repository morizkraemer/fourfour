import './color-picker.css';
import { el } from '../utils/dom.js';

/**
 * CDJ color picker – choose one of 7 track colors, or none.
 *   selected: 0 (none) or 1–7
 *   onChange: (index) => void
 */
export function createColorPicker({ selected = 0, onChange } = {}) {
  let current = selected;
  const chips = [];

  function updateSelection(index) {
    current = index;
    chips.forEach((chip, i) => {
      chip.classList.toggle('ff-color-picker__chip--selected', i === current);
    });
  }

  function handleClick(index) {
    updateSelection(index);
    if (onChange) onChange(current);
  }

  // None chip (index 0)
  const noneChip = el('button', {
    class: 'ff-color-picker__chip ff-color-picker__chip--none',
    type: 'button',
    'aria-label': 'No color',
    text: '×',
    onClick: () => handleClick(0),
  });
  chips.push(noneChip);

  // Color chips 1–7
  for (let i = 1; i <= 7; i++) {
    const chip = el('button', {
      class: `ff-color-picker__chip ff-color-picker__chip--${i}`,
      type: 'button',
      'aria-label': `Color ${i}`,
      onClick: () => handleClick(i),
    });
    chips.push(chip);
  }

  // Apply initial selection
  updateSelection(current);

  const container = el('div', { class: 'ff-color-picker' }, chips);

  return { element: container };
}
