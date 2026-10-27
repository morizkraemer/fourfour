import { host, Decisions } from '#design-system/index.js';
import { DECISIONS } from '#design-system/local/decisions-organize-sidebar.js';

export const meta = { title: 'Decisions', layer: 'composite', group: "Organize sidebar (F15, F16)", order: 9 };

export default function render() {
  return host(Decisions, DECISIONS);
}
