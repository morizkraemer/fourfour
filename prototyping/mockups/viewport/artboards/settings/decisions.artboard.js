import { host, Decisions } from '#design-system/index.js';
import { DECISIONS } from '#design-system/local/decisions-settings.js';

export const meta = { title: 'Decisions', layer: 'composite', group: "Settings (F21)", order: 9 };

export default function render() {
  return host(Decisions, DECISIONS);
}
