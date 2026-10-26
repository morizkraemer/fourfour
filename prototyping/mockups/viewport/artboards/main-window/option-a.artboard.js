import { host, MwBoards } from '#design-system/index.js';

export const meta = { title: "A. One right dock, two tabs", layer: 'composite', group: "Main window (F23, F16)", order: 2 };

export default function render() {
  return host(MwBoards, { variant: "a" });
}
