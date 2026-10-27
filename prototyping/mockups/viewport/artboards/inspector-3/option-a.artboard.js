import { host, W3InspBoards } from '#design-system/index.js';

export const meta = { title: 'A. Document, one quiet column', layer: 'composite', group: 'Inspector round 3 (F22)', order: 1 };

export default function render() {
  return host(W3InspBoards, { variant: 'a' });
}
