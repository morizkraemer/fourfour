import { host, OrgBoards } from '#design-system/index.js';

export const meta = { title: "Now: empty state", layer: 'composite', group: "Organize sidebar (F15, F16)", order: 0 };

export default function render() {
  return host(OrgBoards, { variant: "now" });
}
