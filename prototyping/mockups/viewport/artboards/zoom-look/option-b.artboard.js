import { zoomLookBoard } from '#design-system/local/zoom-look.js';

export const meta = { title: 'B. Signal: Ableton-style, one colour per column', layer: 'composite', group: 'Zoom look (Q1-Q3)', order: 2 };

export default function render() {
  return zoomLookBoard('signal', 'B. Signal: Ableton-style, one colour per column', 'Band envelope while zoomed out; closer in it fades into the real sample span (lowest to highest), filled in one colour mixed from the bands. Below a sample per pixel it is a line, with dots on the samples once they are 5 px apart.');
}
