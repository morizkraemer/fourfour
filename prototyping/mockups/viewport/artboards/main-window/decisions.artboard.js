import { host, Decisions } from '#design-system/index.js';
import { DECISIONS } from '#design-system/local/decisions-main-window.js';

export const meta = { title: 'Decisions', layer: 'composite', group: "Main window (F23, F16)", order: 9 };

export default function render() {
  return host(Decisions, DECISIONS);
}
