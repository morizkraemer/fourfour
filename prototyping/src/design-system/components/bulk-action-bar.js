import './bulk-action-bar.css';
import { el } from '../utils/dom.js';

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
    const children = [el('span', {}, [act.label])];

    if (act.withEsc) {
      children.push(el('kbd', { class: 'ff-bulk-bar__esc-kbd' }, ['esc']));
    }

    return el('button', {
      class: `ff-bulk-bar__action ff-bulk-bar__action--${variant}`,
      onClick: (e) => {
        if (typeof act.onClick === 'function') act.onClick(e);
      }
    }, children);
  });

  const right = el('div', { class: 'ff-bulk-bar__right' }, actionElements);

  const element = el('div', { class: 'ff-bulk-bar' }, [left, right]);

  return { element };
}
