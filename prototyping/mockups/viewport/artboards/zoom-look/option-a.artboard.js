import { zoomLookBoard } from '#design-system/local/zoom-look.js';

export const meta = { title: 'A. Body: Rekordbox-style, always a filled 3-band body', layer: 'composite', group: 'Zoom look (Q1-Q3)', order: 1 };

export default function render() {
  return zoomLookBoard('body', 'A. Body: Rekordbox-style, always a filled 3-band body', "Symmetric about the centre at every zoom. Its outline sharpens from the band envelope to each column's loudest sample; the signal's sign and the samples never show. Never a line.");
}
