import './module-statusbar-v2.css';
import { el } from '../design-system/utils/dom.js';
import { createStatusBar, createLevelMeter, createIcon } from '../design-system/index.js';

export const meta = { title: 'Module · Status Bar v2 (master meter + settings)', layer: 'module' };

/*
 * Status bar v2: the v1 activity/count bar plus a master level meter and a
 * settings button on the right. Composed from the status-bar primitive (passing
 * the meter + settings via its rightExtras slot) so the full-width chrome and
 * left activity region are reused as-is. v1 is left untouched in the full UI.
 * Exports buildStatusBarV2() for future composites.
 */
function settingsButton() {
  return el('button', { class: 'ff-statusbar2__settings', title: 'Settings' }, [
    createIcon({ name: 'settings', size: 13 }).element,
  ]);
}

export function buildStatusBarV2() {
  const { element: meter } = createLevelMeter({ left: 0.72, right: 0.64 });

  const { element } = createStatusBar({
    message: 'Analyzing 14 of 24',
    showSpinner: true,
    context: 'ETA 1m 12s',
    count: '1,284 tracks · 6.4 GB',
    rightExtras: [meter, settingsButton()],
  });
  return el('div', { class: 'ff-statusbar2-module' }, [element]);
}

export default function render() {
  return el('div', { class: 'ff-statusbar2-demo' }, [buildStatusBarV2()]);
}
