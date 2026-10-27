import { host, W2SetBoards } from '#design-system/index.js';

export const meta = { title: 'C. Side panel next to the player', layer: 'composite', group: 'Settings round 2 (F21)', order: 8 };

export default function render() {
  return host(W2SetBoards, { variant: 'c' });
}
