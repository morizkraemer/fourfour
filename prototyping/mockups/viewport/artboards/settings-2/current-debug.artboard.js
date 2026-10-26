import { host, W2SetBoards } from '#design-system/index.js';

export const meta = { title: 'Now: Debug', layer: 'composite', group: 'Settings round 2 (F21)', order: 3 };

export default function render() {
  return host(W2SetBoards, { variant: 'now-debug' });
}
