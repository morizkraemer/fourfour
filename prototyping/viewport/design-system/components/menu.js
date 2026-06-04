import './menu.css';
import { el } from '../utils/dom.js';

/**
 * @param {object} options
 * @param {Array<{ label?: string, shortcut?: string, variant?: 'default'|'danger'|'disabled', disabled?: boolean, isSeparator?: boolean, onClick?: function }>} options.items
 */
export function createMenu({ items = [] } = {}) {
  const itemElements = items.map(item => {
    if (item.isSeparator) {
      return el('div', { class: 'ff-menu__separator' });
    }

    const variant = item.disabled ? 'disabled' : (item.variant || 'default');
    const classes = [
      'ff-menu__item',
      `ff-menu__item--${variant}`
    ].join(' ');

    const labelSpan = el('span', { class: 'ff-menu__item-label' }, [item.label || '']);
    const shortcutKbd = item.shortcut ? el('kbd', { class: 'ff-menu__item-shortcut' }, [item.shortcut]) : null;

    return el('div', {
      class: classes,
      onClick: (e) => {
        if (item.disabled) return;
        if (typeof item.onClick === 'function') item.onClick(e);
      }
    }, [labelSpan, shortcutKbd]);
  });

  const element = el('div', { class: 'ff-menu' }, itemElements);

  return { element };
}
