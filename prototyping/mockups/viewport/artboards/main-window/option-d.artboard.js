import { host, MwBoards } from '#design-system/index.js';

export const meta = { title: "D. Board column outside the main view (open, above player)", layer: 'composite', group: "Main window (F23, F16)", order: 5 };

export default function render() {
  return host(MwBoards, { variant: "d" });
}
