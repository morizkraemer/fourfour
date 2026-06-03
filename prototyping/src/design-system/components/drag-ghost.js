import './drag-ghost.css';
import { el } from '../utils/dom.js';

/**
 * @param {object} options
 * @param {string} [options.label]
 * @param {number} [options.count=1]
 */
export function createDragGhost({ label = '', count = 1 } = {}) {
  const children = [];

  if (count > 1) {
    const badge = el('span', { class: 'ff-drag-ghost__badge' }, [String(count)]);
    const labelText = el('span', { class: 'ff-drag-ghost__label' }, [`${count} tracks`]);
    children.push(badge, labelText);
  } else {
    children.push(el('span', { class: 'ff-drag-ghost__label' }, [label || '1 track']));
  }

  const element = el('div', { class: 'ff-drag-ghost' }, children);

  return { element };
}

export function createDropLine() {
  const line = el('div', { class: 'ff-drop-line__line' });
  const marker = el('div', { class: 'ff-drop-line__marker' });

  const element = el('div', { class: 'ff-drop-line' }, [marker, line]);

  return { element };
}

/**
 * @param {object} options
 * @param {string} options.title
 * @param {string} [options.sub]
 */
export function createDropOverlay({ title, sub = '' } = {}) {
  const children = [el('div', { class: 'ff-drop-overlay__title' }, [title])];

  if (sub) {
    children.push(el('div', { class: 'ff-drop-overlay__sub' }, [sub]));
  }

  const element = el('div', { class: 'ff-drop-overlay' }, children);

  return { element };
}
