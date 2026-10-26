import { host, Decisions } from '#design-system/index.js';
import { DECISIONS } from '#design-system/local/decisions-w2-inspector.js';

export const meta = { title: 'Decisions', layer: 'composite', group: 'Inspector round 2 (F22)', order: 9 };

export default function render() {
  return host(Decisions, DECISIONS);
}
