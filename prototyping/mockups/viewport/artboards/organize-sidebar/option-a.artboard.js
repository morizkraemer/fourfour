import { host, OrgBoards } from '#design-system/index.js';

export const meta = { title: "A. Destination cards", layer: 'composite', group: "Organize sidebar (F15, F16)", order: 1 };

export default function render() {
  return host(OrgBoards, { variant: "a" });
}
