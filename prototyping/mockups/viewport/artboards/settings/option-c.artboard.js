import { host, SetBoards } from '#design-system/index.js';

export const meta = { title: "C. Settings where they matter", layer: 'composite', group: "Settings (F21)", order: 3 };

export default function render() {
  return host(SetBoards, { variant: "c" });
}
