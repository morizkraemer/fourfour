import { host, W2SetBoards } from '#design-system/index.js';

export const meta = { title: 'Now: Keyboard shortcuts', layer: 'composite', group: 'Settings round 2 (F21)', order: 2 };

export default function render() {
  return host(W2SetBoards, { variant: 'now-shortcuts' });
}
