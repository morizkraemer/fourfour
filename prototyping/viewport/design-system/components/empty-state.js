import './empty-state.css';
import { el } from '../utils/dom.js';

/**
 * @param {object} options
 * @param {string} [options.title]  – primary message
 * @param {string} [options.sub]    – secondary / helper text
 */
export function createEmptyState({ title = '', sub = '' } = {}) {
  const children = [];

  if (title) {
    children.push(el('p', { class: 'ff-empty-state__title' }, [title]));
  }

  if (sub) {
    children.push(el('p', { class: 'ff-empty-state__sub' }, [sub]));
  }

  const element = el('div', { class: 'ff-empty-state' }, children);

  return { element };
}
