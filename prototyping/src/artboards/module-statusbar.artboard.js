import './module-statusbar.css';
import { el } from '../design-system/utils/dom.js';
import { createStatusBar } from '../design-system/index.js';

export const meta = { title: 'Module · Status Bar (full width)', layer: 'module' };

/*
 * Bottom status bar (Pencil "Statusbar"): left = analysis/idle activity,
 * right = library totals. Exports buildStatusBar() for the Browse composite.
 */
export function buildStatusBar() {
  const { element } = createStatusBar({
    message: 'Analyzing 14 of 24',
    showSpinner: true,
    context: 'ETA 1m 12s',
    count: '1,284 tracks · 6.4 GB',
  });
  return el('div', { class: 'ff-statusbar-module' }, [element]);
}

export default function render() {
  return el('div', { class: 'ff-statusbar-demo' }, [buildStatusBar()]);
}
