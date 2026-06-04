import './dialog.css';
import { el } from '../utils/dom.js';
import { createButton } from './button.js';

/**
 * @param {object} options
 * @param {string} options.title
 * @param {string} options.body
 * @param {Array<{ label: string, variant?: 'ghost'|'primary'|'destructive', onClick?: function }>} options.actions
 * @param {boolean} [options.withBackdrop=true]
 */
export function createDialog({ title, body, actions = [], withBackdrop = true } = {}) {
  const actionElements = actions.map(act => {
    const variant = act.variant || 'default';
    const { element: btnEl } = createButton({
      label: act.label,
      variant: variant,
      onClick: act.onClick
    });
    return btnEl;
  });

  const dialogElement = el('div', { class: 'ff-dialog' }, [
    el('h2', { class: 'ff-dialog__title' }, [title]),
    el('div', { class: 'ff-dialog__body' }, [body]),
    el('div', { class: 'ff-dialog__actions' }, actionElements),
  ]);

  if (withBackdrop) {
    const wrapper = el('div', { class: 'ff-dialog-wrapper' }, [
      el('div', { class: 'ff-dialog-backdrop' }),
      dialogElement
    ]);
    return { element: wrapper, dialogElement };
  }

  return { element: dialogElement, dialogElement };
}
