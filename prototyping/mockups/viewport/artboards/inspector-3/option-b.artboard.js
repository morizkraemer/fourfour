import { host, W3InspBoards } from '#design-system/index.js';

export const meta = { title: 'B. Hero and folds', layer: 'composite', group: 'Inspector round 3 (F22)', order: 2 };

export default function render() {
  return host(W3InspBoards, { variant: 'b' });
}
