import { el } from '../design-system/utils/dom.js';
import {
  createSidebarSection,
  createSidebarRow,
  createPanelHeader,
  createFilterInput,
  createStatusBar,
  createBulkActionBar,
  createTargetChip
} from '../design-system/index.js';

export const meta = { title: 'Navigation Chrome (sidebar-section · panel-header · filter · status · bulk)', layer: 'composite' };

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
  // 1. Sidebar Sections using createSidebarSection
  const favSection = createSidebarSection({
    label: 'FAVORITES',
    showAdd: true,
    onAdd: () => alert('Add Favorite'),
    rows: [
      createSidebarRow({ kind: 'favorite', digit: 1, label: 'Peak Time', count: 24 }).element,
      createSidebarRow({ kind: 'favorite', digit: 2, label: 'Warmup', count: 18, state: 'selected' }).element
    ]
  }).element;

  const librarySection = createSidebarSection({
    label: 'LIBRARY',
    rows: [
      createSidebarRow({ kind: 'leaf', icon: 'list', label: 'All Tracks', count: 4218 }).element,
      createSidebarRow({ kind: 'leaf', icon: 'clock-3', label: 'Recently Added', count: 52 }).element
    ]
  }).element;

  const sidebarContainer = el('div', {
    style: {
      width: '240px',
      background: 'var(--ff-bg)',
      border: '1px solid var(--ff-border)',
      borderRadius: 'var(--ff-radius-md)',
      display: 'flex',
      flexDirection: 'column',
      padding: '4px 0'
    }
  }, [favSection, librarySection]);

  // 2. Panel Headers
  const headerLibrary = createPanelHeader({
    title: 'All Tracks',
    subtitle: '4,218 tracks',
    variant: 'library',
    actions: [
      { label: 'Import Folder', variant: 'ghost', icon: 'plus', onClick: () => alert('Import') }
    ]
  }).element;

  const headerPlaylist = createPanelHeader({
    title: 'Festival Prep',
    subtitle: '40 tracks',
    variant: 'playlist-source',
    actions: [
      { label: 'Filter', variant: 'ghost', icon: 'search', onClick: () => alert('Filter') },
      { label: 'Curate Playlist', variant: 'primary', icon: 'list', onClick: () => alert('Curate') }
    ]
  }).element;

  const headerUsb = createPanelHeader({
    title: 'SanDisk 64GB',
    subtitle: 'Mounted · Synced',
    variant: 'usb-source',
    actions: [
      { label: 'Eject', variant: 'ghost', icon: 'eject', onClick: () => alert('Eject') },
      { label: 'Wipe', variant: 'destructive', onClick: () => alert('Wipe') },
      { label: 'Sync USB', variant: 'primary', icon: 'plus', onClick: () => alert('Sync') }
    ]
  }).element;

  // Header in Target mode showing Target Chip
  const targetChip = createTargetChip({
    name: 'SanDisk 64GB / Sunday Closing',
    onClose: () => alert('Close target')
  }).element;

  const headerTarget = createPanelHeader({
    variant: 'in-target-mode',
    targetChip: targetChip,
    actions: [
      { label: 'Save Cues', variant: 'primary', onClick: () => alert('Saved') }
    ]
  }).element;

  // Header in Filtering mode showing Filter Input
  const filterInput = createFilterInput({
    placeholder: 'Filter tracks...',
    count: '12 matches',
    onClose: () => alert('Close filter'),
    onChange: (val) => console.log('Filter change:', val)
  }).element;

  const headerFiltering = createPanelHeader({
    variant: 'filtering',
    filterInput: filterInput
  }).element;

  const headersContainer = el('div', {
    style: {
      display: 'flex',
      flexDirection: 'column',
      gap: '12px',
      width: '100%',
      maxWidth: '680px'
    }
  }, [
    headerLibrary,
    headerPlaylist,
    headerUsb,
    headerTarget,
    headerFiltering
  ]);

  // 3. Status Bar
  const statusBarIdle = createStatusBar({
    message: 'Library Idle',
    count: '4,218 tracks'
  }).element;

  const statusBarSync = createStatusBar({
    message: 'Syncing to USB',
    context: 'Writing database...',
    showSpinner: true,
    progress: 68,
    count: '24 tracks remaining'
  }).element;

  const statusBarsContainer = el('div', {
    style: {
      display: 'flex',
      flexDirection: 'column',
      gap: '8px',
      width: '100%',
      maxWidth: '680px'
    }
  }, [
    statusBarIdle,
    statusBarSync
  ]);

  // 4. Bulk Action Bar
  const bulkBar = createBulkActionBar({
    count: 14,
    summary: 'Selected tracks in Saturday Playlist',
    actions: [
      { label: 'Set Color', variant: 'ghost', onClick: () => alert('Set Color') },
      { label: 'Add to Playlist…', variant: 'default', onClick: () => alert('Add to Playlist') },
      { label: 'Clear selection', variant: 'ghost', withEsc: true, onClick: () => alert('Clear') }
    ]
  }).element;

  return el('div', {
    style: {
      display: 'flex',
      flexDirection: 'column',
      gap: 'var(--ff-space-4)',
      width: '720px',
      padding: 'var(--ff-space-7)',
      background: 'var(--ff-surface)',
      border: '1px solid var(--ff-border)',
      borderRadius: 'var(--ff-radius-md)',
    }
  }, [
    cap('SIDEBAR SECTION'),
    sidebarContainer,
    cap('PANEL HEADERS'),
    headersContainer,
    cap('STATUS BARS'),
    statusBarsContainer,
    cap('BULK ACTION BAR'),
    el('div', { style: { width: '100%', maxWidth: '680px' } }, [bulkBar])
  ]);
}
