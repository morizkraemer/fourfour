import { host, OrgBoards } from '#design-system/index.js';

export const meta = { title: "B. Drop to start", layer: 'composite', group: "Organize sidebar (F15, F16)", order: 2 };

export default function render() {
  return host(OrgBoards, { variant: "b" });
}
