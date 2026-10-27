import { host, W3InspBoards } from '#design-system/index.js';

export const meta = { title: 'Now', layer: 'composite', group: 'Inspector round 3 (F22)', order: 0 };

export default function render() {
  return host(W3InspBoards, { variant: 'now' });
}
