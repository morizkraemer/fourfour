import { host, SetBoards } from '#design-system/index.js';

export const meta = { title: "A. Preferences sheet", layer: 'composite', group: "Settings (F21)", order: 1 };

export default function render() {
  return host(SetBoards, { variant: "a" });
}
