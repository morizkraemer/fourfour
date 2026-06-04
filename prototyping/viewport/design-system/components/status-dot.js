import './status-dot.css';
import { el } from '../utils/dom.js';

/*
 * Status dot. 6px circle used on USB device rows (§6).
 *   status: 'online' | 'warn' | 'offline'
 */
export function createStatusDot({ status = 'online' } = {}) {
  return { element: el('span', { class: `ff-status-dot ff-status-dot--${status}` }) };
}
