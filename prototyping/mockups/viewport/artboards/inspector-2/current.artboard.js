import { host, W2InspBoards } from '#design-system/index.js';

export const meta = { title: 'Now', layer: 'composite', group: 'Inspector round 2 (F22)', order: 0 };

export default function render() {
  return host(W2InspBoards, { variant: 'now' });
}
