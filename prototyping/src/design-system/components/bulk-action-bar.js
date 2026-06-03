import './bulk-action-bar.css';
import { el } from '../utils/dom.js';
import { createButton } from './button.js';

/**
 * @param {object} options
 * @param {string|number} options.count
 * @param {string} options.summary
 * @param {Array<{ label: string, variant?: 'ghost'|'default', onClick?: function, withEsc?: boolean }>} options.actions
 */
export function createBulkActionBar({ count, summary, actions = [] } = {}) {
  const left = el('div', { class: 'ff-bulk-bar__left' }, [
    el('span', { class: 'ff-bulk-bar__count' }, [`${count} tracks`]),
    el('span', { class: 'ff-bulk-bar__separator' }, ['·']),
    el('span', { class: 'ff-bulk-bar__summary' }, [summary])
  ]);

  const actionElements = actions.map(act => {
    const variant = act.variant || 'default';
    const { element: btn } = createButton({
      label: act.label,
      variant: variant,
      size: 'small',
      kbd: act.withEsc ? 'esc' : undefined,
      onClick: act.onClick
    });
    return btn;
  });

  const right = el('div', { class: 'ff-bulk-bar__right' }, actionElements);

  const element = el('div', { class: 'ff-bulk-bar' }, [left, right]);

  return { element };
}
