import './status-bar.css';
import { el } from '../utils/dom.js';
import { createSpinner } from './spinner.js';
import { createProgressBar } from './progress-bar.js';

/**
 * @param {object} options
 * @param {string} options.message
 * @param {string} [options.context]
 * @param {string|number} [options.count]
 * @param {boolean} [options.showSpinner=false]
 * @param {number} [options.progress]          – 0-100 value. If provided, renders progress bar.
 */
export function createStatusBar({ message, context, count, showSpinner = false, progress } = {}) {
  const leftElements = [];

  if (showSpinner) {
    const { element: spinnerEl } = createSpinner({ size: 'small' });
    spinnerEl.classList.add('ff-status-bar__spinner');
    leftElements.push(spinnerEl);
  }

  leftElements.push(el('span', { class: 'ff-status-bar__message' }, [message]));

  if (context) {
    leftElements.push(el('span', { class: 'ff-status-bar__separator' }, ['·']));
    leftElements.push(el('span', { class: 'ff-status-bar__context' }, [context]));
  }

  const rightElements = [];

  if (progress !== undefined && progress !== null) {
    const { element: progressEl } = createProgressBar({ value: progress, variant: 'thin' });
    const progressWrapper = el('div', { class: 'ff-status-bar__progress-wrapper' }, [progressEl]);
    rightElements.push(progressWrapper);
  }

  if (count !== undefined && count !== null) {
    rightElements.push(el('span', { class: 'ff-status-bar__count' }, [String(count)]));
  }

  const leftRegion = el('div', { class: 'ff-status-bar__left' }, leftElements);
  const rightRegion = el('div', { class: 'ff-status-bar__right' }, rightElements);

  const element = el('div', { class: 'ff-status-bar' }, [
    leftRegion,
    rightRegion
  ]);

  return { element };
}
