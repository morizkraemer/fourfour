import './panel-header.css';
import { el } from '../utils/dom.js';
import { createButton } from './button.js';

/**
 * @param {object} options
 * @param {string} [options.title]
 * @param {string} [options.subtitle]
 * @param {Array<{ label: string, variant?: 'ghost'|'default'|'primary'|'destructive', icon?: string, onClick?: function }>} [options.actions=[]]
 * @param {'library'|'playlist-source'|'usb-source'|'filtering'|'in-target-mode'} [options.variant='library']
 * @param {Element} [options.filterInput]  – if variant is 'filtering', the filter-input element to display
 * @param {Element} [options.targetChip]   – if variant is 'in-target-mode', the target-chip element to display
 */
export function createPanelHeader({
  title = '',
  subtitle = '',
  actions = [],
  variant = 'library',
  filterInput,
  targetChip
} = {}) {
  const leftChildren = [];

  if (variant === 'filtering' && filterInput) {
    leftChildren.push(filterInput);
  } else if (variant === 'in-target-mode' && targetChip) {
    leftChildren.push(targetChip);
  } else {
    // Standard Title + Subtitle
    if (title) {
      leftChildren.push(el('h2', { class: 'ff-panel-header__title' }, [title]));
    }
    if (subtitle) {
      leftChildren.push(el('span', { class: 'ff-panel-header__subtitle' }, [subtitle]));
    }
  }

  const rightChildren = [];
  if (variant !== 'filtering') {
    actions.forEach(action => {
      const btnVariant = action.variant || 'default';
      const { element: btn } = createButton({
        label: action.label,
        variant: btnVariant,
        icon: action.icon,
        size: 'small',
        onClick: action.onClick
      });
      rightChildren.push(btn);
    });
  }

  const leftArea = el('div', {
    class: `ff-panel-header__left ${variant === 'filtering' ? 'ff-panel-header__left--full' : ''}`
  }, leftChildren);

  const rightArea = el('div', { class: 'ff-panel-header__right' }, rightChildren);

  const element = el('div', {
    class: `ff-panel-header ff-panel-header--${variant}`
  }, [leftArea, rightArea]);

  return { element };
}
