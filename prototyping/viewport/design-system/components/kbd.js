import './kbd.css';
import { el } from '../utils/dom.js';

/**
 * Keyboard hint — inline visual for a keyboard key.
 * Label-only, never interactive.
 *   key: display string, e.g. '⌘K', 'Esc', '⏎'
 */
export function createKbd({ key } = {}) {
  return {
    element: el('kbd', { class: 'ff-kbd', text: key }),
  };
}
