import { host, W2InspBoards } from '#design-system/index.js';

export const meta = { title: 'B. Tiles, cue editor open', layer: 'composite', group: 'Inspector round 2 (F22)', order: 3 };

export default function render() {
  return host(W2InspBoards, { variant: 'b-cue' });
}
