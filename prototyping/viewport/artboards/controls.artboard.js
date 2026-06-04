import { el } from '../design-system/utils/dom.js';
import {
  createInput,
  createCheckbox,
  createToggle,
  createRadio,
  createSegmentedControl,
  createSlider,
} from '../design-system/index.js';

export const meta = { title: 'Controls (input · checkbox · toggle · radio · segment · slider)', layer: 'primitive' };

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

const col = (children, gap = 'var(--ff-space-4)') =>
  el('div', { style: { display: 'flex', flexDirection: 'column', gap } }, children);

export default function render() {
  /* ── Inputs ──────────────────────────────── */
  const inputs = col([
    createInput({ placeholder: 'Default input…' }).element,
    createInput({ variant: 'search', placeholder: 'Search tracks…' }).element,
    createInput({ variant: 'numeric', value: '128.00' }).element,
  ]);

  /* ── Checkboxes ──────────────────────────── */
  const checkboxes = row([
    createCheckbox({ label: 'Unchecked' }).element,
    createCheckbox({ checked: true, label: 'Checked' }).element,
    createCheckbox({ label: 'With label' }).element,
  ]);

  /* ── Toggles ─────────────────────────────── */
  const toggles = row([
    createToggle({ label: 'Off' }).element,
    createToggle({ on: true, label: 'On' }).element,
  ]);

  /* ── Radios ──────────────────────────────── */
  const radios = row([
    createRadio({ label: 'Option A' }).element,
    createRadio({ selected: true, label: 'Option B' }).element,
    createRadio({ label: 'Option C' }).element,
  ]);

  /* ── Segmented control ───────────────────── */
  const segmented = row([
    createSegmentedControl({
      items: ['All', 'Tracks', 'Playlists'],
      activeIndex: 0,
    }).element,
    createSegmentedControl({
      items: ['Color', 'Mono'],
      activeIndex: 1,
    }).element,
  ]);

  /* ── Sliders ─────────────────────────────── */
  const sliders = col([
    el('div', { style: { width: '200px' } }, [
      createSlider({ min: 0, max: 100, value: 30 }).element,
    ]),
    el('div', { style: { width: '200px' } }, [
      createSlider({ min: 0, max: 100, value: 75 }).element,
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
      cap('INPUT'),
      inputs,
      cap('CHECKBOX'),
      checkboxes,
      cap('TOGGLE'),
      toggles,
      cap('RADIO'),
      radios,
      cap('SEGMENTED CONTROL'),
      segmented,
      cap('SLIDER'),
      sliders,
    ]
  );
}
