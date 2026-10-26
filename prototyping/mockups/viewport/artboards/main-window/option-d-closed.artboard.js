import { host, MwBoards } from '#design-system/index.js';

export const meta = { title: "D. Closed (toggle off)", layer: 'composite', group: "Main window (F23, F16)", order: 6 };

export default function render() {
  return host(MwBoards, { variant: "d", boardOpen: false });
}
