import { host, W2SetBoards } from '#design-system/index.js';

export const meta = { title: 'A. Sheet by object (Analysis)', layer: 'composite', group: 'Settings round 2 (F21)', order: 4 };

export default function render() {
  return host(W2SetBoards, { variant: 'a' });
}
