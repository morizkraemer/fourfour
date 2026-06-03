import { el } from '../design-system/utils/dom.js';
import {
  createToast,
  createDialog,
  createInlineAlert,
  createMenu,
  createCommandPalette
} from '../design-system/index.js';

export const meta = { title: 'Overlays (toast · dialog · alert · menu · palette)', layer: 'composite' };

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

export default function render() {
  // Toasts
  const defaultToast = createToast({ message: 'Track analyzed successfully', sub: '128 BPM · 8A' }).element;
  const successToast = createToast({ message: 'USB Sync Complete', sub: 'PIONEER drive mounted', variant: 'success' }).element;
  const errorToast = createToast({
    message: 'Write failed',
    sub: 'Device disconnected unexpectedly',
    variant: 'error',
    showUndo: true,
    onUndo: () => alert('Undo triggered!')
  }).element;

  const toasts = el('div', { style: { display: 'flex', flexDirection: 'column', gap: '8px' } }, [
    defaultToast,
    successToast,
    errorToast
  ]);

  // Dialog (rendered inline without backdrop)
  const dialog = createDialog({
    title: 'Wipe USB Drive',
    body: 'This will erase all content on the PIONEER drive, including all sync directories, cues, and metadata files. This action cannot be undone.',
    actions: [
      { label: 'Cancel', variant: 'ghost', onClick: () => alert('Cancel') },
      { label: 'Wipe', variant: 'destructive', onClick: () => alert('Wipe') },
    ],
    withBackdrop: false
  }).element;

  // Inline Alerts
  const infoAlert = createInlineAlert({
    variant: 'info',
    html: 'Analysis metadata will be stored in the local cache. USB sync will run <strong>significantly faster</strong> on subsequent exports.'
  }).element;

  const warnAlert = createInlineAlert({
    variant: 'warn',
    html: 'Conflict detected: <strong>PIONEER database has newer cues</strong> than local library. Sync will overwrite local database.'
  }).element;

  const alerts = el('div', { style: { display: 'flex', flexDirection: 'column', gap: '8px' } }, [
    infoAlert,
    warnAlert
  ]);

  // Menus
  const contextMenu = createMenu({
    items: [
      { label: 'Play Track', shortcut: '⏎' },
      { label: 'Add to Curate Playlist', shortcut: '⌘P' },
      { isSeparator: true },
      { label: 'Locate File on Disk…' },
      { label: 'Reanalyze Track' },
      { isSeparator: true },
      { label: 'Remove Track', variant: 'danger', onClick: () => alert('Remove') },
      { label: 'Permanently Delete', variant: 'disabled', disabled: true }
    ]
  }).element;

  // Command Palette
  const palette = createCommandPalette({
    sections: [
      {
        title: 'Recent Searches',
        items: [
          { name: 'Daft Punk', sub: 'Artist', right: '⌥1' },
          { name: 'Summer Vibes 2026', sub: 'Playlist', right: '⌥2' }
        ]
      },
      {
        title: 'Commands',
        items: [
          { name: 'Sync library to USB', sub: 'Sync', right: '⌘S' },
          { name: 'Import folders', sub: 'Library', right: '⌘O' }
        ]
      }
    ]
  }).element;

  return el('div', {
    style: {
      display: 'flex',
      flexDirection: 'column',
      gap: 'var(--ff-space-4)',
      width: '580px',
      padding: 'var(--ff-space-7)',
      background: 'var(--ff-surface)',
      border: '1px solid var(--ff-border)',
      borderRadius: 'var(--ff-radius-md)',
    }
  }, [
    cap('TOAST NOTIFICATIONS'),
    toasts,
    cap('INLINE ALERTS'),
    alerts,
    cap('DIALOGS (INLINE PREVIEW)'),
    dialog,
    cap('CONTEXT MENU'),
    el('div', { style: { display: 'flex', justifyContent: 'flex-start' } }, [contextMenu]),
    cap('COMMAND PALETTE (INLINE PREVIEW)'),
    palette
  ]);
}
