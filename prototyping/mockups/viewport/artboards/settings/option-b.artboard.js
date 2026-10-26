import { host, SetBoards } from '#design-system/index.js';

export const meta = { title: "B. One searchable page", layer: 'composite', group: "Settings (F21)", order: 2 };

export default function render() {
  return host(SetBoards, { variant: "b" });
}
