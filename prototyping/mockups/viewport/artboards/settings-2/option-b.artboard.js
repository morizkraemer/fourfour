import { host, W2SetBoards } from '#design-system/index.js';

export const meta = { title: 'B. Settings in the palette', layer: 'composite', group: 'Settings round 2 (F21)', order: 6 };

export default function render() {
  return host(W2SetBoards, { variant: 'b' });
}
