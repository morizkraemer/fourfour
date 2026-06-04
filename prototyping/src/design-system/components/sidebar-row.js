import './sidebar-row.css';
import { el } from '../utils/dom.js';
import { createIcon } from './icon.js';
import { createTagBadge } from './tag-badge.js';
import { createStatusDot } from './status-dot.js';

/*
 * Sidebar row (§6). One factory covers every row kind in the nav.
 *   kind: 'section' | 'favorite' | 'leaf' | 'playlist' | 'usb'
 *   state: 'rest' | 'hover' | 'active' | 'selected'   (items only)
 *
 * Per kind:
 *   section  → { label, action? }            uppercase mono group header + optional '+' action
 *   favorite → { label, digit, count?, state } numbered pill + label + count
 *   leaf     → { label, icon?, count?, state } lucide icon + label + count (Library items)
 *   playlist → { label, count?, state }        label + count
 *   usb      → { label, status?, state }        status dot + label
 */
export function createSidebarRow(props = {}) {
  if (props.kind === 'section') return renderSection(props);
  return renderItem(props);
}

function renderSection({ label, action }) {
  const children = [el('span', { class: 'ff-sbrow__section-label', text: label })];
  if (action) children.push(el('span', { class: 'ff-sbrow__section-action', text: action }));
  const cls = `ff-sbrow__section${action ? ' ff-sbrow__section--with-action' : ''}`;
  return { element: el('div', { class: cls }, children) };
}

function renderItem({ kind = 'leaf', label, icon, count, digit, status, state = 'rest', splitAction = true, onSplit }) {
  const selected = state === 'active' || state === 'selected';
  const children = [];

  if (kind === 'favorite') {
    children.push(createTagBadge({ value: digit, variant: selected ? 'filled' : 'outline', size: 'md' }).element);
  } else if (kind === 'usb') {
    children.push(createStatusDot({ status: status ?? 'offline' }).element);
  } else if (icon) {
    children.push(createIcon({ name: icon, size: 13 }).element);
  }

  children.push(el('span', { class: 'ff-sbrow__label', text: label }));

  // Trailing zone: count (rest) ⇄ open-in-side-panel action (hover).
  // The action reveals on row hover and opens this collection in a second
  // list panel (the Pencil "2-column" view).
  if (count != null) children.push(el('span', { class: 'ff-sbrow__count', text: String(count) }));
  if (splitAction) {
    const splitBtn = el('button', {
      class: 'ff-sbrow__action',
      title: 'Open in side panel',
      onClick: (e) => { e.stopPropagation(); if (typeof onSplit === 'function') onSplit(); },
    }, [createIcon({ name: 'columns-2', size: 13 }).element]);
    children.push(splitBtn);
  }

  return { element: el('div', { class: `ff-sbrow ff-sbrow--${state}` }, children) };
}
