import { host, W2SetBoards } from '#design-system/index.js';

export const meta = { title: 'Now: General', layer: 'composite', group: 'Settings round 2 (F21)', order: 0 };

export default function render() {
  return host(W2SetBoards, { variant: 'now-general' });
}
