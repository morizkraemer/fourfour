import { el } from '../design-system/utils/dom.js';
import { host, SidebarRow } from '../design-system/svelte.js';

export const meta = { title: 'Sidebar Row', layer: 'primitive' };

// A section = header + its item rows, stacked with a 1px gap. Non-first groups
// get a hairline top divider, matching §6.
function group({ header, items, first = false }) {
  const rows = [host(SidebarRow, { kind: 'section', ...header })];
  for (const it of items) rows.push(host(SidebarRow, it));
  return el(
    'div',
    {
      style: {
        display: 'flex',
        flexDirection: 'column',
        gap: '1px',
        padding: first ? 'var(--ff-space-3) 0' : 'var(--ff-space-4) 0 var(--ff-space-3)',
        borderTop: first ? 'none' : '1px solid var(--ff-border)',
      },
    },
    rows
  );
}

export default function render() {
  return el(
    'div',
    {
      style: {
        width: '240px',
        display: 'flex',
        flexDirection: 'column',
        padding: 'var(--ff-space-3) 0',
        background: 'var(--ff-bg)',
        border: '1px solid var(--ff-border)',
        borderRadius: 'var(--ff-radius-md)',
      },
    },
    [
      group({
        first: true,
        header: { label: 'FAVORITES', action: '+' },
        items: [
          { kind: 'favorite', digit: 1, label: 'Peak Time', count: 24 },
          { kind: 'favorite', digit: 2, label: 'Warmup', count: 18, state: 'selected' },
          { kind: 'favorite', digit: 3, label: 'Weird', count: 7 },
        ],
      }),
      group({
        header: { label: 'LIBRARY' },
        items: [
          { kind: 'leaf', icon: 'list', label: 'All Tracks', count: 4218 },
          { kind: 'leaf', icon: 'clock-3', label: 'Recently Added', count: 52, state: 'hover' },
          { kind: 'leaf', icon: 'plus', label: 'Unfiled', count: 311 },
        ],
      }),
      group({
        header: { label: 'PLAYLISTS', action: '+' },
        items: [
          { kind: 'playlist', label: 'Saturday', count: 40 },
          { kind: 'playlist', label: 'Sunday Closing', count: 28 },
          { kind: 'playlist', label: 'Festival Prep' },
        ],
      }),
      group({
        header: { label: 'USB DEVICES' },
        items: [
          { kind: 'usb', label: 'SanDisk 64GB', status: 'online' },
          { kind: 'usb', label: 'Tour Stick', status: 'offline' },
        ],
      }),
    ]
  );
}
