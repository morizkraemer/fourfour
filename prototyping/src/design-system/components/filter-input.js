import './filter-input.css';
import { el } from '../utils/dom.js';

const NS = 'http://www.w3.org/2000/svg';

function createSearchIcon() {
  const svg = document.createElementNS(NS, 'svg');
  svg.setAttribute('viewBox', '0 0 24 24');
  svg.setAttribute('width', '12');
  svg.setAttribute('height', '12');
  svg.setAttribute('fill', 'none');
  svg.setAttribute('stroke', 'currentColor');
  svg.setAttribute('stroke-width', '2.5');
  svg.setAttribute('stroke-linecap', 'round');
  svg.setAttribute('stroke-linejoin', 'round');
  svg.innerHTML = '<circle cx="11" cy="11" r="8" /><line x1="21" y1="21" x2="16.65" y2="16.65" />';
  return svg;
}

/**
 * @param {object} options
 * @param {string} [options.placeholder='Filter…']
 * @param {string|number} [options.count='']
 * @param {function} [options.onClose]
 * @param {function} [options.onChange]
 */
export function createFilterInput({ placeholder = 'Filter…', count = '', onClose, onChange } = {}) {
  const searchIcon = createSearchIcon();
  searchIcon.classList.add('ff-filter-input__search-icon');

  const inputEl = el('input', {
    class: 'ff-filter-input__field',
    type: 'text',
    placeholder,
    onInput: (e) => {
      if (typeof onChange === 'function') onChange(e.target.value);
    }
  });

  const countEl = el('span', { class: 'ff-filter-input__count' }, [count ? String(count) : '']);

  const closeBtn = el('button', {
    class: 'ff-filter-input__close-btn',
    onClick: (e) => {
      e.stopPropagation();
      if (typeof onClose === 'function') onClose();
    }
  }, ['×']);

  const element = el('div', { class: 'ff-filter-input' }, [
    searchIcon,
    inputEl,
    countEl,
    closeBtn
  ]);

  return { element, inputElement: inputEl, countElement: countEl };
}
