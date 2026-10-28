import { zoomLookBoard } from '#design-system/local/zoom-look.js';

export const meta = { title: 'Now: morph (round 10)', layer: 'composite', group: 'Zoom look (Q1-Q3)', order: 0 };

export default function render() {
  return zoomLookBoard('morph', 'Now: morph (round 10)', 'The envelope span slides towards the sample span between 2 and 0.4 ms per pixel. In between it is a thin trace with the band colours squeezed inside it.');
}
