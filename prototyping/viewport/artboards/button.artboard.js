import { el } from '../design-system/utils/dom.js';
import { createButton } from '../design-system/index.js';

export const meta = { title: 'Button', layer: 'primitive' };

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
        createButton({ label: 'Curate', variant: 'primary' }).element,
        createButton({ label: 'Import', variant: 'default' }).element,
        createButton({ label: 'Filter', variant: 'ghost' }).element,
        createButton({ label: 'Delete', variant: 'destructive' }).element,
      ]),
      cap('WITH ICON'),
      row([
        createButton({ label: 'Add', icon: 'plus', variant: 'default' }).element,
        createButton({ label: 'Search', icon: 'search', variant: 'ghost' }).element,
      ]),
      cap('ICON BUTTON'),
      row([
        createButton({ icon: 'eject', iconOnly: true, variant: 'ghost' }).element,
        createButton({ icon: 'x', iconOnly: true, variant: 'ghost' }).element,
        createButton({ icon: 'disc', iconOnly: true, variant: 'default' }).element,
      ]),
      cap('DISABLED'),
      row([
        createButton({ label: 'Curate', variant: 'primary', disabled: true }).element,
        createButton({ label: 'Import', variant: 'default', disabled: true }).element,
      ]),
    ]
  );
}
