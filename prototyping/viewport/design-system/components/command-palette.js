import './command-palette.css';
import { el } from '../utils/dom.js';

const NS = 'http://www.w3.org/2000/svg';

function createSearchIcon() {
  const svg = document.createElementNS(NS, 'svg');
  svg.setAttribute('viewBox', '0 0 24 24');
  svg.setAttribute('width', '14');
  svg.setAttribute('height', '14');
  svg.setAttribute('fill', 'none');
  svg.setAttribute('stroke', 'currentColor');
  svg.setAttribute('stroke-width', '2');
  svg.setAttribute('stroke-linecap', 'round');
  svg.setAttribute('stroke-linejoin', 'round');
  svg.innerHTML = '<circle cx="11" cy="11" r="8" /><line x1="21" y1="21" x2="16.65" y2="16.65" />';
  return svg;
}

function createItemIcon() {
  const svg = document.createElementNS(NS, 'svg');
  svg.setAttribute('viewBox', '0 0 24 24');
  svg.setAttribute('width', '14');
  svg.setAttribute('height', '14');
  svg.setAttribute('fill', 'none');
  svg.setAttribute('stroke', 'currentColor');
  svg.setAttribute('stroke-width', '2');
  svg.setAttribute('stroke-linecap', 'round');
  svg.setAttribute('stroke-linejoin', 'round');
  svg.innerHTML = '<circle cx="12" cy="12" r="10" /><polygon points="10 8 16 12 10 16 10 8" />';
  return svg;
}

/**
 * @param {object} options
 * @param {Array<{ title: string, items: Array<{ icon?: Element, name: string, sub?: string, right?: string, onClick?: function }> }>} options.sections
 */
export function createCommandPalette({ sections = [] } = {}) {
  const searchIcon = createSearchIcon();
  searchIcon.classList.add('ff-cp__search-icon');

  const inputEl = el('input', {
    class: 'ff-cp__input',
    type: 'text',
    placeholder: 'Search…',
  });

  const escKbd = el('kbd', { class: 'ff-cp__esc-kbd' }, ['esc']);

  const inputRow = el('div', { class: 'ff-cp__input-row' }, [
    searchIcon,
    inputEl,
    escKbd,
  ]);

  const bodyChildren = [];
  let totalCount = 0;

  sections.forEach((section, sIdx) => {
    // Section Header
    const headEl = el('div', { class: 'ff-cp__section-head' }, [section.title]);
    bodyChildren.push(headEl);

    // Section Items
    section.items.forEach(item => {
      totalCount++;
      const itemIcon = item.icon || createItemIcon();
      itemIcon.classList.add('ff-cp__row-icon');

      const nameEl = el('span', { class: 'ff-cp__row-name' }, [
        item.name,
        item.sub ? el('span', { class: 'ff-cp__row-sub' }, [` ${item.sub}`]) : null
      ]);

      const rightEl = item.right ? el('span', { class: 'ff-cp__row-right' }, [item.right]) : null;

      const rowEl = el('div', {
        class: 'ff-cp__row',
        onClick: (e) => {
          if (typeof item.onClick === 'function') item.onClick(e);
        }
      }, [
        itemIcon,
        nameEl,
        rightEl
      ]);

      bodyChildren.push(rowEl);
    });
  });

  const contentArea = el('div', { class: 'ff-cp__content' }, bodyChildren);

  const footer = el('div', { class: 'ff-cp__footer' }, [
    el('span', { class: 'ff-cp__footer-left' }, ['↑↓ navigate  ⏎ open']),
    el('span', { class: 'ff-cp__footer-right' }, [`${totalCount} results`]),
  ]);

  const element = el('div', { class: 'ff-cp' }, [
    inputRow,
    contentArea,
    footer,
  ]);

  return { element, inputElement: inputEl };
}
