import { host, W2SetBoards } from '#design-system/index.js';

export const meta = { title: 'A. Sheet by object (Keyboard)', layer: 'composite', group: 'Settings round 2 (F21)', order: 5 };

export default function render() {
  return host(W2SetBoards, { variant: 'a-keys' });
}
