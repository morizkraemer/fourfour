import { host, SetBoards } from '#design-system/index.js';

export const meta = { title: "Now", layer: 'composite', group: "Settings (F21)", order: 0 };

export default function render() {
  return host(SetBoards, { variant: "now" });
}
