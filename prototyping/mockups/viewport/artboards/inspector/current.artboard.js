import { host, InspBoards } from '#design-system/index.js';

export const meta = { title: "Now", layer: 'composite', group: "Inspector (F22)", order: 0 };

export default function render() {
  return host(InspBoards, { variant: "now" });
}
