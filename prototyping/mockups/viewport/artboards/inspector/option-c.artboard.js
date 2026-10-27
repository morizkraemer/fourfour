import { host, InspBoards } from '#design-system/index.js';

export const meta = { title: "C. Wide editor strip", layer: 'composite', group: "Inspector (F22)", order: 3 };

export default function render() {
  return host(InspBoards, { variant: "c" });
}
