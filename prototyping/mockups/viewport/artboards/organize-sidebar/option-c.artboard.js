import { host, OrgBoards } from '#design-system/index.js';

export const meta = { title: "C. Set builder", layer: 'composite', group: "Organize sidebar (F15, F16)", order: 3 };

export default function render() {
  return host(OrgBoards, { variant: "c" });
}
