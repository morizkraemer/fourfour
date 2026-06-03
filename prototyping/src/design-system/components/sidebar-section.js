import './sidebar-section.css';
import { el } from '../utils/dom.js';

const NS = 'http://www.w3.org/2000/svg';

function createPlusIcon() {
  const svg = document.createElementNS(NS, 'svg');
  svg.setAttribute('viewBox', '0 0 24 24');
  svg.setAttribute('width', '13');
  svg.setAttribute('height', '13');
  svg.setAttribute('fill', 'none');
  svg.setAttribute('stroke', 'currentColor');
  svg.setAttribute('stroke-width', '2');
  svg.setAttribute('stroke-linecap', 'round');
  svg.setAttribute('stroke-linejoin', 'round');
  svg.innerHTML = '<line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" />';
  return svg;
}

/**
 * @param {object} options
 * @param {string} options.label
 * @param {Array<Element>} options.rows
 * @param {boolean} [options.showAdd=false]
 * @param {function} [options.onAdd]
 */
export function createSidebarSection({ label, rows = [], showAdd = false, onAdd } = {}) {
  const labelEl = el('span', { class: 'ff-sidebar-section__label' }, [label]);

  const headElements = [labelEl];

  if (showAdd) {
    const plusIcon = createPlusIcon();
    plusIcon.classList.add('ff-sidebar-section__add-icon');

    const addBtn = el('button', {
      class: 'ff-sidebar-section__add-btn',
      onClick: (e) => {
        e.stopPropagation();
        if (typeof onAdd === 'function') onAdd();
      }
    }, [plusIcon]);
    headElements.push(addBtn);
  }

  const head = el('div', { class: 'ff-sidebar-section__head' }, headElements);
  const rowsContainer = el('div', { class: 'ff-sidebar-section__rows' }, rows);

  const element = el('div', { class: 'ff-sidebar-section' }, [
    head,
    rowsContainer
  ]);

  return { element };
}
