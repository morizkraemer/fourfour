import { host, InspBoards } from '#design-system/index.js';

export const meta = { title: "B. Tabbed panel, cues on waveform", layer: 'composite', group: "Inspector (F22)", order: 2 };

export default function render() {
  return host(InspBoards, { variant: "b" });
}
