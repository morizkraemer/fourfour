import './screen-browse.css';
import { el } from '../design-system/utils/dom.js';
import { buildSidebar } from './module-sidebar.artboard.js';
import { buildTable } from './module-table.artboard.js';
import { buildDetailPane } from './module-detail-pane.artboard.js';
import { buildGlobalHeader } from './module-global-header.artboard.js';
import { buildPlayer } from './module-player.artboard.js';
import { buildStatusBar } from './module-statusbar.artboard.js';

export const meta = { title: 'Screen · Browse (first window, 1440×900)', layer: 'composite' };

/*
 * The Browse first-screen — the app's resting state (ui_vision.md). Composes the
 * main-window modules AS-IS into the "main layout v3" window frame:
 *   [ sidebar 240 | body[ global-header / (table fill | detail 278) ] ]   ← main area
 *   global player (full width)
 *   status bar   (full width)
 * The global header sits inside the body column (right of the sidebar), not full
 * width. Composite adds no internal spacing; each module owns its own padding/width.
 */
export default function render() {
  const contentRow = el('div', { class: 'ff-browse__content' }, [
    el('div', { class: 'ff-browse__slot ff-browse__slot--table' }, [buildTable()]),
    el('div', { class: 'ff-browse__slot ff-browse__slot--detail' }, [buildDetailPane()]),
  ]);

  const body = el('div', { class: 'ff-browse__body' }, [
    el('div', { class: 'ff-browse__slot ff-browse__slot--header' }, [buildGlobalHeader()]),
    contentRow,
  ]);

  const mainArea = el('div', { class: 'ff-browse__main' }, [
    el('div', { class: 'ff-browse__slot ff-browse__slot--sidebar' }, [buildSidebar()]),
    body,
  ]);

  return el('div', { class: 'ff-browse' }, [
    mainArea,
    el('div', { class: 'ff-browse__slot ff-browse__slot--player' }, [buildPlayer()]),
    el('div', { class: 'ff-browse__slot ff-browse__slot--status' }, [buildStatusBar()]),
  ]);
}
