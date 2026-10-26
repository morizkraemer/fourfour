import { host, Decisions } from '#design-system/index.js';
import { DECISIONS } from '#design-system/local/decisions-w2-settings.js';

export const meta = { title: 'Decisions', layer: 'composite', group: 'Settings round 2 (F21)', order: 9 };

export default function render() {
  return host(Decisions, DECISIONS);
}
