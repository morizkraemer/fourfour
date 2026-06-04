import './checkbox.css';
import { el } from '../utils/dom.js';

/*
 * Checkbox, Toggle, and Radio primitives — spec §2.4.
 * All return { element }. Clicking toggles visual state.
 */

// ── Checkbox ────────────────────────────────────────────────

const CHECK_SVG = `<svg width="10" height="10" viewBox="0 0 10 10" fill="none" xmlns="http://www.w3.org/2000/svg"><polyline points="2,5.5 4.2,7.5 8,2.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>`;

export function createCheckbox({ checked = false, label, onChange } = {}) {
  const icon = el('span', { class: 'ff-checkbox__icon', html: CHECK_SVG });
  const box = el('span', { class: 'ff-checkbox__box' }, [icon]);

  const children = [box];
  if (label) children.push(el('span', { class: 'ff-checkbox__label', text: label }));

  const classes = ['ff-checkbox'];
  if (checked) classes.push('ff-checkbox--on');

  const root = el('span', {
    class: classes.join(' '),
    role: 'checkbox',
    'aria-checked': String(checked),
    tabindex: '0',
  }, children);

  root.addEventListener('click', () => {
    const isOn = root.classList.toggle('ff-checkbox--on');
    root.setAttribute('aria-checked', String(isOn));
    onChange?.(isOn);
  });

  return { element: root };
}

// ── Toggle ──────────────────────────────────────────────────

export function createToggle({ on = false, label, onChange } = {}) {
  const knob = el('span', { class: 'ff-toggle__knob' });
  const track = el('span', { class: 'ff-toggle__track' }, [knob]);

  const children = [track];
  if (label) children.push(el('span', { class: 'ff-toggle__label', text: label }));

  const classes = ['ff-toggle'];
  if (on) classes.push('ff-toggle--on');

  const root = el('span', {
    class: classes.join(' '),
    role: 'switch',
    'aria-checked': String(on),
    tabindex: '0',
  }, children);

  root.addEventListener('click', () => {
    const isOn = root.classList.toggle('ff-toggle--on');
    root.setAttribute('aria-checked', String(isOn));
    onChange?.(isOn);
  });

  return { element: root };
}

// ── Radio ───────────────────────────────────────────────────

export function createRadio({ selected = false, label, name, onChange } = {}) {
  const dot = el('span', { class: 'ff-radio__dot' });
  const circle = el('span', { class: 'ff-radio__circle' }, [dot]);

  const children = [circle];
  if (label) children.push(el('span', { class: 'ff-radio__label', text: label }));

  const classes = ['ff-radio'];
  if (selected) classes.push('ff-radio--on');

  const root = el('span', {
    class: classes.join(' '),
    role: 'radio',
    'aria-checked': String(selected),
    tabindex: '0',
    dataset: name ? { name } : {},
  }, children);

  root.addEventListener('click', () => {
    const isOn = root.classList.toggle('ff-radio--on');
    root.setAttribute('aria-checked', String(isOn));
    onChange?.(isOn);
  });

  return { element: root };
}
