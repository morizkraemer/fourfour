import { host, W2InspBoards } from '#design-system/index.js';

export const meta = { title: 'C. Prep and Tags (Prep)', layer: 'composite', group: 'Inspector round 2 (F22)', order: 4 };

export default function render() {
  return host(W2InspBoards, { variant: 'c' });
}
