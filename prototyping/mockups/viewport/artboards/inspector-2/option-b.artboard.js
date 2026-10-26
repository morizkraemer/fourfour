import { host, W2InspBoards } from '#design-system/index.js';

export const meta = { title: 'B. Tiles and focused editors (key wheel open)', layer: 'composite', group: 'Inspector round 2 (F22)', order: 2 };

export default function render() {
  return host(W2InspBoards, { variant: 'b' });
}
