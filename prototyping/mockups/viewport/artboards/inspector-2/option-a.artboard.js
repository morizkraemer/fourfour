import { host, W2InspBoards } from '#design-system/index.js';

export const meta = { title: 'A. Form column, every value a ghost field', layer: 'composite', group: 'Inspector round 2 (F22)', order: 1 };

export default function render() {
  return host(W2InspBoards, { variant: 'a' });
}
