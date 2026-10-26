import { host, MwBoards } from '#design-system/index.js';

export const meta = { title: "C. Inline row editor, board right", layer: 'composite', group: "Main window (F23, F16)", order: 4 };

export default function render() {
  return host(MwBoards, { variant: "c" });
}
