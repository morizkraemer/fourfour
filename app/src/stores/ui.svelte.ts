/**
 * ui.svelte.ts — UI panel state for the Browse screen.
 * State lives in a single exported object so consumers can read + write fields.
 */

export type SplitPanelTarget = {
  /** Sidebar source key (label, playlist name, or USB path). */
  sourceId: string;
  /** Display title for the right panel header. */
  label: string;
};

export const ui = $state({
  /** Whether the right detail pane is visible. */
  detailPaneOpen: true,
  /** Whether the left sidebar is collapsed (future). */
  sidebarCollapsed: false,
  /** Which sidebar row is currently active (drives library view). */
  activeSidebarRow: 'All Tracks',
  /** Last-focused track list (split view); falls back to activeSidebarRow for file drop. */
  focusedListSource: null as string | null,
  /** Second list panel on the right (columns-2 sidebar action). */
  splitPanel: null as SplitPanelTarget | null,
  /** Preferences modal opened from the footer settings button. */
  settingsOpen: false,
  /** ⌘F inline filter — narrows visible track lists in place. */
  listFilter: '',
  /** Incremented to focus the filter field (GlobalHeader). */
  filterFocusNonce: 0,
});

/** Clear the inline list filter. */
export function clearListFilter() {
  ui.listFilter = '';
}

/** Focus the global filter input (⌘F). */
export function focusListFilter() {
  ui.filterFocusNonce += 1;
}

/** Toggle the detail pane. */
export function toggleDetailPane() {
  ui.detailPaneOpen = !ui.detailPaneOpen;
}

export function openSettings() {
  ui.settingsOpen = true;
}

export function closeSettings() {
  ui.settingsOpen = false;
}

/** Open a collection in the right-hand split list panel. */
export function openSplitPanel(sourceId: string, label: string) {
  ui.splitPanel = { sourceId, label };
}

/** Close split view and return to single-panel layout. */
export function closeSplitPanel() {
  ui.splitPanel = null;
}

/** Mark which list panel receives the next file drop (split / curate). */
export function focusListPanel(sourceId: string) {
  ui.focusedListSource = sourceId;
}

/** Sidebar source id used when importing via drag-and-drop. */
export function importDropTarget(): string {
  return ui.focusedListSource ?? ui.activeSidebarRow;
}
