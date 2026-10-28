import { host, Decisions } from '#design-system/index.js';
import { DECISIONS } from '#design-system/local/decisions-zoom-look.js';

export const meta = { title: 'Decisions', layer: 'composite', group: 'Zoom look (Q1-Q3)', order: 9 };

export default function render() {
  return host(Decisions, DECISIONS);
}
