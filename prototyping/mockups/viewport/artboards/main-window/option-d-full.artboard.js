import { host, MwBoards } from '#design-system/index.js';

export const meta = { title: "D. Open, column also spans player and status", layer: 'composite', group: "Main window (F23, F16)", order: 7 };

export default function render() {
  return host(MwBoards, { variant: "d", bottom: 'full' });
}
