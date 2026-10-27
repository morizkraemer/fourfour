import { host, InspBoards } from '#design-system/index.js';

export const meta = { title: "A. Live form panel", layer: 'composite', group: "Inspector (F22)", order: 1 };

export default function render() {
  return host(InspBoards, { variant: "a" });
}
