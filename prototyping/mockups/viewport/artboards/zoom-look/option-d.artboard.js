import { zoomLookBoard } from '#design-system/local/zoom-look.js';

export const meta = { title: 'D. Filled: the signal filled to the centre line, bands inside', layer: 'composite', group: 'Zoom look (Q1-Q3)', order: 4 };

export default function render() {
  return zoomLookBoard('filled', 'D. Filled: the signal filled to the centre line, bands inside', 'The outline is the real signal, but it is always filled to the centre line and the 3 bands stay nested inside it from the centre out, so the body never thins to a trace. Dots on the samples once they are 5 px apart.');
}
