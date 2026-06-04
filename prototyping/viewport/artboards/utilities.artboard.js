import { el } from '../design-system/utils/dom.js';
import {
  createProgressBar,
  createSpinner,
  createEmptyState,
  createTagChip,
  createDigitPill,
  createTargetChip,
  createNudge
} from '../design-system/index.js';

export const meta = { title: 'Utilities (progress · spinner · empty · chip · nudge)', layer: 'primitive' };

const cap = (t) =>
  el('div', {
    style: {
      fontFamily: 'var(--ff-font-mono)',
      fontSize: 'var(--ff-type-label)',
      letterSpacing: '0.06em',
      fontWeight: '500',
      color: 'var(--ff-faint)',
      marginTop: 'var(--ff-space-4)',
      marginBottom: 'var(--ff-space-2)',
    },
    text: t,
  });

const row = (children, gap = 'var(--ff-space-5)') =>
  el('div', { style: { display: 'flex', alignItems: 'center', gap, flexWrap: 'wrap' } }, children);

export default function render() {
  // Progress bars
  const detBar = createProgressBar({ value: 45, variant: 'determinate' }).element;
  const indetBar = createProgressBar({ variant: 'indeterminate' }).element;
  const thinBar = createProgressBar({ value: 80, variant: 'thin' }).element;

  const progressSection = el('div', { style: { display: 'flex', flexDirection: 'column', gap: '10px', width: '240px' } }, [
    detBar,
    indetBar,
    thinBar
  ]);

  // Spinners
  const defaultSpinner = createSpinner().element;
  const smallSpinner = createSpinner({ size: 'small' }).element;

  const spinnersSection = row([
    defaultSpinner,
    el('span', { style: { fontSize: '11px', color: 'var(--ff-muted)' }, text: 'default (11px)' }),
    smallSpinner,
    el('span', { style: { fontSize: '11px', color: 'var(--ff-muted)' }, text: 'small (9px)' })
  ]);

  // Empty State
  const emptyState = createEmptyState({
    title: 'No tracks loaded',
    sub: 'Import a folder or drop audio files anywhere in this window.',
  }).element;

  // Chips & Pills
  const tagChip = createTagChip({ digit: 1, name: 'Warmup Set' }).element;
  const digitPill = createDigitPill({ value: 4 }).element;
  const targetChip = createTargetChip({ name: 'Peak Time Tracks', onClose: () => alert('Close') }).element;

  const chipsSection = row([
    tagChip,
    digitPill,
    targetChip
  ]);

  // Nudge
  const nudgeBpm = createNudge({
    value: '128.00',
    label: 'BPM',
    onChange: (action) => alert(`BPM nudge: ${action}`)
  }).element;

  const nudgeKey = createNudge({
    value: '8A',
    label: 'Key',
    onChange: (action) => alert(`Key nudge: ${action}`)
  }).element;

  const nudgesSection = row([
    nudgeBpm,
    nudgeKey
  ]);

  return el('div', {
    style: {
      display: 'flex',
      flexDirection: 'column',
      gap: 'var(--ff-space-4)',
      width: '420px',
      padding: 'var(--ff-space-7)',
      background: 'var(--ff-surface)',
      border: '1px solid var(--ff-border)',
      borderRadius: 'var(--ff-radius-md)',
    }
  }, [
    cap('PROGRESS BARS'),
    progressSection,
    cap('SPINNERS'),
    spinnersSection,
    cap('EMPTY STATE'),
    emptyState,
    cap('TAG CHIPS & PILLS'),
    chipsSection,
    cap('NUDGE ADJUSTERS'),
    nudgesSection
  ]);
}
