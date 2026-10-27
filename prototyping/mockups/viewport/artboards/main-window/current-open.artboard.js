import { host, MwBoards } from '#design-system/index.js';

export const meta = { title: "Now: organize drawer open", layer: 'composite', group: "Main window (F23, F16)", order: 1 };

export default function render() {
  return host(MwBoards, { variant: "now-open" });
}
