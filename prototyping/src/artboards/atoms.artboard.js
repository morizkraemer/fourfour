import { el } from '../design-system/utils/dom.js';
import {
  createIcon,
  ICON_NAMES,
  createTagBadge,
  createStatusDot,
  createColorSwatch,
} from '../design-system/index.js';

export const meta = { title: 'Atoms (icon · badge · dot · swatch)', layer: 'primitive' };

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

const row = (children, gap = 'var(--ff-space-5)') =>
  el('div', { style: { display: 'flex', alignItems: 'center', gap, flexWrap: 'wrap' } }, children);

export default function render() {
  const icons = row(
    ICON_NAMES.map((name) => {
      const { element } = createIcon({ name, size: 16 });
      return el('span', { style: { color: 'var(--ff-text-mid)', display: 'inline-flex' } }, [element]);
    })
  );

  const badges = row([
    createTagBadge({ value: 1, variant: 'outline' }).element,
    createTagBadge({ value: 2, variant: 'filled' }).element,
    createTagBadge({ value: 3, variant: 'color', color: 4, size: 'sm' }).element,
    createTagBadge({ value: 5, variant: 'color', color: 1, size: 'sm' }).element,
  ]);

  const dots = row([
    createStatusDot({ status: 'online' }).element,
    createStatusDot({ status: 'warn' }).element,
    createStatusDot({ status: 'offline' }).element,
  ]);

  const swatches = row(
    [1, 2, 3, 4, 5, 6, 7].map((n) => createColorSwatch({ color: n, size: 14 }).element)
  );

  return el(
    'div',
    {
      style: {
        display: 'flex',
        flexDirection: 'column',
        gap: 'var(--ff-space-6)',
        width: '420px',
        padding: 'var(--ff-space-7)',
        background: 'var(--ff-surface)',
        border: '1px solid var(--ff-border)',
        borderRadius: 'var(--ff-radius-md)',
      },
    },
    [
      cap('ICONS'),
      icons,
      cap('TAG / DIGIT BADGE'),
      badges,
      cap('STATUS DOT'),
      dots,
      cap('CDJ COLOR SWATCH'),
      swatches,
    ]
  );
}
