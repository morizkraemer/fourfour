import { zoomLookBoard } from '#design-system/local/zoom-look.js';

export const meta = { title: 'C. Layered: soft bands under a signal outline', layer: 'composite', group: 'Zoom look (Q1-Q3)', order: 3 };

export default function render() {
  return zoomLookBoard('layered', 'C. Layered: soft bands under a signal outline', 'The band envelope stays as a soft fill underneath; the signal is drawn on top as a crisp outline that fades in as you zoom, a single line at sample level with dots.');
}
