import { el } from '../design-system/utils/dom.js';
import {
  createKbd,
  createStars,
  createColorPicker,
} from '../design-system/index.js';

export const meta = { title: 'Primitives II (kbd · stars · color-picker)', layer: 'primitive' };

const cap = (t) =>
  el('div', {
    style: {
      fontFamily: 'var(--ff-font-mono)',
      fontSize: 'var(--ff-type-label)',
      letterSpacing: '0.06em',
      fontWeight: '500',
      color: 'var(--ff-faint)',
      marginTop: 'var(--ff-space-4)',
    },
    text: t,
  });

const row = (children, gap = 'var(--ff-space-5)') =>
  el('div', { style: { display: 'flex', alignItems: 'center', gap, flexWrap: 'wrap' } }, children);

export default function render() {
  /* ── Kbd ─────────────────────────────────── */
  const kbds = row([
    createKbd({ key: '⌘K' }).element,
    createKbd({ key: 'Esc' }).element,
    createKbd({ key: '⏎' }).element,
    createKbd({ key: '⌘F' }).element,
    createKbd({ key: '⇧⇥' }).element,
    createKbd({ key: 'Del' }).element,
  ]);

  /* ── Stars ───────────────────────────────── */
  const stars = el('div', {
    style: { display: 'flex', flexDirection: 'column', gap: 'var(--ff-space-4)' },
  }, [
    row([
      createStars({ rating: 0 }).element,
      el('span', { style: { fontSize: 'var(--ff-type-caption)', color: 'var(--ff-muted)' }, text: '0 stars' }),
    ]),
    row([
      createStars({ rating: 3 }).element,
      el('span', { style: { fontSize: 'var(--ff-type-caption)', color: 'var(--ff-muted)' }, text: '3 stars' }),
    ]),
    row([
      createStars({ rating: 5 }).element,
      el('span', { style: { fontSize: 'var(--ff-type-caption)', color: 'var(--ff-muted)' }, text: '5 stars' }),
    ]),
    row([
      createStars({ rating: 2, interactive: true }).element,
      el('span', { style: { fontSize: 'var(--ff-type-caption)', color: 'var(--ff-muted)' }, text: 'interactive (click)' }),
    ]),
  ]);

  /* ── Color picker ────────────────────────── */
  const pickers = el('div', {
    style: { display: 'flex', flexDirection: 'column', gap: 'var(--ff-space-4)' },
  }, [
    row([
      createColorPicker({ selected: 0 }).element,
      el('span', { style: { fontSize: 'var(--ff-type-caption)', color: 'var(--ff-muted)' }, text: 'none selected' }),
    ]),
    row([
      createColorPicker({ selected: 3 }).element,
      el('span', { style: { fontSize: 'var(--ff-type-caption)', color: 'var(--ff-muted)' }, text: 'yellow selected' }),
    ]),
    row([
      createColorPicker({ selected: 5 }).element,
      el('span', { style: { fontSize: 'var(--ff-type-caption)', color: 'var(--ff-muted)' }, text: 'blue selected (click to change)' }),
    ]),
  ]);

  return el(
    'div',
    {
      style: {
        display: 'flex',
        flexDirection: 'column',
        gap: 'var(--ff-space-4)',
        width: '420px',
        padding: 'var(--ff-space-7)',
        background: 'var(--ff-surface)',
        border: '1px solid var(--ff-border)',
        borderRadius: 'var(--ff-radius-md)',
      },
    },
    [
      cap('KEYBOARD HINTS'),
      kbds,
      cap('STAR RATING'),
      stars,
      cap('CDJ COLOR PICKER'),
      pickers,
    ]
  );
}
