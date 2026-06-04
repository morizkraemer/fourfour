import { el } from '../design-system/utils/dom.js';
import { host, Button } from '../design-system/svelte.js';

export const meta = { title: 'Button', layer: 'primitive' };

const btn = (props) => host(Button, props);

const cap = (t) =>
  el('div', {
    style: {
      fontFamily: 'var(--ff-font-mono)',
      fontSize: 'var(--ff-type-label)',
      letterSpacing: '0.06em',
      fontWeight: '500',
      color: 'var(--ff-faint)',
    },
    text: t,
  });

const row = (children) =>
  el('div', { style: { display: 'flex', alignItems: 'center', gap: 'var(--ff-space-5)' } }, children);

export default function render() {
  return el(
    'div',
    {
      style: {
        display: 'flex',
        flexDirection: 'column',
        gap: 'var(--ff-space-6)',
        padding: 'var(--ff-space-7)',
        background: 'var(--ff-surface)',
        border: '1px solid var(--ff-border)',
        borderRadius: 'var(--ff-radius-md)',
      },
    },
    [
      cap('VARIANTS · (hover is live)'),
      row([
        btn({ label: 'Curate', variant: 'primary' }),
        btn({ label: 'Import', variant: 'default' }),
        btn({ label: 'Filter', variant: 'ghost' }),
        btn({ label: 'Delete', variant: 'destructive' }),
      ]),
      cap('WITH ICON'),
      row([
        btn({ label: 'Add', icon: 'plus', variant: 'default' }),
        btn({ label: 'Search', icon: 'search', variant: 'ghost' }),
      ]),
      cap('ICON BUTTON'),
      row([
        btn({ icon: 'eject', iconOnly: true, variant: 'ghost' }),
        btn({ icon: 'x', iconOnly: true, variant: 'ghost' }),
        btn({ icon: 'disc', iconOnly: true, variant: 'default' }),
      ]),
      cap('DISABLED'),
      row([
        btn({ label: 'Curate', variant: 'primary', disabled: true }),
        btn({ label: 'Import', variant: 'default', disabled: true }),
      ]),
    ]
  );
}
