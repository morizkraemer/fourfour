import { host, W2SetBoards } from '#design-system/index.js';

export const meta = { title: 'B. Keyboard map', layer: 'composite', group: 'Settings round 2 (F21)', order: 7 };

export default function render() {
  return host(W2SetBoards, { variant: 'b-keys' });
}
